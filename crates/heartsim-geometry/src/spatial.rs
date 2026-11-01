//! Uniform hash grid for k-nearest-neighbour queries on a static point set.
//!
//! Used for the cell-cloud neighbourhood graph, the vertex→cell hoods and landmark lookup. The
//! grid is exact (not approximate): the search widens shell by shell until the k-th best candidate
//! is provably closer than any unvisited bin, so results do not depend on the bin size — only the
//! speed does.

use glam::Vec3;
use std::collections::HashMap;

/// A uniform grid over a borrowed point slice. Build once, query many times.
#[derive(Debug, Clone)]
pub struct HashGrid<'a> {
    points: &'a [Vec3],
    lo: Vec3,
    hi: Vec3,
    cell_mm: f32,
    bins: HashMap<(i32, i32, i32), Vec<u32>>,
}

impl<'a> HashGrid<'a> {
    /// Grid with an explicit bin size (mm). A bin size close to the typical point spacing is fastest.
    #[must_use]
    pub fn new(points: &'a [Vec3], cell_mm: f32) -> Self {
        let (lo, hi) = points.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(l, h), p| (l.min(*p), h.max(*p)),
        );
        let cell_mm = cell_mm.max(1e-3);
        let mut bins: HashMap<(i32, i32, i32), Vec<u32>> = HashMap::with_capacity(points.len() / 2 + 1);
        for (i, p) in points.iter().enumerate() {
            bins.entry(Self::key_for(lo, cell_mm, *p))
                .or_default()
                .push(i as u32);
        }
        Self {
            points,
            lo,
            hi,
            cell_mm,
            bins,
        }
    }

    /// Grid whose bin size is 1.5× the mean spacing estimated from the bounding-box volume.
    #[must_use]
    pub fn with_auto_cell(points: &'a [Vec3]) -> Self {
        let (lo, hi) = points.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(l, h), p| (l.min(*p), h.max(*p)),
        );
        let n = points.len().max(1) as f32;
        let extent = (hi - lo).max(Vec3::splat(1e-3));
        let spacing = (extent.x * extent.y * extent.z / n).cbrt().max(1e-3) * 1.5;
        Self::new(points, spacing)
    }

    fn key_for(lo: Vec3, cell: f32, p: Vec3) -> (i32, i32, i32) {
        let q = ((p - lo) / cell).floor();
        (q.x as i32, q.y as i32, q.z as i32)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// The `k` nearest points to `p` (excluding index `exclude`), nearest first, as
    /// `(distance_mm, index)`. Returns fewer than `k` only if the set is too small.
    pub fn knn(&self, p: Vec3, k: usize, exclude: Option<u32>, out: &mut Vec<(f32, u32)>) {
        out.clear();
        if k == 0 || self.points.is_empty() {
            return;
        }
        let (kx, ky, kz) = Self::key_for(self.lo, self.cell_mm, p);
        // Beyond this radius every bin of the grid has been visited.
        let far = (p - self.lo).abs().max((p - self.hi).abs()).max_element();
        let max_radius = (far / self.cell_mm).ceil() as i32 + 1;
        let mut cand: Vec<(f32, u32)> = Vec::new();
        let mut radius = 0i32;
        loop {
            // visit only the shell at Chebyshev distance == radius
            for dx in -radius..=radius {
                for dy in -radius..=radius {
                    let on_xy_edge = dx.abs() == radius || dy.abs() == radius;
                    let mut dz = -radius;
                    while dz <= radius {
                        if let Some(bin) = self.bins.get(&(kx + dx, ky + dy, kz + dz)) {
                            for &j in bin {
                                if Some(j) != exclude {
                                    cand.push((self.points[j as usize].distance_squared(p), j));
                                }
                            }
                        }
                        // interior of the shell is skipped: jump from -radius to +radius
                        dz = if on_xy_edge || dz == radius {
                            dz + 1
                        } else {
                            radius
                        };
                    }
                }
            }
            let complete_r2 = (radius as f32 * self.cell_mm).powi(2);
            if cand.len() >= k {
                cand.select_nth_unstable_by(k - 1, |a, b| a.0.total_cmp(&b.0));
                if cand[k - 1].0 <= complete_r2 {
                    break;
                }
            }
            if radius >= max_radius {
                break;
            }
            radius += 1;
        }
        let take = k.min(cand.len());
        cand.select_nth_unstable_by(take.saturating_sub(1), |a, b| a.0.total_cmp(&b.0));
        cand.truncate(take);
        cand.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        out.extend(cand.into_iter().map(|(d2, j)| (d2.sqrt(), j)));
    }

    /// Nearest point to `p` as `(distance_mm, index)`.
    #[must_use]
    pub fn nearest(&self, p: Vec3) -> Option<(f32, u32)> {
        let mut out = Vec::with_capacity(1);
        self.knn(p, 1, None, &mut out);
        out.first().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brute(points: &[Vec3], p: Vec3, k: usize, exclude: Option<u32>) -> Vec<u32> {
        let mut v: Vec<(f32, u32)> = points
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i as u32) != exclude)
            .map(|(i, q)| (q.distance(p), i as u32))
            .collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        v.into_iter().take(k).map(|(_, i)| i).collect()
    }

    #[test]
    fn knn_matches_brute_force_on_a_lattice_with_jitter() {
        let mut pts = Vec::new();
        let mut s = 12345u32;
        let mut rnd = || {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (s >> 8) as f32 / (1u32 << 24) as f32 - 0.5
        };
        for x in 0..9 {
            for y in 0..7 {
                for z in 0..5 {
                    pts.push(Vec3::new(
                        x as f32 + 0.3 * rnd(),
                        y as f32 + 0.3 * rnd(),
                        z as f32 + 0.3 * rnd(),
                    ));
                }
            }
        }
        for cell in [0.5f32, 1.0, 2.7] {
            let g = HashGrid::new(&pts, cell);
            let mut out = Vec::new();
            for (i, p) in pts.iter().enumerate() {
                g.knn(*p, 8, Some(i as u32), &mut out);
                let got: Vec<u32> = out.iter().map(|(_, j)| *j).collect();
                assert_eq!(got, brute(&pts, *p, 8, Some(i as u32)), "cell {cell} point {i}");
            }
            // query far outside the grid still finds the true nearest
            let far = Vec3::new(40.0, -30.0, 12.0);
            g.knn(far, 3, None, &mut out);
            let got: Vec<u32> = out.iter().map(|(_, j)| *j).collect();
            assert_eq!(got, brute(&pts, far, 3, None));
        }
    }

    #[test]
    fn small_sets_return_what_exists() {
        let pts = [Vec3::ZERO, Vec3::X];
        let g = HashGrid::with_auto_cell(&pts);
        let mut out = Vec::new();
        g.knn(Vec3::Y, 8, None, &mut out);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].1, 0);
    }
}
