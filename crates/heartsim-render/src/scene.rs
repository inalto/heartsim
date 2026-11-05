use crate::camera::OrbitCamera;
use crate::glyph::{GlyphInstance, GlyphSet, GlyphSetId, unit_tube};
use crate::layer::{Layer, LayerDesc, LayerId, LayerUniform, ScalarMode, srgb_to_linear};
use crate::lut::{LUT_ROWS, LutId};
use crate::pick::{Hit, ray_aabb, ray_triangle};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use heartsim_geometry::{TriMesh, primitives::icosphere};
use std::num::NonZeroU64;
use wgpu::util::DeviceExt;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

const MESH_ATTRS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
const SCALAR_ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![2 => Float32];
const INSTANCE_ATTRS: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4];

/// Frame-wide shader constants (bind group 0, binding 0). Mirrors `Globals` in `common.wgsl`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SceneUniforms {
    /// World → clip transform.
    pub view_proj: [[f32; 4]; 4],
    /// Eye position (xyz).
    pub camera_pos: [f32; 4],
    /// Key light: unit direction towards the light (xyz), intensity (w).
    pub key_dir: [f32; 4],
    /// Fill light, same layout.
    pub fill_dir: [f32; 4],
    /// Point on the clip plane (xyz), enabled flag (w).
    pub clip_point: [f32; 4],
    /// Clip-plane normal (xyz); the side it points to is removed.
    pub clip_normal: [f32; 4],
    /// Linear RGB of the flat cut-away interior.
    pub cut_color: [f32; 4],
    /// Linear RGB at the top of the viewport.
    pub bg_top: [f32; 4],
    /// Linear RGB at the bottom of the viewport.
    pub bg_bottom: [f32; 4],
    /// Ambient, specular strength, shininess, rim strength.
    pub shading: [f32; 4],
    /// Rim power, viewport width, viewport height, unused.
    pub misc: [f32; 4],
}

impl Default for SceneUniforms {
    fn default() -> Self {
        let s = SceneSettings::default();
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 1.0, 0.0],
            key_dir: [0.0, 0.0, 1.0, s.key_intensity],
            fill_dir: [0.0, 0.0, 1.0, s.fill_intensity],
            clip_point: [0.0; 4],
            clip_normal: [1.0, 0.0, 0.0, 0.0],
            cut_color: pad3(s.cut_color),
            bg_top: pad3(s.background_top),
            bg_bottom: pad3(s.background_bottom),
            shading: [s.ambient, s.specular, s.shininess, s.rim_strength],
            misc: [s.rim_power, 1.0, 1.0, 0.0],
        }
    }
}

fn pad3(c: [f32; 3]) -> [f32; 4] {
    [c[0], c[1], c[2], 0.0]
}

fn srgb3(c: [f32; 3]) -> [f32; 3] {
    c.map(srgb_to_linear)
}

/// Look and lighting of the whole scene. All colours are **linear** RGB (the target is sRGB).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneSettings {
    /// Key light intensity. The key light sits up-left of the camera so shapes read the same from
    /// any orbit angle.
    pub key_intensity: f32,
    /// Fill light intensity (low, from the opposite side, keeps shadows from going black).
    pub fill_intensity: f32,
    /// Ambient term multiplying the albedo.
    pub ambient: f32,
    /// Blinn-Phong specular strength.
    pub specular: f32,
    /// Blinn-Phong exponent.
    pub shininess: f32,
    /// Rim-light strength; 0 disables it.
    pub rim_strength: f32,
    /// Rim falloff exponent (higher = thinner rim).
    pub rim_power: f32,
    /// Flat colour of the interior revealed by the clip plane.
    pub cut_color: [f32; 3],
    /// Background gradient, top of the viewport.
    pub background_top: [f32; 3],
    /// Background gradient, bottom of the viewport.
    pub background_bottom: [f32; 3],
}

impl Default for SceneSettings {
    fn default() -> Self {
        Self {
            key_intensity: 0.75,
            fill_intensity: 0.3,
            ambient: 0.22,
            specular: 0.25,
            shininess: 48.0,
            rim_strength: 0.25,
            rim_power: 3.0,
            cut_color: srgb3([0.86, 0.74, 0.55]),
            background_top: srgb3([0.17, 0.19, 0.24]),
            background_bottom: srgb3([0.05, 0.055, 0.07]),
        }
    }
}

/// Plane that removes part of every layer (and glyph set) that has `clip` set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipPlane {
    /// Whether the plane is active.
    pub enabled: bool,
    /// A point on the plane (mm).
    pub point: Vec3,
    /// Plane normal; geometry on the side it points to is removed.
    pub normal: Vec3,
}

impl Default for ClipPlane {
    fn default() -> Self {
        Self {
            enabled: false,
            point: Vec3::ZERO,
            normal: Vec3::X,
        }
    }
}

impl ClipPlane {
    /// Is `p` on the removed side of an enabled plane?
    #[must_use]
    pub fn removes(&self, p: Vec3) -> bool {
        self.enabled && (p - self.point).dot(self.normal) > 0.0
    }
}

/// Construction options of [`SceneRenderer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneOptions {
    /// MSAA sample count of the offscreen target: 1 (off) or 4. Use [`best_msaa_samples`] to ask
    /// the adapter; the pipelines are built for this count, so it cannot change later.
    pub msaa_samples: u32,
}

impl Default for SceneOptions {
    fn default() -> Self {
        Self { msaa_samples: 1 }
    }
}

/// 4 when the adapter can multisample-and-resolve both the colour and depth formats the scene
/// uses, otherwise 1.
#[must_use]
pub fn best_msaa_samples(adapter: &wgpu::Adapter) -> u32 {
    let ok = |f| {
        adapter
            .get_texture_format_features(f)
            .flags
            .contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4)
    };
    let resolve = adapter
        .get_texture_format_features(COLOR_FORMAT)
        .flags
        .contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE);
    if ok(COLOR_FORMAT) && ok(DEPTH_FORMAT) && resolve {
        4
    } else {
        1
    }
}

/// An anatomical label ready for the UI to draw: where, in widget-relative coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenLabel {
    /// Layer the label belongs to.
    pub layer: LayerId,
    /// Label text.
    pub text: String,
    /// Horizontal position, 0 (left) – 1 (right) of the viewport.
    pub u: f32,
    /// Vertical position, 0 (top) – 1 (bottom) of the viewport.
    pub v: f32,
    /// NDC depth (0 near – 1 far), for sorting overlapping labels.
    pub depth: f32,
}

#[derive(Debug)]
struct Pipelines {
    background: wgpu::RenderPipeline,
    opaque: wgpu::RenderPipeline,
    cut: wgpu::RenderPipeline,
    transparent_front: wgpu::RenderPipeline,
    transparent_back: wgpu::RenderPipeline,
    sphere: wgpu::RenderPipeline,
    tube: wgpu::RenderPipeline,
}

#[derive(Debug)]
struct UnitMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
}

impl UnitMesh {
    fn new(device: &wgpu::Device, mesh: &TriMesh, label: &str) -> Self {
        Self {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: mesh.indices.len() as u32,
        }
    }
}

#[derive(Debug)]
struct Targets {
    size: (u32, u32),
    /// Single-sample colour: the blit source and the screenshot source (read back on desktop only).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    /// Multisampled colour, resolved into `color`; only when MSAA is on.
    msaa_view: Option<wgpu::TextureView>,
    depth_view: wgpu::TextureView,
}

struct PipelineSpec<'a> {
    label: &'a str,
    vs: &'a str,
    fs: &'a str,
    buffers: &'a [Option<wgpu::VertexBufferLayout<'a>>],
    cull: wgpu::Face,
    blend: Option<wgpu::BlendState>,
    depth_write: bool,
    depth_compare: wgpu::CompareFunction,
}

/// Offscreen renderer for a layered anatomical scene.
///
/// The scene is a list of [`Layer`]s (chambers, conduction system, vessels, torso …), each with
/// its own visibility, opacity, colour map and scalar window, plus [`GlyphSet`]s of instanced
/// spheres and tubes (electrodes, catheters, the heart-vector arrow). Opaque layers are drawn
/// first with depth writes, then glyphs, then translucent layers sorted back to front by centroid
/// without depth writes. Per frame the CPU touches one uniform upload (O(layers)) plus whatever
/// scalar buffers the caller updates.
#[derive(Debug)]
pub struct SceneRenderer {
    pipelines: Pipelines,
    blit_pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    layer_layout: wgpu::BindGroupLayout,
    layer_buffer: wgpu::Buffer,
    layer_bind_group: wgpu::BindGroup,
    layer_slots: usize,
    layer_stride: usize,
    staging: Vec<u8>,
    sort_scratch: Vec<(f32, usize)>,
    lut_texture: wgpu::Texture,
    unit_sphere: UnitMesh,
    unit_tube: UnitMesh,
    blit_layout: wgpu::BindGroupLayout,
    blit_bind_group: Option<wgpu::BindGroup>,
    sampler: wgpu::Sampler,
    targets: Option<Targets>,
    samples: u32,
    layers: Vec<Layer>,
    glyph_sets: Vec<GlyphSet>,
    next_layer: u32,
    next_glyph_set: u32,
    primary: Option<LayerId>,
    /// Orbit camera; the UI drives it, the renderer reads it every frame.
    pub camera: OrbitCamera,
    /// Clip plane applied to layers and glyph sets whose `clip` flag is set.
    pub clip: ClipPlane,
    /// Lighting and colours.
    pub settings: SceneSettings,
}

impl SceneRenderer {
    /// Single-sample renderer; `target_format` is the format of the GUI pass the offscreen colour
    /// is blitted into.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target_format: wgpu::TextureFormat) -> Self {
        Self::with_options(device, queue, target_format, SceneOptions::default())
    }

    /// As [`Self::new`] with explicit [`SceneOptions`].
    pub fn with_options(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        options: SceneOptions,
    ) -> Self {
        let samples = options.msaa_samples.max(1);
        let common = include_str!("shaders/common.wgsl");
        let shader = |label: &str, body: &str| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(format!("{common}\n{body}").into()),
            })
        };
        let scene_shader = shader("heartsim scene shader", include_str!("shaders/scene.wgsl"));
        let glyph_shader = shader("heartsim glyph shader", include_str!("shaders/glyph.wgsl"));
        let blit_shader = device.create_shader_module(wgpu::include_wgsl!("shaders/blit.wgsl"));

        // LUT texture: one 256-texel row per `LutId`.
        let mut rows = Vec::with_capacity(LUT_ROWS as usize * 256);
        for id in [LutId::Voltage, LutId::Isochrone, LutId::Apd, LutId::Conduction] {
            rows.extend_from_slice(&id.default_table());
        }
        let lut_texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("heartsim lut"),
                size: wgpu::Extent3d {
                    width: 256,
                    height: LUT_ROWS,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            bytemuck::cast_slice(&rows),
        );
        let lut_view = lut_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("heartsim sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let globals = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("heartsim globals"),
            contents: bytemuck::bytes_of(&SceneUniforms::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("heartsim globals layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("heartsim globals bind group"),
            layout: &globals_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&lut_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let layer_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("heartsim layer layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: NonZeroU64::new(size_of::<LayerUniform>() as u64),
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("heartsim scene pipeline layout"),
            bind_group_layouts: &[Some(&globals_layout), Some(&layer_layout)],
            immediate_size: 0,
        });

        let layer_stride = size_of::<LayerUniform>()
            .next_multiple_of(device.limits().min_uniform_buffer_offset_alignment as usize);
        let layer_slots = 8;
        let (layer_buffer, layer_bind_group) =
            Self::make_layer_buffer(device, &layer_layout, layer_slots, layer_stride);

        let mesh_buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: 24,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &MESH_ATTRS,
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: 4,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &SCALAR_ATTRS,
            }),
        ];
        let glyph_buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: 24,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &MESH_ATTRS,
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<GlyphInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &INSTANCE_ATTRS,
            }),
        ];
        let make = |module: &wgpu::ShaderModule, spec: &PipelineSpec<'_>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(spec.label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some(spec.vs),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: spec.buffers,
                },
                primitive: wgpu::PrimitiveState {
                    cull_mode: Some(spec.cull),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(spec.depth_write),
                    depth_compare: Some(spec.depth_compare),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some(spec.fs),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend: spec.blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let less = wgpu::CompareFunction::Less;
        let pipelines = Pipelines {
            background: make(
                &scene_shader,
                &PipelineSpec {
                    label: "heartsim background",
                    vs: "vs_bg",
                    fs: "fs_bg",
                    buffers: &[],
                    cull: wgpu::Face::Back,
                    blend: None,
                    depth_write: false,
                    depth_compare: wgpu::CompareFunction::Always,
                },
            ),
            opaque: make(
                &scene_shader,
                &PipelineSpec {
                    label: "heartsim opaque layer",
                    vs: "vs_main",
                    fs: "fs_main",
                    buffers: &mesh_buffers,
                    cull: wgpu::Face::Back,
                    blend: None,
                    depth_write: true,
                    depth_compare: less,
                },
            ),
            cut: make(
                &scene_shader,
                &PipelineSpec {
                    label: "heartsim cut interior",
                    vs: "vs_main",
                    fs: "fs_cut",
                    buffers: &mesh_buffers,
                    cull: wgpu::Face::Front,
                    blend: None,
                    depth_write: true,
                    depth_compare: less,
                },
            ),
            transparent_front: make(
                &scene_shader,
                &PipelineSpec {
                    label: "heartsim transparent front",
                    vs: "vs_main",
                    fs: "fs_main",
                    buffers: &mesh_buffers,
                    cull: wgpu::Face::Back,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    depth_write: false,
                    depth_compare: less,
                },
            ),
            transparent_back: make(
                &scene_shader,
                &PipelineSpec {
                    label: "heartsim transparent back",
                    vs: "vs_main",
                    fs: "fs_main",
                    buffers: &mesh_buffers,
                    cull: wgpu::Face::Front,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    depth_write: false,
                    depth_compare: less,
                },
            ),
            sphere: make(
                &glyph_shader,
                &PipelineSpec {
                    label: "heartsim glyph spheres",
                    vs: "vs_sphere",
                    fs: "fs_glyph",
                    buffers: &glyph_buffers,
                    cull: wgpu::Face::Back,
                    blend: None,
                    depth_write: true,
                    depth_compare: less,
                },
            ),
            tube: make(
                &glyph_shader,
                &PipelineSpec {
                    label: "heartsim glyph tubes",
                    vs: "vs_tube",
                    fs: "fs_glyph",
                    buffers: &glyph_buffers,
                    cull: wgpu::Face::Back,
                    blend: None,
                    depth_write: true,
                    depth_compare: less,
                },
            ),
        };

        let (blit_layout, blit_pipeline) = Self::make_blit(device, &blit_shader, target_format);

        Self {
            pipelines,
            blit_pipeline,
            globals,
            globals_bind_group,
            layer_layout,
            layer_buffer,
            layer_bind_group,
            layer_slots,
            layer_stride,
            staging: Vec::new(),
            sort_scratch: Vec::new(),
            lut_texture,
            unit_sphere: UnitMesh::new(device, &icosphere(2), "heartsim unit sphere"),
            unit_tube: UnitMesh::new(device, &unit_tube(), "heartsim unit tube"),
            blit_layout,
            blit_bind_group: None,
            sampler,
            targets: None,
            samples,
            layers: Vec::new(),
            glyph_sets: Vec::new(),
            next_layer: 0,
            next_glyph_set: 0,
            primary: None,
            camera: OrbitCamera::default(),
            clip: ClipPlane::default(),
            settings: SceneSettings::default(),
        }
    }

    fn make_blit(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        target_format: wgpu::TextureFormat,
    ) -> (wgpu::BindGroupLayout, wgpu::RenderPipeline) {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("heartsim blit layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("heartsim blit pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("heartsim blit pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        (layout, pipeline)
    }

    fn make_layer_buffer(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        slots: usize,
        stride: usize,
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("heartsim layer uniforms"),
            size: (slots * stride) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("heartsim layer bind group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &buffer,
                    offset: 0,
                    size: NonZeroU64::new(size_of::<LayerUniform>() as u64),
                }),
            }],
        });
        (buffer, group)
    }

    /// MSAA sample count this renderer was built with.
    #[must_use]
    pub fn msaa_samples(&self) -> u32 {
        self.samples
    }

    // ---- layers --------------------------------------------------------------------------

    /// Upload a mesh as a new layer, visible and drawn in insertion order among opaque layers.
    /// The first layer added becomes the *primary* layer targeted by [`Self::set_scalars`].
    pub fn add_layer(&mut self, device: &wgpu::Device, desc: &LayerDesc<'_>) -> LayerId {
        let id = LayerId(self.next_layer);
        self.next_layer += 1;
        self.layers.push(Layer::new(id, device, desc));
        self.primary.get_or_insert(id);
        id
    }

    /// Remove a layer and free its GPU buffers. Returns whether it existed.
    pub fn remove_layer(&mut self, id: LayerId) -> bool {
        let before = self.layers.len();
        self.layers.retain(|l| l.id != id);
        if self.primary == Some(id) {
            self.primary = self.layers.first().map(|l| l.id);
        }
        self.layers.len() != before
    }

    /// All layers in insertion order.
    #[must_use]
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// Mutable access to all layers (toggle `visible`, `alpha`, `clip` …).
    pub fn layers_mut(&mut self) -> &mut [Layer] {
        &mut self.layers
    }

    /// Look a layer up by id.
    #[must_use]
    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    /// Mutable lookup by id.
    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    /// Update a layer's per-vertex scalars (length must equal its vertex count). This is the
    /// per-frame upload: one `write_buffer` of 4 bytes per vertex.
    pub fn set_layer_scalars(&self, queue: &wgpu::Queue, id: LayerId, scalars: &[f32]) {
        if let Some(l) = self.layer(id) {
            l.mesh.write_scalars(queue, scalars);
        }
    }

    /// Single-mesh convenience kept from the first renderer: replace the primary layer by `mesh`,
    /// colour-mapped through the voltage LUT over 0–1, and frame the camera on it.
    pub fn set_mesh(&mut self, device: &wgpu::Device, mesh: &TriMesh) {
        if let Some(old) = self.primary {
            self.remove_layer(old);
            self.primary = None;
        }
        let mut desc = LayerDesc::new("mesh", mesh);
        desc.scalar_mode = ScalarMode::Lut(LutId::Voltage);
        desc.base_color = [0.75, 0.25, 0.25];
        self.add_layer(device, &desc);
        let (lo, hi) = mesh.bounds();
        self.camera.frame(lo, hi);
    }

    /// Update the scalars of the primary layer (see [`Self::set_mesh`]).
    pub fn set_scalars(&self, queue: &wgpu::Queue, scalars: &[f32]) {
        if let Some(id) = self.primary {
            self.set_layer_scalars(queue, id, scalars);
        }
    }

    /// Aim the camera at the union of the bounds of all visible layers.
    pub fn frame_scene(&mut self) {
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for l in self.layers.iter().filter(|l| l.visible) {
            let (a, b) = l.mesh.bounds();
            lo = lo.min(a);
            hi = hi.max(b);
        }
        if lo.cmple(hi).all() {
            self.camera.frame(lo, hi);
        }
    }

    /// Replace one colour map. `table` has 256 sRGB-encoded RGBA entries.
    pub fn set_lut(&self, queue: &wgpu::Queue, id: LutId, table: &[[u8; 4]; 256]) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.lut_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: id as u32,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(table),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(1024),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }

    // ---- glyphs --------------------------------------------------------------------------

    /// Create an empty glyph set.
    pub fn add_glyph_set(&mut self, name: impl Into<String>) -> GlyphSetId {
        let id = GlyphSetId(self.next_glyph_set);
        self.next_glyph_set += 1;
        self.glyph_sets.push(GlyphSet::new(id, name.into()));
        id
    }

    /// Remove a glyph set. Returns whether it existed.
    pub fn remove_glyph_set(&mut self, id: GlyphSetId) -> bool {
        let before = self.glyph_sets.len();
        self.glyph_sets.retain(|g| g.id != id);
        self.glyph_sets.len() != before
    }

    /// All glyph sets.
    #[must_use]
    pub fn glyph_sets(&self) -> &[GlyphSet] {
        &self.glyph_sets
    }

    /// Mutable lookup by id (toggle `visible` / `clip`).
    pub fn glyph_set_mut(&mut self, id: GlyphSetId) -> Option<&mut GlyphSet> {
        self.glyph_sets.iter_mut().find(|g| g.id == id)
    }

    /// Replace the spheres of a set (electrodes, markers, catheter tips).
    pub fn set_spheres(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: GlyphSetId,
        items: &[GlyphInstance],
    ) {
        if let Some(g) = self.glyph_set_mut(id) {
            g.spheres.set(device, queue, items);
        }
    }

    /// Replace the tube segments of a set (arrow, vector loop, catheter bodies, pacing wires).
    pub fn set_tubes(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: GlyphSetId,
        items: &[GlyphInstance],
    ) {
        if let Some(g) = self.glyph_set_mut(id) {
            g.tubes.set(device, queue, items);
        }
    }

    // ---- rendering -----------------------------------------------------------------------

    fn ensure_targets(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        // WebGL2 devices cap textures at 2048-8192 px; a window larger than that renders at the cap
        // and the blit scales it up.
        let max = device.limits().max_texture_dimension_2d.max(1);
        let size = (size.0.clamp(1, max), size.1.clamp(1, max));
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let extent = wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        };
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("heartsim offscreen color"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let msaa_view = (self.samples > 1).then(|| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("heartsim offscreen msaa color"),
                    size: extent,
                    mip_level_count: 1,
                    sample_count: self.samples,
                    dimension: wgpu::TextureDimension::D2,
                    format: COLOR_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        });
        let depth_view = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("heartsim offscreen depth"),
                size: extent,
                mip_level_count: 1,
                sample_count: self.samples,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
        self.blit_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("heartsim blit bind group"),
            layout: &self.blit_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        }));
        self.targets = Some(Targets {
            size,
            color,
            color_view,
            msaa_view,
            depth_view,
        });
    }

    fn globals_for(&self, aspect: f32, size: (u32, u32)) -> SceneUniforms {
        let s = &self.settings;
        let (right, up, back) = self.camera.basis();
        // Lights are fixed in camera space so the model is lit the same from every orbit angle.
        let key = (-right * 0.5 + up * 0.7 + back * 0.6).normalize();
        let fill = (right * 0.7 - up * 0.2 + back * 0.4).normalize();
        let eye = self.camera.eye();
        let n = self.clip.normal.normalize_or_zero();
        SceneUniforms {
            view_proj: self.camera.view_projection(aspect).to_cols_array_2d(),
            camera_pos: eye.extend(0.0).to_array(),
            key_dir: key.extend(s.key_intensity).to_array(),
            fill_dir: fill.extend(s.fill_intensity).to_array(),
            clip_point: self
                .clip
                .point
                .extend(f32::from(u8::from(self.clip.enabled && n != Vec3::ZERO)))
                .to_array(),
            clip_normal: n.extend(0.0).to_array(),
            cut_color: pad3(s.cut_color),
            bg_top: pad3(s.background_top),
            bg_bottom: pad3(s.background_bottom),
            shading: [s.ambient, s.specular, s.shininess, s.rim_strength],
            misc: [s.rim_power, size.0 as f32, size.1 as f32, 0.0],
        }
    }

    /// Render the scene into the offscreen texture at `size` physical pixels, recording into
    /// `encoder`. Uniforms are uploaded with `queue.write_buffer`, so one frame needs its own
    /// `render_offscreen` call per submitted encoder.
    pub fn render_offscreen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        size: (u32, u32),
    ) {
        self.ensure_targets(device, size);
        let size = self.size();
        let aspect = size.0 as f32 / size.1 as f32;

        // Uniform slots: layers first, then glyph sets.
        let slots = self.layers.len() + self.glyph_sets.len();
        if slots > self.layer_slots {
            self.layer_slots = slots.next_power_of_two();
            let (buf, group) =
                Self::make_layer_buffer(device, &self.layer_layout, self.layer_slots, self.layer_stride);
            self.layer_buffer = buf;
            self.layer_bind_group = group;
        }
        let globals = self.globals_for(aspect, size);
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        self.staging.clear();
        self.staging.resize(slots * self.layer_stride, 0);
        for (i, l) in self.layers.iter().enumerate() {
            let u = LayerUniform::for_layer(l);
            let at = i * self.layer_stride;
            self.staging[at..at + size_of::<LayerUniform>()].copy_from_slice(bytemuck::bytes_of(&u));
        }
        for (j, g) in self.glyph_sets.iter().enumerate() {
            let u = LayerUniform {
                base_color: [1.0; 4],
                scalar: [0.0; 4],
                flags: [f32::from(u8::from(g.clip)), 0.0, 0.0, 0.0],
                contour_color: [0.0; 4],
            };
            let at = (self.layers.len() + j) * self.layer_stride;
            self.staging[at..at + size_of::<LayerUniform>()].copy_from_slice(bytemuck::bytes_of(&u));
        }
        if slots > 0 {
            queue.write_buffer(&self.layer_buffer, 0, &self.staging);
        }

        // Transparent layers back to front by distance of their centroid from the eye.
        let eye = self.camera.eye();
        self.sort_scratch.clear();
        self.sort_scratch.extend(
            self.layers
                .iter()
                .enumerate()
                .filter(|(_, l)| l.is_drawn() && l.is_transparent())
                .map(|(i, l)| (l.mesh.centroid().distance_squared(eye), i)),
        );
        self.sort_scratch.sort_by(|a, b| b.0.total_cmp(&a.0));

        let this = &*self;
        let targets = this.targets.as_ref().expect("targets exist after ensure_targets");
        let (view, resolve) = match &targets.msaa_view {
            Some(m) => (m, Some(&targets.color_view)),
            None => (&targets.color_view, None),
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("heartsim scene pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: resolve,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: if resolve.is_some() {
                        wgpu::StoreOp::Discard
                    } else {
                        wgpu::StoreOp::Store
                    },
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let stride = this.layer_stride as u32;
        pass.set_bind_group(0, &this.globals_bind_group, &[]);

        pass.set_pipeline(&this.pipelines.background);
        pass.set_bind_group(1, &this.layer_bind_group, &[0]);
        pass.draw(0..3, 0..1);

        // 1. opaque layers (+ the interior seen through the cut)
        for (i, l) in this.layers.iter().enumerate() {
            if !l.is_drawn() || l.is_transparent() {
                continue;
            }
            pass.set_bind_group(1, &this.layer_bind_group, &[i as u32 * stride]);
            Self::draw_layer(&mut pass, &this.pipelines.opaque, l);
            if l.clip && this.clip.enabled {
                Self::draw_layer(&mut pass, &this.pipelines.cut, l);
            }
        }
        // 2. glyphs
        for (j, g) in this.glyph_sets.iter().enumerate() {
            if !g.visible {
                continue;
            }
            pass.set_bind_group(
                1,
                &this.layer_bind_group,
                &[(this.layers.len() + j) as u32 * stride],
            );
            if let Some(inst) = g.spheres.slice() {
                Self::draw_unit(
                    &mut pass,
                    &this.pipelines.sphere,
                    &this.unit_sphere,
                    inst,
                    g.spheres.count,
                );
            }
            if let Some(inst) = g.tubes.slice() {
                Self::draw_unit(
                    &mut pass,
                    &this.pipelines.tube,
                    &this.unit_tube,
                    inst,
                    g.tubes.count,
                );
            }
        }
        // 3. translucent layers, far to near, back faces first
        for &(_, i) in &this.sort_scratch {
            let l = &this.layers[i];
            pass.set_bind_group(1, &this.layer_bind_group, &[i as u32 * stride]);
            Self::draw_layer(&mut pass, &this.pipelines.transparent_back, l);
            Self::draw_layer(&mut pass, &this.pipelines.transparent_front, l);
        }
    }

    fn draw_layer(pass: &mut wgpu::RenderPass<'_>, pipeline: &wgpu::RenderPipeline, l: &Layer) {
        pass.set_pipeline(pipeline);
        pass.set_vertex_buffer(0, l.mesh.vertices.slice(..));
        pass.set_vertex_buffer(1, l.mesh.scalars.slice(..));
        pass.set_index_buffer(l.mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..l.mesh.index_count, 0, 0..1);
    }

    fn draw_unit(
        pass: &mut wgpu::RenderPass<'_>,
        pipeline: &wgpu::RenderPipeline,
        unit: &UnitMesh,
        instances: wgpu::BufferSlice<'_>,
        count: u32,
    ) {
        pass.set_pipeline(pipeline);
        pass.set_vertex_buffer(0, unit.vertices.slice(..));
        pass.set_vertex_buffer(1, instances);
        pass.set_index_buffer(unit.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..unit.index_count, 0, 0..count);
    }

    /// Blit the offscreen texture into the current (GUI) render pass; the caller has already
    /// set the viewport/scissor to the widget rectangle.
    pub fn blit(&self, pass: &mut wgpu::RenderPass<'_>) {
        if let Some(bg) = &self.blit_bind_group {
            pass.set_pipeline(&self.blit_pipeline);
            pass.set_bind_group(0, bg, &[]);
            pass.draw(0..3, 0..1);
        }
    }

    /// Size in physical pixels of the last rendered frame, `(0, 0)` before the first.
    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        self.targets.as_ref().map_or((0, 0), |t| t.size)
    }

    fn aspect(&self) -> f32 {
        let (w, h) = self.size();
        if h == 0 { 1.0 } else { w as f32 / h as f32 }
    }

    // ---- screenshots ---------------------------------------------------------------------

    /// Read back the last rendered frame as tightly packed sRGB RGBA8, with its size. `None`
    /// before the first frame. Blocks until the GPU is idle.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn capture_rgba(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Option<(u32, u32, Vec<u8>)> {
        let t = self.targets.as_ref()?;
        Some((
            t.size.0,
            t.size.1,
            crate::capture::read_rgba8(device, queue, &t.color),
        ))
    }

    /// The last rendered frame as PNG bytes (empty before the first frame). Rows are copied
    /// through a 256-byte-aligned staging buffer and unpadded.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn capture_png(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<u8> {
        self.capture_rgba(device, queue)
            .map(|(w, h, px)| crate::capture::encode_png(w, h, &px))
            .unwrap_or_default()
    }

    /// Render one frame at `size` into a private encoder, submit it and return the PNG: the
    /// one-call screenshot for exports and tests.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn render_to_png(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, size: (u32, u32)) -> Vec<u8> {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("heartsim screenshot"),
        });
        self.render_offscreen(device, queue, &mut encoder, size);
        queue.submit([encoder.finish()]);
        self.capture_png(device, queue)
    }

    // ---- picking and labels --------------------------------------------------------------

    /// Unproject a widget-relative point (`u`, `v` in 0–1, `v` down) to a world ray
    /// `(origin, unit direction)`.
    #[must_use]
    pub fn pick_ray(&self, uv: (f32, f32), aspect: f32) -> (Vec3, Vec3) {
        let inv = self.camera.view_projection(aspect).inverse();
        let ndc = glam::Vec4::new(uv.0 * 2.0 - 1.0, 1.0 - uv.1 * 2.0, 1.0, 1.0);
        let far = inv * ndc;
        let far = far.truncate() / far.w;
        let eye = self.camera.eye();
        (eye, (far - eye).normalize())
    }

    /// Nearest surface under the widget-relative point `uv` among visible, pickable layers,
    /// using the aspect of the last rendered frame. Surface removed by the clip plane cannot be
    /// hit. Brute force over triangles with a per-layer bounding-box early-out.
    #[must_use]
    pub fn pick(&self, uv: (f32, f32)) -> Option<Hit> {
        let (origin, dir) = self.pick_ray(uv, self.aspect());
        let mut best: Option<Hit> = None;
        for l in self.layers.iter().filter(|l| l.is_drawn() && l.pickable) {
            let (lo, hi) = l.mesh.bounds();
            if !ray_aabb(origin, dir, lo, hi) {
                continue;
            }
            let cpu = l.mesh.cpu();
            let clipped = l.clip && self.clip.enabled;
            for (ti, tri) in cpu.indices.as_chunks::<3>().0.iter().enumerate() {
                let p = |k: usize| Vec3::from(cpu.vertices[tri[k] as usize].position);
                let (a, b, c) = (p(0), p(1), p(2));
                let Some(t) = ray_triangle(origin, dir, a, b, c) else {
                    continue;
                };
                if best.as_ref().is_some_and(|h| h.distance <= t) {
                    continue;
                }
                let position = origin + dir * t;
                if clipped && self.clip.removes(position) {
                    continue;
                }
                let mut normal = (b - a).cross(c - a).normalize_or_zero();
                if normal.dot(dir) > 0.0 {
                    normal = -normal;
                }
                best = Some(Hit {
                    layer: l.id,
                    triangle: ti as u32,
                    position,
                    normal,
                    distance: t,
                });
            }
        }
        best
    }

    /// Project a world point to widget-relative `(u, v)` (0–1, `v` down) for the last frame's
    /// aspect; `None` behind the camera. The UI multiplies by the widget rectangle.
    #[must_use]
    pub fn project_to_screen(&self, world: Vec3) -> Option<(f32, f32)> {
        self.camera.project(world, self.aspect()).map(|(u, v, _)| (u, v))
    }

    /// Screen positions of the labels of all visible layers that have one, skipping anchors that
    /// are behind the camera or cut away. egui draws the text (so labels are localisable).
    #[must_use]
    pub fn label_anchors(&self) -> Vec<ScreenLabel> {
        let aspect = self.aspect();
        self.layers
            .iter()
            .filter(|l| l.is_drawn())
            .filter_map(|l| {
                let text = l.label.as_ref()?;
                if l.clip && self.clip.removes(l.label_anchor) {
                    return None;
                }
                let (u, v, depth) = self.camera.project(l.label_anchor, aspect)?;
                Some(ScreenLabel {
                    layer: l.id,
                    text: text.clone(),
                    u,
                    v,
                    depth,
                })
            })
            .collect()
    }
}
