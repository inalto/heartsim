//! Tetrahedral volume meshes as the open datasets ship them: legacy VTK (Rodero/Strocchi) and
//! openCARP text (`.pts/.elem/.lon`, KCL torso). Only what the bake needs: nodes, tets, element
//! tags, per-element fibres, per-node scalars, boundary extraction.

use anyhow::{Context, Result, bail};
use glam::Vec3;
use heartsim_geometry::bridge::vtk::{VtkGrid, parse_legacy_vtk};
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead as _, BufReader};
use std::path::Path;

/// A tagged tetrahedral mesh in millimetres.
#[derive(Debug, Default)]
pub struct TetMesh {
    pub points: Vec<Vec3>,
    pub tets: Vec<[u32; 4]>,
    pub tags: Vec<u16>,
    /// Per-element fibre direction (unit, or zero where the dataset has none); empty if absent.
    pub fibres: Vec<Vec3>,
    /// Per-node scalar fields by name (the UVC of the Rodero meshes).
    pub point_scalars: BTreeMap<String, Vec<f32>>,
}

impl TetMesh {
    /// Parse a legacy ASCII/binary VTK unstructured grid of tetrahedra with an integer cell tag
    /// array (`ID`), optional `fibres` cell vectors and point scalars.
    pub fn read_vtk(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let grid = parse_legacy_vtk(&bytes).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        Self::from_grid(&grid)
    }

    fn from_grid(g: &VtkGrid) -> Result<Self> {
        let points: Vec<Vec3> = g
            .points
            .iter()
            .map(|p| Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32))
            .collect();
        let mut tets = Vec::with_capacity(g.cell_count());
        for w in g.offsets.windows(2) {
            let nodes = &g.connectivity[w[0] as usize..w[1] as usize];
            if nodes.len() != 4 {
                bail!(
                    "VTK cell with {} nodes: only tetrahedra are supported",
                    nodes.len()
                );
            }
            tets.push([nodes[0], nodes[1], nodes[2], nodes[3]]);
        }
        let n = tets.len();
        let tag_field = g
            .cell_fields
            .iter()
            .find(|f| f.components == 1 && (f.name == "ID" || f.name.eq_ignore_ascii_case("tags")))
            .or_else(|| g.cell_fields.iter().find(|f| f.components == 1))
            .context("VTK file has no scalar cell tag array")?;
        if tag_field.data.len() != n {
            bail!("tag array has {} values for {n} cells", tag_field.data.len());
        }
        let tags = tag_field.data.iter().map(|v| *v as u16).collect();
        let fibres = g
            .cell_fields
            .iter()
            .find(|f| f.components == 3 && f.name.eq_ignore_ascii_case("fibres"))
            .map(|f| {
                f.data
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|v| Vec3::new(v[0] as f32, v[1] as f32, v[2] as f32))
                    .collect()
            })
            .unwrap_or_default();
        let point_scalars = g
            .point_fields
            .iter()
            .filter(|f| f.components == 1 && f.data.len() == points.len())
            .map(|f| {
                let name = f.name.trim_end_matches(".dat").to_ascii_lowercase();
                (name, f.data.iter().map(|v| *v as f32).collect())
            })
            .collect();
        Ok(Self {
            points,
            tets,
            tags,
            fibres,
            point_scalars,
        })
    }

    /// Read an openCARP text mesh `<base>.pts` + `<base>.elem` (+ `.lon` if present), scaling
    /// coordinates by `scale` (CARP files are in micrometres: `1e-3`).
    pub fn read_carp(base: &Path, scale: f32) -> Result<Self> {
        let pts_path = base.with_extension("pts");
        let elem_path = base.with_extension("elem");
        let lon_path = base.with_extension("lon");

        let mut points = Vec::new();
        {
            let f =
                std::fs::File::open(&pts_path).with_context(|| format!("opening {}", pts_path.display()))?;
            let mut lines = BufReader::with_capacity(1 << 20, f).lines();
            let n: usize = lines
                .next()
                .context("empty .pts")??
                .trim()
                .parse()
                .context(".pts: bad count")?;
            points.reserve(n);
            for line in lines {
                let line = line?;
                let mut it = line.split_ascii_whitespace();
                let (Some(x), Some(y), Some(z)) = (it.next(), it.next(), it.next()) else {
                    continue;
                };
                points.push(Vec3::new(x.parse::<f32>()?, y.parse::<f32>()?, z.parse::<f32>()?) * scale);
            }
            if points.len() != n {
                bail!("{}: {} points, header says {n}", pts_path.display(), points.len());
            }
        }

        let mut tets = Vec::new();
        let mut tags = Vec::new();
        {
            let f = std::fs::File::open(&elem_path)
                .with_context(|| format!("opening {}", elem_path.display()))?;
            let mut lines = BufReader::with_capacity(1 << 20, f).lines();
            let n: usize = lines
                .next()
                .context("empty .elem")??
                .trim()
                .parse()
                .context(".elem: bad count")?;
            tets.reserve(n);
            tags.reserve(n);
            for line in lines {
                let line = line?;
                let mut it = line.split_ascii_whitespace();
                let Some(kind) = it.next() else { continue };
                if kind != "Tt" {
                    bail!(
                        "{}: element type {kind}: only tetrahedra (Tt) are supported",
                        elem_path.display()
                    );
                }
                let mut t = [0u32; 4];
                for v in &mut t {
                    *v = it.next().context(".elem: short line")?.parse()?;
                }
                let tag: u16 = it.next().map_or(Ok(0), str::parse)?;
                tets.push(t);
                tags.push(tag);
            }
        }

        let mut fibres = Vec::new();
        if lon_path.is_file() {
            let f = std::fs::File::open(&lon_path)?;
            let mut lines = BufReader::with_capacity(1 << 20, f).lines();
            let per: usize = lines
                .next()
                .context("empty .lon")??
                .trim()
                .parse()
                .context(".lon: bad header")?;
            fibres.reserve(tets.len());
            for line in lines {
                let line = line?;
                let v: Vec<f32> = line
                    .split_ascii_whitespace()
                    .take(3)
                    .map(str::parse)
                    .collect::<Result<_, _>>()?;
                if v.len() == 3 {
                    fibres.push(Vec3::new(v[0], v[1], v[2]));
                }
                let _ = per; // the sheet direction (per == 2) is not used
            }
            if fibres.len() != tets.len() {
                eprintln!(
                    "{}: {} fibre rows for {} elements; ignoring fibres",
                    lon_path.display(),
                    fibres.len(),
                    tets.len()
                );
                fibres.clear();
            }
        }
        Ok(Self {
            points,
            tets,
            tags,
            fibres,
            point_scalars: BTreeMap::new(),
        })
    }

    #[must_use]
    pub fn centroid(&self, t: usize) -> Vec3 {
        let [a, b, c, d] = self.tets[t];
        (self.points[a as usize]
            + self.points[b as usize]
            + self.points[c as usize]
            + self.points[d as usize])
            * 0.25
    }

    /// Mean of the centroids of the tets with `tag`.
    #[must_use]
    pub fn tag_centroid(&self, tag: u16) -> Vec3 {
        let (sum, n) = (0..self.tets.len())
            .filter(|t| self.tags[*t] == tag)
            .fold((Vec3::ZERO, 0usize), |(s, n), t| (s + self.centroid(t), n + 1));
        if n == 0 { Vec3::ZERO } else { sum / n as f32 }
    }

    /// Sorted unique node indices of the tets with `tag`.
    #[must_use]
    pub fn nodes_of_tag(&self, tag: u16) -> Vec<u32> {
        let mut v: Vec<u32> = self
            .tets
            .iter()
            .zip(&self.tags)
            .filter(|(_, g)| **g == tag)
            .flat_map(|(t, _)| t.iter().copied())
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Number of nodes shared by the tets of two tags (> 0 ⇔ the structures touch).
    #[must_use]
    pub fn shared_nodes(&self, a: u16, b: u16) -> Vec<u32> {
        let na = self.nodes_of_tag(a);
        let nb = self.nodes_of_tag(b);
        let mut out = Vec::new();
        let (mut i, mut j) = (0, 0);
        while i < na.len() && j < nb.len() {
            match na[i].cmp(&nb[j]) {
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
                std::cmp::Ordering::Equal => {
                    out.push(na[i]);
                    i += 1;
                    j += 1;
                }
            }
        }
        out
    }

    /// Boundary faces of the tets selected by `keep` (faces used by exactly one selected tet),
    /// wound counter-clockwise seen from outside.
    #[must_use]
    pub fn boundary_faces(&self, keep: impl Fn(usize) -> bool) -> Vec<[u32; 3]> {
        let mut seen: HashMap<[u32; 3], (u32, [u32; 3])> = HashMap::new();
        for t in 0..self.tets.len() {
            if !keep(t) {
                continue;
            }
            let tet = self.tets[t];
            for i in 0..4 {
                let face = [tet[(i + 1) % 4], tet[(i + 2) % 4], tet[(i + 3) % 4]];
                let opposite = tet[i];
                let mut key = face;
                key.sort_unstable();
                let oriented = self.outward(face, opposite);
                seen.entry(key).and_modify(|e| e.0 += 1).or_insert((1, oriented));
            }
        }
        let mut out: Vec<[u32; 3]> = seen
            .into_values()
            .filter(|(n, _)| *n == 1)
            .map(|(_, f)| f)
            .collect();
        out.sort_unstable();
        out
    }

    fn outward(&self, face: [u32; 3], opposite: u32) -> [u32; 3] {
        let (a, b, c) = (
            self.points[face[0] as usize],
            self.points[face[1] as usize],
            self.points[face[2] as usize],
        );
        let n = (b - a).cross(c - a);
        if n.dot(self.points[opposite as usize] - a) > 0.0 {
            [face[0], face[2], face[1]]
        } else {
            face
        }
    }

    /// Interpolate a node scalar at a tet centroid (mean of its four nodes).
    #[must_use]
    pub fn scalar_at(&self, name: &str, t: usize) -> Option<f32> {
        let s = self.point_scalars.get(name)?;
        let [a, b, c, d] = self.tets[t];
        Some((s[a as usize] + s[b as usize] + s[c as usize] + s[d as usize]) * 0.25)
    }
}

/// Plane fitted to a point set: centroid, unit normal (smallest principal axis) and the RMS
/// in-plane radius scaled to the radius of a uniform disc of the same spread.
#[derive(Debug, Clone, Copy)]
pub struct FittedDisc {
    pub centre: Vec3,
    pub normal: Vec3,
    pub radius_mm: f32,
}

/// Fit a disc to `points` (a valve plane or a vessel inlet). `away_from` orients the normal.
#[must_use]
pub fn fit_disc(points: &[Vec3], away_from: Vec3) -> FittedDisc {
    let n = points.len().max(1) as f32;
    let centre = points.iter().copied().sum::<Vec3>() / n;
    // covariance
    let mut c = [[0.0f32; 3]; 3];
    for p in points {
        let q = *p - centre;
        for i in 0..3 {
            for j in 0..3 {
                c[i][j] += q[i] * q[j] / n;
            }
        }
    }
    let normal = smallest_eigenvector(c);
    let normal = if normal.dot(centre - away_from) < 0.0 {
        -normal
    } else {
        normal
    };
    let rms: f32 = points
        .iter()
        .map(|p| {
            let q = *p - centre;
            (q - normal * q.dot(normal)).length_squared()
        })
        .sum::<f32>()
        / n;
    FittedDisc {
        centre,
        normal,
        // a uniform disc of radius R has mean squared radius R²/2
        radius_mm: (2.0 * rms).sqrt(),
    }
}

/// Eigenvector of the smallest eigenvalue of a symmetric 3×3 matrix (power iteration on the
/// inverse-shifted matrix; the matrices here are tiny and well conditioned).
fn smallest_eigenvector(c: [[f32; 3]; 3]) -> Vec3 {
    let m = glam::Mat3::from_cols_array_2d(&c);
    // shift so that the smallest eigenvalue becomes the largest in magnitude
    let trace = c[0][0] + c[1][1] + c[2][2];
    let shifted = glam::Mat3::IDENTITY * (trace + 1e-6) - m;
    let mut v = Vec3::new(0.3, 0.5, 0.8).normalize();
    for _ in 0..200 {
        let w = shifted * v;
        let l = w.length();
        if l < 1e-12 {
            break;
        }
        v = w / l;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_faces_point_outward_for_either_node_order() {
        for order in [[0u32, 1, 2, 3], [0, 2, 1, 3]] {
            let mesh = TetMesh {
                points: vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
                tets: vec![order],
                tags: vec![1],
                fibres: Vec::new(),
                point_scalars: BTreeMap::new(),
            };
            let faces = mesh.boundary_faces(|_| true);
            assert_eq!(faces.len(), 4);
            let volume: f32 = faces
                .iter()
                .map(|f| {
                    let (a, b, c) = (
                        mesh.points[f[0] as usize],
                        mesh.points[f[1] as usize],
                        mesh.points[f[2] as usize],
                    );
                    a.dot(b.cross(c)) / 6.0
                })
                .sum();
            assert!((volume - 1.0 / 6.0).abs() < 1e-6, "{order:?}: volume {volume}");
        }
    }

    #[test]
    fn disc_fit_recovers_centre_normal_and_radius() {
        let pts: Vec<Vec3> = (0..400)
            .map(|i| {
                let a = i as f32 * 0.7;
                let r = 10.0 * ((i % 20) as f32 / 20.0).sqrt();
                Vec3::new(5.0 + r * a.cos(), 2.0, -3.0 + r * a.sin())
            })
            .collect();
        let d = fit_disc(&pts, Vec3::new(5.0, -10.0, -3.0));
        assert!(
            d.centre.distance(Vec3::new(5.0, 2.0, -3.0)) < 1.0,
            "{:?}",
            d.centre
        );
        assert!(
            d.normal.dot(Vec3::Y) > 0.99,
            "normal {:?} should point away from the chamber",
            d.normal
        );
        assert!((d.radius_mm - 10.0).abs() < 1.5, "radius {}", d.radius_mm);
    }
}
