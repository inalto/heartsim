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
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 4.0,
            yaw: 0.0,
            pitch: 0.15,
            fov_y: 45f32.to_radians(),
            near: 0.05,
            far: 100.0,
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
        self.distance = (self.distance * (1.0 - scroll * 0.1)).clamp(0.2, 50.0);
    }
    /// Preset clinical views: anterior (AP), LAO, RAO, cranial, caudal — angles in degrees.
    pub fn preset(&mut self, name: &str) {
        let (yaw, pitch) = match name {
            "ap" => (0.0, 0.0),
            "lao" => (-45.0, 0.0),
            "rao" => (45.0, 0.0),
            "left_lateral" => (-90.0, 0.0),
            "cranial" => (0.0, 60.0),
            "caudal" => (0.0, -60.0),
            _ => return,
        };
        self.yaw = (yaw as f32).to_radians();
        self.pitch = (pitch as f32).to_radians();
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
    fn eye_is_at_distance_and_presets_rotate() {
        let mut c = OrbitCamera::default();
        assert!((c.eye().length() - c.distance).abs() < 1e-5);
        c.preset("lao");
        assert!(c.eye().x < 0.0, "LAO looks from the patient's left-anterior side");
        c.preset("cranial");
        assert!(c.eye().y > 0.0);
    }
}
