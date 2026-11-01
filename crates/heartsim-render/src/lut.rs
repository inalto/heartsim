//! Colour look-up tables as 256×1 RGBA8 textures. Scalars are normalised to [0, 1] by the caller.

/// Voltage map: blue (rest) → yellow → red (depolarised); matches what students see in textbooks.
#[must_use]
pub fn voltage_lut() -> [[u8; 4]; 256] {
    let mut out = [[0u8; 4]; 256];
    for (i, px) in out.iter_mut().enumerate() {
        let t = i as f32 / 255.0;
        let (r, g, b) = if t < 0.5 {
            let u = t * 2.0; // blue → yellow
            (u, u * 0.9, 1.0 - u)
        } else {
            let u = (t - 0.5) * 2.0; // yellow → red
            (1.0, 0.9 * (1.0 - u), 0.0)
        };
        *px = [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 255];
    }
    out
}

/// Isochrone map (activation time): perceptually ordered viridis-like ramp.
#[must_use]
pub fn isochrone_lut() -> [[u8; 4]; 256] {
    let mut out = [[0u8; 4]; 256];
    for (i, px) in out.iter_mut().enumerate() {
        let t = i as f32 / 255.0;
        let r = (0.27 + 0.9 * t * t).min(1.0);
        let g = (0.0 + 1.1 * t).min(0.9);
        let b = (0.33 + 0.6 * (1.0 - t) * (0.5 + 0.5 * t)).min(1.0);
        *px = [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 255];
    }
    out
}
