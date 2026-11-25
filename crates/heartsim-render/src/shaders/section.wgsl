// Scan conversion of the echo page (briefs B31 §12 and B32): a polar label image (one row per
// scan line, one column per depth sample, the tissue label in the byte, bit 7 = hidden behind
// bone or lung) and a polar grey image of the same layout (the simulated B-mode brightness)
// drawn as a sector, apex up. The labels give flat colours from a palette, the grey image its
// brightness; the mode picks one of them or the colours blended over the grey. No compute, no
// storage buffers, no sampler: one uniform block and two non-filterable textures read with
// textureLoad (the grey image is interpolated by hand between four texels).
//
// Geometry (logical px of the widget, y down): the apex sits at `geom.xy`; the beam runs straight
// down; `theta` is the angle from the beam, positive to the right (the index-marker side), so line
// 0 is at the left edge and the last line at the right edge of the sector.

struct Section {
    // apex x, apex y (px), px per mm, depth (mm)
    geom: vec4<f32>,
    // widget width, height (px), half sector angle (rad), hatch period (px)
    view: vec4<f32>,
    // lines, samples, brightness of a hidden sample, mode (0 labels, 1 grey, 2 labels over grey)
    tex: vec4<f32>,
    // grey image: lines, samples, opacity of the colours in mode 2, 1 when the target is sRGB
    grey: vec4<f32>,
    // linear or sRGB, as the target needs, indexed by the label byte (0..63)
    palette: array<vec4<f32>, 64>,
};

@group(0) @binding(0) var<uniform> section: Section;
@group(0) @binding(1) var labels: texture_2d<f32>;
@group(0) @binding(2) var greys: texture_2d<f32>;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Fullscreen triangle; the caller sets the viewport and scissor to the widget rectangle.
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    var out: VsOut;
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    out.clip = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, 1.0 - (y + 1.0) * 0.5);
    return out;
}

fn grey_load(x: i32, y: i32) -> f32 {
    let xi = clamp(x, 0, max(i32(section.grey.y) - 1, 0));
    let yi = clamp(y, 0, max(i32(section.grey.x) - 1, 0));
    return textureLoad(greys, vec2<i32>(xi, yi), 0).r;
}

// The grey level at a fractional (line, sample) position of the grey image, bilinear between the
// four nearest texel centres.
fn grey_bilinear(line_f: f32, sample_f: f32) -> f32 {
    let fx = sample_f - 0.5;
    let fy = line_f - 0.5;
    let x0 = floor(fx);
    let y0 = floor(fy);
    let tx = fx - x0;
    let ty = fy - y0;
    let xi = i32(x0);
    let yi = i32(y0);
    let top = mix(grey_load(xi, yi), grey_load(xi + 1, yi), tx);
    let bottom = mix(grey_load(xi, yi + 1), grey_load(xi + 1, yi + 1), tx);
    return mix(top, bottom, ty);
}

// The display value (perceptual, 0..1) as the target needs it: linear for an sRGB target.
fn to_target(v: f32) -> f32 {
    if (section.grey.w > 0.5) {
        return select(pow((v + 0.055) / 1.055, 2.4), v / 12.92, v <= 0.04045);
    }
    return v;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = in.uv * section.view.xy;
    let d = p - section.geom.xy;
    // above the apex there is nothing
    if (d.y <= 0.0) {
        discard;
    }
    let r_px = length(d);
    let depth_px = section.geom.w * section.geom.z;
    let half_angle = section.view.z;
    let theta = atan2(d.x, d.y);
    // distance inside each edge of the sector, px: a one-pixel anti-aliased border
    let radial = depth_px - r_px;
    let angular = (half_angle - abs(theta)) * r_px;
    let cover = clamp(min(radial, angular) + 0.5, 0.0, 1.0);
    if (cover <= 0.0) {
        discard;
    }
    let mode = section.tex.w;
    let unit_line = clamp((theta + half_angle) / (2.0 * half_angle), 0.0, 0.999999);
    let unit_sample = clamp(r_px / depth_px, 0.0, 0.999999);
    var grey = vec3<f32>(0.0);
    if (mode > 0.5) {
        let g = grey_bilinear(unit_line * section.grey.x, unit_sample * section.grey.y);
        grey = vec3<f32>(to_target(clamp(g, 0.0, 1.0)));
        if (mode < 1.5) {
            return vec4<f32>(grey, cover);
        }
    }
    let line_f = unit_line * section.tex.x;
    let sample_f = unit_sample * section.tex.y;
    let texel = textureLoad(labels, vec2<i32>(i32(sample_f), i32(line_f)), 0).r;
    let code = u32(round(texel * 255.0));
    let hidden = (code & 128u) != 0u;
    var colour = section.palette[code & 63u].rgb;
    if (hidden) {
        colour = colour * section.tex.z;
        // diagonal hatch over the dimmed colour, so "behind a rib" does not rely on brightness alone
        let period = max(section.view.w, 2.0);
        if (fract((p.x + p.y) / period) < 0.3) {
            colour = colour * 0.55;
        }
    }
    if (mode > 1.5) {
        // the colours over the grey image; where the model has no tissue the grey shows alone
        if ((code & 63u) == 0u) {
            return vec4<f32>(grey, cover);
        }
        colour = mix(grey, colour, section.grey.z);
    }
    return vec4<f32>(colour, cover);
}
