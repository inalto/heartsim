//! Scene layers: one GPU mesh plus the display state that the UI toggles (visibility, opacity,
//! colour map, clipping). Geometry is uploaded once; only the per-vertex scalar buffer changes
//! per frame.

use crate::lut::LutId;
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use heartsim_geometry::TriMesh;
use wgpu::util::DeviceExt;

/// Stable handle of a [`Layer`]; survives removal of other layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerId(pub u32);

/// How a layer is coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarMode {
    /// Flat [`Layer::base_color`]; the scalar buffer is ignored.
    None,
    /// The per-vertex scalar, normalised through [`Layer::scalar_min`]..[`Layer::scalar_max`],
    /// is looked up in this colour map.
    Lut(LutId),
}

/// An iso-line of the scalar drawn over the surface — the "outline of the wavefront" when the
/// scalar is V_m and `level` is the activation threshold (about −30 mV, or 0.5 normalised).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contour {
    /// Scalar value (same units as the layer's scalar window) at which the line is drawn.
    pub level: f32,
    /// Line width in screen pixels.
    pub width_px: f32,
    /// Linear RGB colour of the line.
    pub color: [f32; 3],
}

impl Default for Contour {
    fn default() -> Self {
        Self {
            level: 0.5,
            width_px: 2.0,
            color: [0.02, 0.02, 0.02],
        }
    }
}

/// Convert an sRGB-encoded colour channel (0–1) to linear light.
#[must_use]
pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Convert an sRGB-encoded RGBA colour (as a colour picker or a CSS value gives it) to the linear
/// RGBA that [`Layer::base_color`] and the glyph instances expect. Alpha is unchanged.
#[must_use]
pub fn srgb_color(c: [f32; 4]) -> [f32; 4] {
    [
        srgb_to_linear(c[0]),
        srgb_to_linear(c[1]),
        srgb_to_linear(c[2]),
        c[3],
    ]
}

/// Immutable GPU geometry of one layer, plus the CPU copy that picking needs.
#[derive(Debug)]
pub struct MeshGpu {
    pub(crate) vertices: wgpu::Buffer,
    pub(crate) scalars: wgpu::Buffer,
    pub(crate) indices: wgpu::Buffer,
    pub(crate) index_count: u32,
    vertex_count: u32,
    cpu: TriMesh,
    bounds: (Vec3, Vec3),
}

impl MeshGpu {
    /// Upload `mesh`. The scalar buffer is created zeroed.
    pub fn new(device: &wgpu::Device, mesh: &TriMesh) -> Self {
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
        Self {
            vertices,
            scalars,
            indices,
            index_count: mesh.indices.len() as u32,
            vertex_count: mesh.vertices.len() as u32,
            cpu: mesh.clone(),
            bounds: mesh.bounds(),
        }
    }

    /// Number of vertices, i.e. the required length of a scalar upload.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.vertex_count as usize
    }

    /// Axis-aligned bounds (min, max) in world space.
    #[must_use]
    pub fn bounds(&self) -> (Vec3, Vec3) {
        self.bounds
    }

    /// Centre of the bounding box; the sort key for transparent layers and the default label
    /// anchor.
    #[must_use]
    pub fn centroid(&self) -> Vec3 {
        (self.bounds.0 + self.bounds.1) * 0.5
    }

    /// CPU copy of the geometry (kept for picking).
    #[must_use]
    pub fn cpu(&self) -> &TriMesh {
        &self.cpu
    }

    /// Overwrite the per-vertex scalars; `values.len()` must equal [`Self::vertex_count`].
    pub fn write_scalars(&self, queue: &wgpu::Queue, values: &[f32]) {
        debug_assert_eq!(values.len(), self.vertex_count());
        let n = values.len().min(self.vertex_count());
        queue.write_buffer(&self.scalars, 0, bytemuck::cast_slice(&values[..n]));
    }
}

/// Everything needed to create a [`Layer`]; `..LayerDesc::new(..)` fills the rest.
#[derive(Debug, Clone)]
pub struct LayerDesc<'a> {
    /// Name used by the UI (layer list, label text).
    pub name: String,
    /// Geometry, in heart coordinates (mm).
    pub mesh: &'a TriMesh,
    /// Initial opacity.
    pub alpha: f32,
    /// Initial flat colour (linear RGB).
    pub base_color: [f32; 3],
    /// Initial colouring mode.
    pub scalar_mode: ScalarMode,
}

impl<'a> LayerDesc<'a> {
    /// An opaque, flat-grey, visible layer.
    #[must_use]
    pub fn new(name: impl Into<String>, mesh: &'a TriMesh) -> Self {
        Self {
            name: name.into(),
            mesh,
            alpha: 1.0,
            base_color: [0.6, 0.6, 0.6],
            scalar_mode: ScalarMode::None,
        }
    }
}

/// One drawable mesh with its display state. The public fields are meant to be edited directly by
/// the UI between frames; they cost nothing until the next [`crate::SceneRenderer::render_offscreen`].
#[derive(Debug)]
pub struct Layer {
    /// Handle that identifies this layer in the scene.
    pub id: LayerId,
    /// Human-readable name.
    pub name: String,
    /// GPU geometry and scalar buffer.
    pub mesh: MeshGpu,
    /// Hidden layers are neither drawn nor picked.
    pub visible: bool,
    /// Opacity in 0–1. Below 1 the layer is drawn after all opaque geometry, sorted back to front
    /// by centroid, without writing depth.
    pub alpha: f32,
    /// Flat colour (linear RGB) used when [`Self::scalar_mode`] is [`ScalarMode::None`].
    pub base_color: [f32; 3],
    /// Flat colour or colour-mapped scalar.
    pub scalar_mode: ScalarMode,
    /// Scalar value that maps to the first LUT entry.
    pub scalar_min: f32,
    /// Scalar value that maps to the last LUT entry.
    pub scalar_max: f32,
    /// Whether the scene clip plane cuts this layer. Cut surfaces show the interior in the flat
    /// cut colour.
    pub clip: bool,
    /// Optional iso-line of the scalar.
    pub contour: Option<Contour>,
    /// Whether [`crate::SceneRenderer::pick`] can hit this layer. Translucent context layers such
    /// as the torso usually set this to `false` so clicks reach the heart behind them.
    pub pickable: bool,
    /// Text for the anatomical label drawn by the UI, if any.
    pub label: Option<String>,
    /// World-space anchor of the label; defaults to the mesh centroid.
    pub label_anchor: Vec3,
}

impl Layer {
    pub(crate) fn new(id: LayerId, device: &wgpu::Device, desc: &LayerDesc<'_>) -> Self {
        let mesh = MeshGpu::new(device, desc.mesh);
        let label_anchor = mesh.centroid();
        Self {
            id,
            name: desc.name.clone(),
            mesh,
            visible: true,
            alpha: desc.alpha,
            base_color: desc.base_color,
            scalar_mode: desc.scalar_mode,
            scalar_min: 0.0,
            scalar_max: 1.0,
            clip: false,
            contour: None,
            pickable: true,
            label: None,
            label_anchor,
        }
    }

    /// Does this layer go through the sorted, blended pass?
    #[must_use]
    pub fn is_transparent(&self) -> bool {
        self.alpha < 0.999
    }

    /// Should the layer be drawn at all this frame?
    #[must_use]
    pub fn is_drawn(&self) -> bool {
        self.visible && self.alpha > 0.001
    }
}

/// Per-layer (and per-glyph-set) shader constants, one dynamic-offset slot each.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LayerUniform {
    /// Linear RGB base colour, alpha.
    pub base_color: [f32; 4],
    /// Scalar min, scalar max, colour-map enabled (0/1), LUT row.
    pub scalar: [f32; 4],
    /// Clip enabled (0/1), contour level, contour width in px (0 = off), unused.
    pub flags: [f32; 4],
    /// Linear RGB contour colour, unused.
    pub contour_color: [f32; 4],
}

impl LayerUniform {
    pub(crate) fn for_layer(l: &Layer) -> Self {
        let (mapped, row) = match l.scalar_mode {
            ScalarMode::None => (0.0, 0.0),
            ScalarMode::Lut(id) => (1.0, f32::from(id as u8)),
        };
        let c = l.contour.unwrap_or_default();
        Self {
            base_color: [l.base_color[0], l.base_color[1], l.base_color[2], l.alpha],
            scalar: [l.scalar_min, l.scalar_max, mapped, row],
            flags: [
                f32::from(u8::from(l.clip)),
                c.level,
                if l.contour.is_some() { c.width_px } else { 0.0 },
                0.0,
            ],
            contour_color: [c.color[0], c.color[1], c.color[2], 0.0],
        }
    }
}
