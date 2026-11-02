// Mesh layers (opaque, cut-away interior, transparent) and the background gradient.
// `common.wgsl` is prepended.

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) scalar: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) scalar: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = g.view_proj * vec4<f32>(in.position, 1.0);
    out.world = in.position;
    out.normal = in.normal;
    out.scalar = in.scalar;
    return out;
}

@fragment
fn fs_main(in: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    // Derivatives and texture reads happen before the (non-uniform) discard.
    let range = max(layer.scalar.y - layer.scalar.x, 1e-6);
    let t = clamp((in.scalar - layer.scalar.x) / range, 0.0, 1.0);
    let row_v = (layer.scalar.w + 0.5) / LUT_ROWS;
    let mapped = textureSampleLevel(lut_tex, lut_sampler, vec2<f32>(t, row_v), 0.0).rgb;
    let fw = max(fwidth(in.scalar), 1e-9);
    if (is_clipped(in.world)) {
        discard;
    }
    var n = normalize(in.normal);
    if (!front) {
        n = -n;
    }
    var albedo = mix(layer.base_color.rgb, mapped, layer.scalar.z);
    if (layer.flags.z > 0.0 && layer.scalar.z > 0.5) {
        // Distance to the iso-line in pixels; anti-aliased with a 1 px ramp.
        let d_px = abs(in.scalar - layer.flags.y) / fw;
        let edge = 1.0 - smoothstep(layer.flags.z * 0.5 - 0.5, layer.flags.z * 0.5 + 0.5, d_px);
        albedo = mix(albedo, layer.contour_color.rgb, edge);
    }
    return vec4<f32>(shade(albedo, n, in.world), layer.base_color.a);
}

// Back faces of a clipped layer, visible only through the cut: flat, unlit "cut" colour.
@fragment
fn fs_cut(in: VsOut) -> @location(0) vec4<f32> {
    if (is_clipped(in.world)) {
        discard;
    }
    return vec4<f32>(g.cut_color.rgb, 1.0);
}

struct BgOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) v: f32,
};

// Fullscreen triangle at the far plane; `v` is 0 at the top of the viewport, 1 at the bottom.
@vertex
fn vs_bg(@builtin(vertex_index) i: u32) -> BgOut {
    var out: BgOut;
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    out.clip = vec4<f32>(x, y, 1.0, 1.0);
    out.v = 0.5 - 0.5 * y;
    return out;
}

@fragment
fn fs_bg(in: BgOut) -> @location(0) vec4<f32> {
    return vec4<f32>(mix(g.bg_top.rgb, g.bg_bottom.rgb, in.v), 1.0);
}
