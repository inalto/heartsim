//! Transcendental functions that give the same bits on every platform.
//!
//! `f32::exp`, `powf`, `tanh`, `sin`, `atan2` … call the system's libm, and glibc, the MSVC CRT
//! and Apple's libm round differently in the last bit. The engines are chaotic where it matters
//! most (re-entry, fibrillation): a one-ulp difference in a gate variable grows into a different
//! rhythm within seconds, so the same scenario and seed gave AF on Linux and a regular
//! tachycardia on an Apple-silicon runner. The `libm` crate is pure Rust (musl's algorithms, plain
//! IEEE arithmetic, no FMA), so its results are identical on x86-64, aarch64 and wasm32.
//!
//! Engines use these through [`Deterministic`] instead of the inherent methods; the renderer and
//! the UI may keep the system functions.

/// The transcendental functions the engines need, with platform-independent results.
pub trait Deterministic: Copy {
    /// `e^self`.
    #[must_use]
    fn exp_d(self) -> Self;
    /// Natural logarithm.
    #[must_use]
    fn ln_d(self) -> Self;
    /// `self^p`.
    #[must_use]
    fn powf_d(self, p: Self) -> Self;
    /// Hyperbolic tangent.
    #[must_use]
    fn tanh_d(self) -> Self;
    /// Sine (radians).
    #[must_use]
    fn sin_d(self) -> Self;
    /// Cosine (radians).
    #[must_use]
    fn cos_d(self) -> Self;
    /// Sine and cosine (radians).
    #[must_use]
    fn sin_cos_d(self) -> (Self, Self) {
        (self.sin_d(), self.cos_d())
    }
    /// Four-quadrant arctangent of `self / x`.
    #[must_use]
    fn atan2_d(self, x: Self) -> Self;
    /// Arccosine, in `[0, π]`.
    #[must_use]
    fn acos_d(self) -> Self;
}

impl Deterministic for f32 {
    fn exp_d(self) -> f32 {
        libm::expf(self)
    }
    fn ln_d(self) -> f32 {
        libm::logf(self)
    }
    fn powf_d(self, p: f32) -> f32 {
        libm::powf(self, p)
    }
    fn tanh_d(self) -> f32 {
        libm::tanhf(self)
    }
    fn sin_d(self) -> f32 {
        libm::sinf(self)
    }
    fn cos_d(self) -> f32 {
        libm::cosf(self)
    }
    fn atan2_d(self, x: f32) -> f32 {
        libm::atan2f(self, x)
    }
    fn acos_d(self) -> f32 {
        libm::acosf(self)
    }
}

impl Deterministic for f64 {
    fn exp_d(self) -> f64 {
        libm::exp(self)
    }
    fn ln_d(self) -> f64 {
        libm::log(self)
    }
    fn powf_d(self, p: f64) -> f64 {
        libm::pow(self, p)
    }
    fn tanh_d(self) -> f64 {
        libm::tanh(self)
    }
    fn sin_d(self) -> f64 {
        libm::sin(self)
    }
    fn cos_d(self) -> f64 {
        libm::cos(self)
    }
    fn atan2_d(self, x: f64) -> f64 {
        libm::atan2(self, x)
    }
    fn acos_d(self) -> f64 {
        libm::acos(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_functions_agree_with_std_to_a_few_ulps() {
        for &x in &[-3.0f32, -0.5, 0.0, 0.25, 1.0, 2.5, 7.0] {
            assert!((x.exp_d() - x.exp()).abs() <= 2.0 * f32::EPSILON * x.exp());
            assert!((x.tanh_d() - x.tanh()).abs() <= 4.0 * f32::EPSILON);
            assert!((x.sin_d() - x.sin()).abs() <= 4.0 * f32::EPSILON);
            assert!((x.cos_d() - x.cos()).abs() <= 4.0 * f32::EPSILON);
            assert!((x.atan2_d(1.5) - x.atan2(1.5)).abs() <= 4.0 * f32::EPSILON);
        }
        for &x in &[0.1f32, 0.9, 1.0, 3.0] {
            assert!((x.ln_d() - x.ln()).abs() <= 4.0 * f32::EPSILON);
            assert!((x.powf_d(1.5) - x.powf(1.5)).abs() <= 4.0 * f32::EPSILON * x.powf(1.5));
        }
        assert!(((0.3f32).acos_d() - 0.3f32.acos()).abs() <= 4.0 * f32::EPSILON);
        let (s, c) = 0.7f64.sin_cos_d();
        assert!((s - 0.7f64.sin()).abs() < 1e-15 && (c - 0.7f64.cos()).abs() < 1e-15);
        assert!((2.0f64.powf_d(0.5) - 2.0f64.sqrt()).abs() < 1e-15);
    }

    #[test]
    fn results_are_the_documented_bits() {
        // pinned values: a platform whose libm differs would change these, which is the point
        assert_eq!(1.0f32.exp_d().to_bits(), 0x402D_F854);
        assert_eq!(0.5f32.tanh_d().to_bits(), 0x3EEC_9A9E);
        // one ulp above the correctly rounded e: libm is not correctly rounded, only consistent
        assert_eq!(1.0f64.exp_d().to_bits(), 0x4005_BF0A_8B14_576A);
    }
}
