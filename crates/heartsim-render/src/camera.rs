use glam::{Mat4, Vec3, Vec4};

/// Right-handed look-at (eye, target, up) — kept local because glam's camera helpers are not part
/// of its core API in the version we pin.
#[must_use]
pub fn look_at_rh(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    let f = (target - eye).normalize();
    let s = f.cross(up).normalize();
    let u = s.cross(f);
    Mat4::from_cols(
        Vec4::new(s.x, u.x, -f.x, 0.0),
        Vec4::new(s.y, u.y, -f.y, 0.0),
        Vec4::new(s.z, u.z, -f.z, 0.0),
        Vec4::new(-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0),
    )
}

/// Right-handed perspective with zero-to-one depth (wgpu convention).
#[must_use]
pub fn perspective_rh_zo(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let f = 1.0 / (fov_y * 0.5).tan();
    let r = far / (near - far);
    Mat4::from_cols(
        Vec4::new(f / aspect, 0.0, 0.0, 0.0),
        Vec4::new(0.0, f, 0.0, 0.0),
        Vec4::new(0.0, 0.0, r, -1.0),
        Vec4::new(0.0, 0.0, r * near, 0.0),
    )
}

/// Standard radiographic / anatomical viewing directions. Names follow the angiography convention:
/// the projection is named after the side of the patient the camera (image intensifier) is on, so
/// in LAO the camera sits at the patient's left-anterior side (+x, +z in the heart frame).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CameraPreset {
    /// Anterior–posterior: camera in front of the chest; the patient's left is on screen right.
    Ap,
    /// Posterior–anterior: camera behind the patient.
    Pa,
    /// Left anterior oblique 45°: septum seen end-on, separates left and right ventricles.
    Lao,
    /// Right anterior oblique 45°: shows the AV groove and the long axis of the LV.
    Rao,
    /// Left lateral (LAO 90°).
    Left,
    /// Cranial tilt 60°: camera above, looking caudally.
    Cranial,
    /// Caudal tilt 60°: camera below, looking cranially.
    Caudal,
}

impl CameraPreset {
    /// Every preset, in toolbar order.
    pub const ALL: [CameraPreset; 7] = [
        CameraPreset::Ap,
        CameraPreset::Pa,
        CameraPreset::Lao,
        CameraPreset::Rao,
        CameraPreset::Left,
        CameraPreset::Cranial,
        CameraPreset::Caudal,
    ];

    /// Short button label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            CameraPreset::Ap => "AP",
            CameraPreset::Pa => "PA",
            CameraPreset::Lao => "LAO",
            CameraPreset::Rao => "RAO",
            CameraPreset::Left => "Left",
            CameraPreset::Cranial => "Cranial",
            CameraPreset::Caudal => "Caudal",
        }
    }

    /// Camera (yaw, pitch) in degrees for [`OrbitCamera`]: yaw is the azimuth from +z towards +x
    /// (patient's left), pitch the elevation towards +y (cranial).
    #[must_use]
    pub fn yaw_pitch_deg(self) -> (f32, f32) {
        match self {
            CameraPreset::Ap => (0.0, 0.0),
            CameraPreset::Pa => (180.0, 0.0),
            CameraPreset::Lao => (45.0, 0.0),
            CameraPreset::Rao => (-45.0, 0.0),
            CameraPreset::Left => (90.0, 0.0),
            CameraPreset::Cranial => (0.0, 60.0),
            CameraPreset::Caudal => (0.0, -60.0),
        }
    }
}

/// Camera yaw (degrees) for an oblique angle on the LAO–RAO axis in the transverse plane:
/// negative = LAO (towards the patient's left), positive = RAO, 0 = AP.
#[must_use]
pub fn oblique_yaw_deg(lao_rao_deg: f32) -> f32 {
    -lao_rao_deg
}

/// Orbit camera around a target: the standard interaction for anatomy viewers.
#[derive(Debug, Clone, Copy)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub distance: f32,
    /// Azimuth (radians) around +Y, 0 = looking from +Z (anterior view).
    pub yaw: f32,
    /// Elevation (radians), positive = looking from above (cranial).
    pub pitch: f32,
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
    /// Closest the user can zoom (same units as `distance`).
    pub min_distance: f32,
    /// Farthest the user can zoom out.
    pub max_distance: f32,
}

/// Default azimuth, degrees: a right anterior oblique of 45°, slightly from above. The heart's long
/// axis runs left, caudal and anterior (about 36° below horizontal in the frontal plane and 46°
/// anterior in the transverse plane), so the straight-on AP view looks down that axis and the left
/// ventricle hides the right ventricle and the atria behind a round outline. RAO 45° puts the long
/// axis in the screen plane with the apex at the lower right, the textbook anterior view.
pub const DEFAULT_YAW_DEG: f32 = -45.0;
/// Default elevation, degrees (see [`DEFAULT_YAW_DEG`]).
pub const DEFAULT_PITCH_DEG: f32 = 8.6;

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 4.0,
            yaw: DEFAULT_YAW_DEG.to_radians(),
            pitch: DEFAULT_PITCH_DEG.to_radians(),
            fov_y: 45f32.to_radians(),
            near: 0.05,
            far: 100.0,
            min_distance: 0.2,
            max_distance: 50.0,
        }
    }
}

impl OrbitCamera {
    #[must_use]
    pub fn eye(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        self.target + Vec3::new(sy * cp, sp, cy * cp) * self.distance
    }
    /// Right-handed look-at view matrix (camera looks down −Z in view space).
    #[must_use]
    pub fn view(&self) -> Mat4 {
        look_at_rh(self.eye(), self.target, Vec3::Y)
    }
    /// Right-handed perspective projection with wgpu/DirectX depth range [0, 1].
    #[must_use]
    pub fn projection(&self, aspect: f32) -> Mat4 {
        perspective_rh_zo(self.fov_y, aspect.max(1e-3), self.near, self.far)
    }
    #[must_use]
    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        self.projection(aspect) * self.view()
    }
    /// Mouse drag in logical pixels → rotation.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.01;
        self.pitch = (self.pitch + dy * 0.01).clamp(-1.5, 1.5);
    }
    pub fn zoom(&mut self, scroll: f32) {
        self.distance = (self.distance * (1.0 - scroll * 0.1)).clamp(self.min_distance, self.max_distance);
    }

    /// Aim at the box `(lo, hi)` from a distance that shows all of it, and scale the clip planes
    /// and zoom limits to the scene so millimetre-sized anatomy and unit test spheres both work.
    pub fn frame(&mut self, lo: Vec3, hi: Vec3) {
        self.target = (lo + hi) * 0.5;
        self.distance = ((hi - lo).length() * 1.4).max(1e-3);
        self.near = self.distance * 0.01;
        self.far = self.distance * 20.0;
        self.min_distance = self.distance * 0.05;
        self.max_distance = self.distance * 10.0;
    }

    /// As [`Self::frame`], from the nearest distance at which the whole box is inside a viewport
    /// of the given `aspect` (width / height) when seen along an axis-aligned direction: a tall
    /// box in a narrow viewport is limited by its width, which [`Self::frame`] ignores. `margin`
    /// is the free space left around the box, as a fraction of its size.
    pub fn frame_to_fit(&mut self, lo: Vec3, hi: Vec3, aspect: f32, margin: f32) {
        self.frame(lo, hi);
        let half = (hi - lo) * 0.5;
        let t = (self.fov_y * 0.5).tan().max(1e-6);
        let across = half.x.max(half.z);
        let fit = (half.y / t).max(across / (t * aspect.max(1e-3)));
        self.distance = (fit * (1.0 + margin) + across).clamp(self.min_distance, self.max_distance);
    }

    /// Unit vectors (right, up, backward) of the camera frame in world space. Backward points from
    /// the target towards the eye.
    #[must_use]
    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let back = (self.eye() - self.target).normalize();
        let right = Vec3::Y.cross(back).normalize();
        (right, back.cross(right), back)
    }

    /// Project a world point to widget-relative coordinates `(u, v, depth)`: `u`, `v` in 0–1 with
    /// `v` growing downwards (egui convention), `depth` the 0–1 NDC depth. `None` when the point is
    /// behind the camera.
    #[must_use]
    pub fn project(&self, world: Vec3, aspect: f32) -> Option<(f32, f32, f32)> {
        let c = self.view_projection(aspect) * world.extend(1.0);
        if c.w <= 1e-6 {
            return None;
        }
        let ndc = c.truncate() / c.w;
        Some((ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5, ndc.z))
    }
    /// Preset clinical views by name (`ap`, `pa`, `lao`, `rao`, `left_lateral`, `cranial`,
    /// `caudal`); unknown names are ignored. See [`CameraPreset`].
    pub fn preset(&mut self, name: &str) {
        let p = match name {
            "ap" => CameraPreset::Ap,
            "pa" => CameraPreset::Pa,
            "lao" => CameraPreset::Lao,
            "rao" => CameraPreset::Rao,
            "left_lateral" | "left" => CameraPreset::Left,
            "cranial" => CameraPreset::Cranial,
            "caudal" => CameraPreset::Caudal,
            _ => return,
        };
        self.apply_preset(p);
    }

    /// Turn the camera to `preset`, keeping target and distance.
    pub fn apply_preset(&mut self, preset: CameraPreset) {
        let (yaw, pitch) = preset.yaw_pitch_deg();
        self.yaw = yaw.to_radians();
        self.pitch = pitch.to_radians();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_maps_near_to_zero_and_far_to_one() {
        let p = perspective_rh_zo(1.0, 1.0, 0.1, 10.0);
        let n = p * Vec4::new(0.0, 0.0, -0.1, 1.0);
        let f = p * Vec4::new(0.0, 0.0, -10.0, 1.0);
        assert!((n.z / n.w).abs() < 1e-5);
        assert!((f.z / f.w - 1.0).abs() < 1e-5);
        let v = look_at_rh(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y) * Vec4::new(0.0, 0.0, 0.0, 1.0);
        assert!(
            (v.z + 5.0).abs() < 1e-5,
            "target is 5 units in front of the camera (−Z)"
        );
    }

    #[test]
    fn project_centre_and_behind() {
        let c = OrbitCamera::default();
        let (u, v, d) = c.project(c.target, 1.0).expect("in front");
        assert!((u - 0.5).abs() < 1e-4 && (v - 0.5).abs() < 1e-4 && (0.0..1.0).contains(&d));
        let behind = c.eye() + (c.eye() - c.target);
        assert!(c.project(behind, 1.0).is_none());
    }

    #[test]
    fn frame_scales_clip_planes_for_millimetre_scenes() {
        let mut c = OrbitCamera::default();
        c.frame(Vec3::splat(-100.0), Vec3::splat(100.0));
        assert!(c.far > c.distance * 2.0 && c.near < c.distance);
        c.zoom(-100.0);
        assert!(
            c.distance <= c.max_distance && c.distance > 50.0,
            "zoom limits follow the scene"
        );
    }

    /// A standing figure fits a narrow viewport as well as a wide one, and fills most of it.
    #[test]
    fn frame_to_fit_keeps_a_tall_box_inside_any_viewport() {
        let (lo, hi) = (Vec3::new(-390.0, -1117.0, -156.0), Vec3::new(283.0, 434.0, 137.0));
        for aspect in [0.2_f32, 0.45, 1.0, 2.5] {
            let mut c = OrbitCamera::default();
            c.frame_to_fit(lo, hi, aspect, 0.06);
            c.yaw = 0.0;
            c.pitch = 0.0;
            let (mut umin, mut umax, mut vmin, mut vmax) = (1.0_f32, 0.0_f32, 1.0_f32, 0.0_f32);
            for i in 0..8 {
                let p = Vec3::new(
                    if i & 1 == 0 { lo.x } else { hi.x },
                    if i & 2 == 0 { lo.y } else { hi.y },
                    if i & 4 == 0 { lo.z } else { hi.z },
                );
                let (u, v, _) = c.project(p, aspect).expect("in front of the camera");
                assert!(
                    (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v),
                    "aspect {aspect}: {u} {v}"
                );
                (umin, umax, vmin, vmax) = (umin.min(u), umax.max(u), vmin.min(v), vmax.max(v));
            }
            assert!(
                (umax - umin).max(vmax - vmin) > 0.6,
                "aspect {aspect}: the box fills the limiting dimension"
            );
        }
    }

    #[test]
    fn default_view_has_the_long_axis_in_the_screen_plane() {
        // The LV long axis of the default anatomy pack (mitral annulus → apex), see
        // `heartsim_geometry::procedural::heart`.
        let apex = Vec3::new(0.62015, -0.45057, 0.64219);
        let c = OrbitCamera::default();
        let view_dir = (c.target - c.eye()).normalize();
        assert!(
            view_dir.dot(apex).abs() < 0.2,
            "the default camera should look across the long axis, not down it: {}",
            view_dir.dot(apex)
        );
        assert!(
            c.eye().x < 0.0 && c.eye().z > 0.0 && c.eye().y > 0.0,
            "RAO from slightly above"
        );
    }

    #[test]
    fn eye_is_at_distance_and_presets_rotate() {
        let mut c = OrbitCamera::default();
        assert!((c.eye().length() - c.distance).abs() < 1e-5);
        c.preset("lao");
        assert!(
            c.eye().x > 0.0 && c.eye().z > 0.0,
            "LAO looks from the patient's left-anterior side (+x = patient's left)"
        );
        c.preset("rao");
        assert!(c.eye().x < 0.0 && c.eye().z > 0.0);
        c.apply_preset(CameraPreset::Pa);
        assert!(c.eye().z < 0.0);
        c.yaw = oblique_yaw_deg(-90.0).to_radians();
        c.pitch = 0.0;
        assert!(c.eye().x > 0.99 * c.distance, "LAO 90 is the left lateral view");
        c.preset("cranial");
        assert!(c.eye().y > 0.0);
    }
}
