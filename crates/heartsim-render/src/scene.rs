use crate::camera::OrbitCamera;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use heartsim_geometry::TriMesh;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SceneUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub light_dir: [f32; 4],
    pub base_color: [f32; 4],
    pub scalar_range: [f32; 4],
}

impl Default for SceneUniforms {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            light_dir: [0.4, 0.8, 1.0, 0.0],
            base_color: [0.75, 0.25, 0.25, 1.0],
            scalar_range: [0.0, 1.0, 1.0, 0.0],
        }
    }
}

/// Offscreen renderer for one mesh with a per-vertex scalar. Multi-mesh scenes (chambers,
/// torso, electrodes, catheters) extend this by holding several `MeshGpu` (brief B6).
#[derive(Debug)]
pub struct SceneRenderer {
    pipeline: wgpu::RenderPipeline,
    blit_pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    blit_layout: wgpu::BindGroupLayout,
    blit_bind_group: Option<wgpu::BindGroup>,
    sampler: wgpu::Sampler,
    color: Option<wgpu::Texture>,
    depth: Option<wgpu::Texture>,
    size: (u32, u32),
    mesh: Option<MeshGpu>,
    pub camera: OrbitCamera,
    pub uniform_data: SceneUniforms,
    pub clear_color: wgpu::Color,
}

#[derive(Debug)]
struct MeshGpu {
    vertices: wgpu::Buffer,
    scalars: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    vertex_count: u32,
}

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

impl SceneRenderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target_format: wgpu::TextureFormat) -> Self {
        let scene_shader = device.create_shader_module(wgpu::include_wgsl!("shaders/scene.wgsl"));
        let blit_shader = device.create_shader_module(wgpu::include_wgsl!("shaders/blit.wgsl"));

        let lut = crate::lut::voltage_lut();
        let lut_tex = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("heartsim lut"),
                size: wgpu::Extent3d {
                    width: 256,
                    height: 1,
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
            bytemuck::cast_slice(&lut),
        );
        let lut_view = lut_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("heartsim sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let uniform_data = SceneUniforms::default();
        let uniforms = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("heartsim uniforms"),
            contents: bytemuck::bytes_of(&uniform_data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("heartsim scene layout"),
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
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("heartsim scene bind group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms.as_entire_binding(),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("heartsim scene pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: 24,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
        };
        let scalar_layout = wgpu::VertexBufferLayout {
            array_stride: 4,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![2 => Float32],
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("heartsim scene pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &scene_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(vertex_layout), Some(scalar_layout)],
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &scene_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: COLOR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("heartsim blit pipeline layout"),
            bind_group_layouts: &[Some(&blit_layout)],
            immediate_size: 0,
        });
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("heartsim blit pipeline"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blit_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
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

        Self {
            pipeline,
            blit_pipeline,
            uniforms,
            bind_group,
            blit_layout,
            blit_bind_group: None,
            sampler,
            color: None,
            depth: None,
            size: (0, 0),
            mesh: None,
            camera: OrbitCamera::default(),
            uniform_data,
            clear_color: wgpu::Color {
                r: 0.09,
                g: 0.1,
                b: 0.12,
                a: 1.0,
            },
        }
    }

    /// Upload a mesh (replaces the previous one). Scalars start at zero.
    pub fn set_mesh(&mut self, device: &wgpu::Device, mesh: &TriMesh) {
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("heartsim mesh vertices"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let scalars = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("heartsim mesh scalars"),
            contents: bytemuck::cast_slice(&vec![0f32; mesh.vertices.len()]),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("heartsim mesh indices"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.mesh = Some(MeshGpu {
            vertices,
            scalars,
            indices,
            index_count: mesh.indices.len() as u32,
            vertex_count: mesh.vertices.len() as u32,
        });
        let (lo, hi) = mesh.bounds();
        self.camera.target = (lo + hi) * 0.5;
        self.camera.distance = (hi - lo).length() * 1.4;
    }

    /// Update the per-vertex scalar buffer (length must equal the mesh vertex count).
    pub fn set_scalars(&self, queue: &wgpu::Queue, scalars: &[f32]) {
        if let Some(m) = &self.mesh {
            debug_assert_eq!(scalars.len() as u32, m.vertex_count);
            queue.write_buffer(&m.scalars, 0, bytemuck::cast_slice(scalars));
        }
    }

    fn ensure_targets(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        if self.size == size && self.color.is_some() {
            return;
        }
        let size = (size.0.max(1), size.1.max(1));
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
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("heartsim offscreen depth"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = color.create_view(&wgpu::TextureViewDescriptor::default());
        self.blit_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("heartsim blit bind group"),
            layout: &self.blit_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        }));
        self.color = Some(color);
        self.depth = Some(depth);
        self.size = size;
    }

    /// Render the scene into the offscreen texture at `size` physical pixels.
    pub fn render_offscreen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        size: (u32, u32),
    ) {
        self.ensure_targets(device, size);
        let aspect = size.0 as f32 / size.1.max(1) as f32;
        self.uniform_data.view_proj = self.camera.view_projection(aspect).to_cols_array_2d();
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&self.uniform_data));
        let color_view = self
            .color
            .as_ref()
            .expect("targets")
            .create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = self
            .depth
            .as_ref()
            .expect("targets")
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("heartsim scene pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
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
        if let Some(m) = &self.mesh {
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, m.vertices.slice(..));
            pass.set_vertex_buffer(1, m.scalars.slice(..));
            pass.set_index_buffer(m.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..m.index_count, 0, 0..1);
        }
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

    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Helper used by picking: unproject a widget-relative point to a world ray.
    #[must_use]
    pub fn pick_ray(&self, uv: (f32, f32), aspect: f32) -> (Vec3, Vec3) {
        let inv = self.camera.view_projection(aspect).inverse();
        let ndc = glam::Vec4::new(uv.0 * 2.0 - 1.0, 1.0 - uv.1 * 2.0, 1.0, 1.0);
        let far = inv * ndc;
        let far = far.truncate() / far.w;
        let eye = self.camera.eye();
        (eye, (far - eye).normalize())
    }
}
