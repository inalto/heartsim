//! Instanced glyphs: spheres (electrodes, markers, catheter tips) and tapered tubes (heart-vector
//! arrow, vector loop, catheter bodies, pacing wires). Each instance is 48 bytes and the GPU
//! expands a shared unit mesh, so a 500-point vector loop costs one draw call and one buffer write.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use heartsim_geometry::{TriMesh, Vertex};

/// Stable handle of a [`GlyphSet`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GlyphSetId(pub u32);

/// One sphere or one (possibly tapered) tube segment.
///
/// For a sphere only `a`/`radius_a` are used. For a tube the surface runs from `a` (radius
/// `radius_a`) to `b` (radius `radius_b`); `radius_b = 0` makes a cone, which is how arrow heads
/// are drawn. Colour is linear RGB; the alpha channel is ignored (glyphs are opaque).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, PartialEq)]
pub struct GlyphInstance {
    /// Sphere centre or tube start (mm).
    pub a: [f32; 3],
    /// Sphere radius or tube radius at `a` (mm).
    pub radius_a: f32,
    /// Tube end (mm); ignored for spheres.
    pub b: [f32; 3],
    /// Tube radius at `b` (mm); ignored for spheres.
    pub radius_b: f32,
    /// Linear RGBA colour.
    pub color: [f32; 4],
}

impl GlyphInstance {
    /// A sphere of `radius` mm at `center`.
    #[must_use]
    pub fn sphere(center: Vec3, radius: f32, color: [f32; 4]) -> Self {
        Self {
            a: center.to_array(),
            radius_a: radius,
            b: center.to_array(),
            radius_b: radius,
            color,
        }
    }

    /// A cylinder from `a` to `b`.
    #[must_use]
    pub fn tube(a: Vec3, b: Vec3, radius: f32, color: [f32; 4]) -> Self {
        Self::cone(a, b, radius, radius, color)
    }

    /// A tapered tube from `a` (radius `ra`) to `b` (radius `rb`).
    #[must_use]
    pub fn cone(a: Vec3, b: Vec3, ra: f32, rb: f32, color: [f32; 4]) -> Self {
        Self {
            a: a.to_array(),
            radius_a: ra,
            b: b.to_array(),
            radius_b: rb,
            color,
        }
    }
}

/// Tubes and joint spheres for a polyline (catheter body, vector loop). The spheres round off the
/// joints and the two ends so consecutive segments join without gaps or hollow ends.
#[must_use]
pub fn polyline(points: &[Vec3], radius: f32, color: [f32; 4]) -> (Vec<GlyphInstance>, Vec<GlyphInstance>) {
    let tubes = points
        .windows(2)
        .filter(|w| w[0].distance_squared(w[1]) > 1e-12)
        .map(|w| GlyphInstance::tube(w[0], w[1], radius, color))
        .collect();
    let joints = points
        .iter()
        .map(|&p| GlyphInstance::sphere(p, radius, color))
        .collect();
    (tubes, joints)
}

/// Shaft plus cone head for a vector arrow (e.g. the instantaneous cardiac dipole). The head
/// occupies the last `head_fraction` of the length (clamped to at most the whole arrow) and is
/// 2.2× the shaft radius wide.
#[must_use]
pub fn arrow(
    origin: Vec3,
    vector: Vec3,
    shaft_radius: f32,
    head_fraction: f32,
    color: [f32; 4],
) -> Vec<GlyphInstance> {
    let len = vector.length();
    if len < 1e-9 {
        return Vec::new();
    }
    let tip = origin + vector;
    let head_len = len * head_fraction.clamp(0.0, 1.0);
    let neck = tip - vector / len * head_len;
    vec![
        GlyphInstance::tube(origin, neck, shaft_radius, color),
        GlyphInstance::cone(neck, tip, shaft_radius * 2.2, 0.0, color),
    ]
}

/// GPU instance storage that grows geometrically and is rewritten in place.
#[derive(Debug, Default)]
pub(crate) struct InstanceBuffer {
    buffer: Option<wgpu::Buffer>,
    capacity: usize,
    pub(crate) count: u32,
}

impl InstanceBuffer {
    pub(crate) fn set(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, items: &[GlyphInstance]) {
        if items.len() > self.capacity {
            self.capacity = items.len().next_power_of_two().max(16);
            self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("heartsim glyph instances"),
                size: (self.capacity * size_of::<GlyphInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if let (Some(buf), false) = (&self.buffer, items.is_empty()) {
            queue.write_buffer(buf, 0, bytemuck::cast_slice(items));
        }
        self.count = items.len() as u32;
    }

    pub(crate) fn slice(&self) -> Option<wgpu::BufferSlice<'_>> {
        (self.count > 0)
            .then(|| self.buffer.as_ref().map(|b| b.slice(..)))
            .flatten()
    }
}

/// A named group of glyphs sharing visibility and clipping, drawn after the opaque layers.
#[derive(Debug)]
pub struct GlyphSet {
    /// Handle that identifies this set in the scene.
    pub id: GlyphSetId,
    /// Human-readable name.
    pub name: String,
    /// Hidden sets are not drawn.
    pub visible: bool,
    /// Whether the scene clip plane cuts these glyphs.
    pub clip: bool,
    pub(crate) spheres: InstanceBuffer,
    pub(crate) tubes: InstanceBuffer,
}

impl GlyphSet {
    pub(crate) fn new(id: GlyphSetId, name: String) -> Self {
        Self {
            id,
            name,
            visible: true,
            clip: false,
            spheres: InstanceBuffer::default(),
            tubes: InstanceBuffer::default(),
        }
    }

    /// Number of spheres currently uploaded.
    #[must_use]
    pub fn sphere_count(&self) -> usize {
        self.spheres.count as usize
    }

    /// Number of tube segments currently uploaded.
    #[must_use]
    pub fn tube_count(&self) -> usize {
        self.tubes.count as usize
    }
}

/// Number of sides of the unit cylinder.
const TUBE_SIDES: u32 = 20;

/// Unit cylinder along +Z (`z` in 0..=1, radius 1) with end caps. The vertex shader scales and
/// orients it per instance; side normals are `(cos, sin, 0)` and cap normals `(0, 0, ∓1)`.
#[must_use]
pub(crate) fn unit_tube() -> TriMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let ring = |z: f32, nz: f32, side: bool| {
        (0..TUBE_SIDES).map(move |i| {
            let th = std::f32::consts::TAU * i as f32 / TUBE_SIDES as f32;
            let (s, c) = th.sin_cos();
            Vertex {
                position: [c, s, z],
                normal: if side { [c, s, 0.0] } else { [0.0, 0.0, nz] },
            }
        })
    };
    // Side wall: ring at z = 0 then ring at z = 1.
    vertices.extend(ring(0.0, 0.0, true));
    vertices.extend(ring(1.0, 0.0, true));
    for i in 0..TUBE_SIDES {
        let j = (i + 1) % TUBE_SIDES;
        let (a0, a1, b0, b1) = (i, j, TUBE_SIDES + i, TUBE_SIDES + j);
        indices.extend_from_slice(&[a0, a1, b1, a0, b1, b0]);
    }
    // Caps: duplicate rings with cap normals plus a centre vertex each.
    for (z, nz) in [(0.0, -1.0), (1.0, 1.0)] {
        let base = vertices.len() as u32;
        vertices.extend(ring(z, nz, false));
        let centre = vertices.len() as u32;
        vertices.push(Vertex {
            position: [0.0, 0.0, z],
            normal: [0.0, 0.0, nz],
        });
        for i in 0..TUBE_SIDES {
            let j = (i + 1) % TUBE_SIDES;
            if nz > 0.0 {
                indices.extend_from_slice(&[centre, base + i, base + j]);
            } else {
                indices.extend_from_slice(&[centre, base + j, base + i]);
            }
        }
    }
    TriMesh { vertices, indices }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_is_48_bytes() {
        assert_eq!(size_of::<GlyphInstance>(), 48);
    }

    #[test]
    fn unit_tube_is_closed_and_outward_facing() {
        let m = unit_tube();
        // Every triangle's geometric normal agrees with the average of its vertex normals.
        for t in m.indices.as_chunks::<3>().0 {
            let p: Vec<Vec3> = t
                .iter()
                .map(|&i| Vec3::from(m.vertices[i as usize].position))
                .collect();
            let n: Vec3 = t.iter().map(|&i| Vec3::from(m.vertices[i as usize].normal)).sum();
            let g = (p[1] - p[0]).cross(p[2] - p[0]);
            assert!(g.dot(n) > 0.0, "inward-facing triangle {t:?}");
        }
    }

    #[test]
    fn arrow_ends_at_tip_and_degenerate_is_empty() {
        let a = arrow(Vec3::ZERO, Vec3::new(0.0, 0.0, 10.0), 0.5, 0.25, [1.0; 4]);
        assert_eq!(a.len(), 2);
        assert_eq!(a[1].b, [0.0, 0.0, 10.0]);
        assert!((a[0].b[2] - 7.5).abs() < 1e-5);
        assert_eq!(arrow(Vec3::ZERO, Vec3::ZERO, 0.5, 0.25, [1.0; 4]), vec![]);
    }

    #[test]
    fn polyline_skips_zero_length_segments() {
        let pts = [Vec3::ZERO, Vec3::ZERO, Vec3::X, Vec3::X * 2.0];
        let (tubes, joints) = polyline(&pts, 0.1, [1.0; 4]);
        assert_eq!(tubes.len(), 2);
        assert_eq!(joints.len(), 4);
    }
}
