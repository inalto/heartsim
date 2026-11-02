//! Where timing events start wavefronts on the mesh.
//!
//! The timing engine says *when* a structure fires; a [`SeedMap`] says *which cells* that means.
//! Everything is resolved once, from the pack's landmarks and PMJ seed sets, so that ingesting an
//! event is a table lookup.

use super::graph::CellKind;
use crate::timing::Side;
use glam::Vec3;
use heartsim_core::{Region, StimSite};
use heartsim_geometry::CellCloud;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Radius of the PMJ patch derived around a fascicle terminal when the pack has no seed set, mm.
const PMJ_PATCH_RADIUS_MM: f32 = 15.0;
/// Endocardial depth limit of a derived PMJ patch.
const PMJ_PATCH_DEPTH: f32 = 0.3;
/// Apex–base coordinate above which a cell counts as basal (accessory-pathway insertion).
const BASAL_LIMIT: f32 = 0.85;

/// Cells seeded by each kind of timing event. Empty sets simply start no front.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SeedMap {
    /// Sinus discharge: the SA-node cells (they exit into the atrial myocardium).
    pub sinus: Vec<u32>,
    /// Atrial cells around the AV node: retrograde (nodal echo, VA conduction) atrial
    /// activation starts here, and the atrial front reaching them is the mesh-level nodal input.
    pub av_input: Vec<u32>,
    /// Right bundle branch PMJ cells.
    pub rbb: Vec<u32>,
    /// Left anterior fascicle PMJ cells.
    pub laf: Vec<u32>,
    /// Left posterior fascicle PMJ cells.
    pub lpf: Vec<u32>,
    /// Default ectopic / stimulus sites per chamber (`ra_lateral`, `la_roof`, `rv_apex`,
    /// `lv_lateral`).
    pub right_atrium: Vec<u32>,
    /// See `right_atrium`.
    pub left_atrium: Vec<u32>,
    /// See `right_atrium`.
    pub right_ventricle: Vec<u32>,
    /// See `right_atrium`.
    pub left_ventricle: Vec<u32>,
    /// Ventricular escape focus (high septum, fascicular origin).
    pub ventricular_escape: Vec<u32>,
    /// Pacemaker lead tips.
    pub atrial_lead: Vec<u32>,
    /// Ventricular lead tip (RV apex by default).
    pub ventricular_lead: Vec<u32>,
    /// Accessory-pathway insertions on the AV ring, indexed `[Left, Right]`.
    pub ap_atrial: [Vec<u32>; 2],
    /// Ventricular insertions (basal LV lateral / RV free wall), `[Left, Right]`.
    pub ap_ventricular: [Vec<u32>; 2],
    /// Per-focus sites (`Origin::Focus(id)` → `foci[id]`); `None` = chamber default.
    pub foci: Vec<Option<Vec<u32>>>,
}

/// Index of `side` in the `[Left, Right]` arrays.
#[must_use]
pub fn side_index(side: Side) -> usize {
    match side {
        Side::Left => 0,
        Side::Right => 1,
    }
}

/// Resolve a stimulus site to a cell: landmark name, explicit index, or nearest cell to a point.
#[must_use]
pub fn resolve_site(cloud: &CellCloud, site: &StimSite) -> Option<u32> {
    match site {
        StimSite::Landmark(name) => cloud.landmarks.get(name).copied(),
        StimSite::Cell(i) => ((*i as usize) < cloud.len()).then_some(*i),
        StimSite::Point(p) => cloud.nearest(Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32)),
    }
}

fn landmark(cloud: &CellCloud, name: &str) -> Vec<u32> {
    cloud.landmarks.get(name).map(|&i| vec![i]).unwrap_or_default()
}

fn nearest_where(cloud: &CellCloud, p: Vec3, pred: impl Fn(usize) -> bool) -> Option<u32> {
    let mut best: Option<(f32, u32)> = None;
    for (i, q) in cloud.positions.iter().enumerate() {
        if pred(i) {
            let d = q.distance_squared(p);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, i as u32));
            }
        }
    }
    best.map(|b| b.1)
}

impl SeedMap {
    /// Derive every seed set from the cloud's landmarks and the pack's named sets (`rbb`, `laf`,
    /// `lpf`; see `docs/anatomy-pack.md`). Missing PMJ sets are rebuilt as the endocardial
    /// cells within 15 mm of the fascicle's landmark.
    #[must_use]
    pub fn derive(
        cloud: &CellCloud,
        sets: &BTreeMap<String, Vec<u32>>,
        kinds: &[CellKind],
        apex_base: &[f32],
        av_input_radius_mm: f32,
    ) -> Self {
        let n = cloud.len();
        let valid = |v: &Vec<u32>| {
            v.iter()
                .copied()
                .filter(|&i| (i as usize) < n)
                .collect::<Vec<_>>()
        };

        let mut sinus: Vec<u32> = (0..n)
            .filter(|&i| cloud.regions[i] == Region::SinoatrialNode)
            .map(|i| i as u32)
            .collect();
        if sinus.is_empty() {
            sinus = landmark(cloud, "sa_node");
        }

        let nodal: Vec<Vec3> = (0..n)
            .filter(|&i| {
                matches!(
                    cloud.regions[i],
                    Region::AvNodeCompact | Region::AvNodeFastPathway | Region::AvNodeSlowPathway
                )
            })
            .map(|i| cloud.positions[i])
            .collect();
        let nodal = if nodal.is_empty() {
            landmark(cloud, "av_node")
                .iter()
                .map(|&i| cloud.positions[i as usize])
                .collect()
        } else {
            nodal
        };
        let r2 = av_input_radius_mm * av_input_radius_mm;
        let av_input: Vec<u32> = (0..n)
            .filter(|&i| {
                kinds[i] == CellKind::Atrial
                    && nodal.iter().any(|q| q.distance_squared(cloud.positions[i]) <= r2)
            })
            .map(|i| i as u32)
            .collect();

        let pmj = |name: &str| -> Vec<u32> {
            if let Some(s) = sets.get(name) {
                return valid(s);
            }
            let terminal = match name {
                "rbb" => cloud.landmarks.get("rbb_terminal"),
                other => cloud.landmarks.get(other),
            };
            let Some(&t) = terminal else {
                return Vec::new();
            };
            let tp = cloud.positions[t as usize];
            (0..n)
                .filter(|&i| {
                    kinds[i] == CellKind::Ventricular
                        && cloud.depth[i] <= PMJ_PATCH_DEPTH
                        && cloud.positions[i].distance(tp) <= PMJ_PATCH_RADIUS_MM
                })
                .map(|i| i as u32)
                .collect()
        };

        // Accessory pathways insert on the AV ring: the basal cell of each ventricle nearest to
        // its lateral landmark, and the atrial cell nearest to that.
        let basal = |region: Region, anchor: &str| -> Vec<u32> {
            let Some(&a) = cloud.landmarks.get(anchor) else {
                return Vec::new();
            };
            let ap = cloud.positions[a as usize];
            nearest_where(cloud, ap, |i| {
                cloud.regions[i] == region && apex_base.get(i).is_some_and(|z| *z >= BASAL_LIMIT)
            })
            .or(Some(a))
            .into_iter()
            .collect()
        };
        let ap_v_left = basal(Region::LeftVentricle, "lv_lateral");
        let ap_v_right = basal(Region::RightVentricle, "rv_free_wall");
        let atrial_near = |v: &[u32], region: Region| -> Vec<u32> {
            v.first()
                .and_then(|&c| {
                    nearest_where(cloud, cloud.positions[c as usize], |i| cloud.regions[i] == region)
                })
                .into_iter()
                .collect()
        };
        let ap_a_left = atrial_near(&ap_v_left, Region::LeftAtrium);
        let ap_a_right = atrial_near(&ap_v_right, Region::RightAtrium);

        let rv_apex = landmark(cloud, "rv_apex");
        let ra_lateral = landmark(cloud, "ra_lateral");
        Self {
            sinus,
            av_input,
            rbb: pmj("rbb"),
            laf: pmj("laf"),
            lpf: pmj("lpf"),
            right_atrium: ra_lateral.clone(),
            left_atrium: landmark(cloud, "la_roof"),
            right_ventricle: rv_apex.clone(),
            left_ventricle: landmark(cloud, "lv_lateral"),
            ventricular_escape: landmark(cloud, "lv_septal"),
            atrial_lead: ra_lateral,
            ventricular_lead: rv_apex,
            ap_atrial: [ap_a_left, ap_a_right],
            ap_ventricular: [ap_v_left, ap_v_right],
            foci: Vec::new(),
        }
    }

    /// Place ectopic focus `id` (`Origin::Focus(id)`) at a site.
    pub fn set_focus_site(&mut self, cloud: &CellCloud, id: u8, site: &StimSite) {
        let id = id as usize;
        if self.foci.len() <= id {
            self.foci.resize(id + 1, None);
        }
        self.foci[id] = Some(resolve_site(cloud, site).into_iter().collect());
    }

    /// Default site of a chamber region (ectopic beats without an explicit site).
    #[must_use]
    pub fn chamber_default(&self, region: Region) -> &[u32] {
        match region {
            Region::LeftAtrium | Region::LeftAtrialAppendage | Region::PulmonaryVeinSleeves => {
                &self.left_atrium
            }
            r if r.is_atrial() => &self.right_atrium,
            Region::RightVentricle => &self.right_ventricle,
            _ => &self.left_ventricle,
        }
    }
}
