//! The heart vector in the 3-D view (briefs B8 §2, B9 §4): the instantaneous cardiac dipole as an
//! arrow from the ventricular centroid and, optionally, the loop of the last beat.
//!
//! The dipole is not taken from the 3-D engine: it is recovered from the twelve leads on the strip
//! with B4's least-squares inverse ([`HeartVectorSolver`]) at the strip's cursor sample. So while
//! scrubbing, the arrow, the colour map and the strip all show the same instant, and whatever the
//! student sees in the leads is exactly what the arrow shows.

use eframe::egui;
use glam::{DVec3, Vec3};
use heartsim_core::LEAD_COUNT;
use heartsim_ecg::{HeartVectorSolver, LeadRing, LeadVectors};
use heartsim_render::{GlyphInstance, GlyphSetId, glyph};
use heartsim_ui::ViewportResources;
use heartsim_ui::t;

/// Default arrow length per millivolt, mm: a 1.5 mV QRS peak reaches ~70 mm, just past the
/// ventricular surface of the default pack.
pub const DEFAULT_SCALE_MM_PER_MV: f32 = 45.0;

/// The 12-lead heart vector (mV, heart frame) at absolute sample `abs` of `rings`.
#[must_use]
pub fn dipole_at(rings: &[LeadRing], abs: u64, solver: &HeartVectorSolver) -> Option<DVec3> {
    if rings.len() < LEAD_COUNT {
        return None;
    }
    let mut v = [0.0; LEAD_COUNT];
    for (o, r) in v.iter_mut().zip(rings) {
        *o = f64::from(r.at(abs)?);
    }
    Some(solver.solve(&v))
}

/// Arrow and loop glyphs plus their display options.
pub struct VectorOverlay {
    solver: HeartVectorSolver,
    /// Show the instantaneous arrow.
    pub arrow: bool,
    /// Show the last beat's loop.
    pub show_loop: bool,
    pub scale_mm_per_mv: f32,
    render_state: Option<egui_wgpu::RenderState>,
    set: Option<GlyphSetId>,
    /// Hooded-layer opacity before "see-through" was switched on.
    saved_alpha: Option<Vec<f32>>,
}

impl std::fmt::Debug for VectorOverlay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorOverlay")
            .field("arrow", &self.arrow)
            .field("show_loop", &self.show_loop)
            .finish_non_exhaustive()
    }
}

impl Default for VectorOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorOverlay {
    #[must_use]
    pub fn new() -> Self {
        Self {
            solver: HeartVectorSolver::new(&LeadVectors::standard()).expect("standard leads span 3-D"),
            arrow: false,
            show_loop: false,
            scale_mm_per_mv: DEFAULT_SCALE_MM_PER_MV,
            render_state: None,
            set: None,
            saved_alpha: None,
        }
    }

    /// The solver the overlay (and the vector tab) uses.
    #[must_use]
    pub fn solver(&self) -> &HeartVectorSolver {
        &self.solver
    }

    /// Keep the render state to upload glyphs later.
    pub fn install(&mut self, rs: &egui_wgpu::RenderState) {
        self.render_state = Some(rs.clone());
    }

    /// Glyphs for `dipole` (arrow) and `loop_mv` (polyline) around the origin, mm. Returns
    /// `(tubes, spheres)`.
    #[must_use]
    pub fn glyphs(
        &self,
        dipole: Option<DVec3>,
        loop_mv: &[DVec3],
    ) -> (Vec<GlyphInstance>, Vec<GlyphInstance>) {
        let s = self.scale_mm_per_mv;
        let to_mm = |v: DVec3| v.as_vec3() * s;
        let (mut tubes, mut spheres) = (Vec::new(), Vec::new());
        if self.show_loop && loop_mv.len() > 1 {
            let pts: Vec<Vec3> = loop_mv.iter().map(|&v| to_mm(v)).collect();
            let (t, j) = glyph::polyline(&pts, 0.8, [0.35, 0.8, 1.0, 1.0]);
            tubes.extend(t);
            spheres.extend(j);
        }
        if self.arrow
            && let Some(d) = dipole
        {
            tubes.extend(glyph::arrow(
                Vec3::ZERO,
                to_mm(d),
                1.6,
                0.22,
                [1.0, 0.82, 0.2, 1.0],
            ));
            spheres.push(GlyphInstance::sphere(Vec3::ZERO, 2.4, [1.0, 0.82, 0.2, 1.0]));
        }
        (tubes, spheres)
    }

    /// Upload this frame's glyphs into the viewport scene.
    pub fn sync(&mut self, dipole: Option<DVec3>, loop_mv: &[DVec3]) {
        let Some(rs) = &self.render_state else {
            return;
        };
        let (tubes, spheres) = self.glyphs(dipole, loop_mv);
        let mut renderer = rs.renderer.write();
        let Some(res) = renderer.callback_resources.get_mut::<ViewportResources>() else {
            return;
        };
        let id = *self
            .set
            .get_or_insert_with(|| res.scene.add_glyph_set("heart vector"));
        res.scene.set_tubes(&rs.device, &rs.queue, id, &tubes);
        res.scene.set_spheres(&rs.device, &rs.queue, id, &spheres);
    }

    /// Toggles for the Simulator tab. `alphas` are the opacities of the heart's layers: the
    /// "see-through" switch lowers them so the arrow inside the ventricles is visible, and
    /// restores them when switched off.
    pub fn controls(&mut self, ui: &mut egui::Ui, alphas: &mut [&mut f32]) {
        ui.strong(t!("vector-heading"));
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.arrow, t!("vec-arrow"));
            ui.checkbox(&mut self.show_loop, t!("vec-loop"));
        });
        let mut see_through = self.saved_alpha.is_some();
        if ui
            .checkbox(&mut see_through, t!("vec-see-through"))
            .on_hover_text(t!("vec-see-through-hint"))
            .changed()
        {
            self.set_see_through(see_through, alphas);
        }
        ui.add(egui::Slider::new(&mut self.scale_mm_per_mv, 10.0..=120.0).text(t!("vec-scale")));
    }

    /// Lower (or restore) the layer opacities.
    pub fn set_see_through(&mut self, on: bool, alphas: &mut [&mut f32]) {
        match (on, self.saved_alpha.take()) {
            (true, None) => {
                self.saved_alpha = Some(alphas.iter().map(|a| **a).collect());
                for a in alphas.iter_mut() {
                    **a = a.min(0.35);
                }
            }
            (false, Some(saved)) => {
                for (a, s) in alphas.iter_mut().zip(saved) {
                    **a = s;
                }
            }
            (_, keep) => self.saved_alpha = keep,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dipole_from_rings_recovers_the_projected_vector() {
        let lv = LeadVectors::standard();
        let solver = HeartVectorSolver::new(&lv).unwrap();
        let d = DVec3::new(0.6, -1.1, 0.4);
        let mut rings: Vec<LeadRing> = (0..LEAD_COUNT).map(|_| LeadRing::new(16, 500.0)).collect();
        for (r, v) in rings.iter_mut().zip(lv.project(d)) {
            r.push(0.0);
            r.push(v as f32);
        }
        let got = dipole_at(&rings, 1, &solver).unwrap();
        assert!((got - d).length() < 1e-5, "{got} vs {d}");
        assert!(dipole_at(&rings, 2, &solver).is_none(), "not pushed yet");
    }

    #[test]
    fn glyphs_follow_the_toggles_and_scale() {
        let mut o = VectorOverlay::new();
        let d = Some(DVec3::new(1.0, 0.0, 0.0));
        let lp = [DVec3::ZERO, DVec3::X, DVec3::Y];
        assert_eq!(o.glyphs(d, &lp), (Vec::new(), Vec::new()));
        o.arrow = true;
        let (tubes, _) = o.glyphs(d, &lp);
        assert_eq!(tubes.len(), 2, "shaft + head");
        let tip = Vec3::from(tubes[1].b);
        assert!((tip - Vec3::X * DEFAULT_SCALE_MM_PER_MV).length() < 1e-3);
        o.show_loop = true;
        assert_eq!(o.glyphs(d, &lp).0.len(), 4, "two loop segments + arrow");
    }

    #[test]
    fn see_through_restores_the_previous_opacity() {
        let mut o = VectorOverlay::new();
        let (mut a, mut b) = (1.0_f32, 0.2_f32);
        o.set_see_through(true, &mut [&mut a, &mut b]);
        assert!((a - 0.35).abs() < 1e-6 && (b - 0.2).abs() < 1e-6);
        o.set_see_through(false, &mut [&mut a, &mut b]);
        assert!((a - 1.0).abs() < 1e-6 && (b - 0.2).abs() < 1e-6);
    }
}
