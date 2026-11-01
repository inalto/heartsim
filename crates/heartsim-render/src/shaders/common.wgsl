// Shared by scene.wgsl and glyph.wgsl (concatenated at build time; WGSL has no #include).

struct Globals {
    view_proj: mat4x4<f32>,
    // xyz: eye position, w: unused
    camera_pos: vec4<f32>,
    // xyz: unit direction *towards* the light (world space), w: intensity
    key_dir: vec4<f32>,
    fill_dir: vec4<f32>,
    // xyz: point on the clip plane, w: enabled (0/1)
    clip_point: vec4<f32>,
    // xyz: unit normal; fragments on the side the normal points to are discarded
    clip_normal: vec4<f32>,
    // rgb: flat colour of the cut-away interior
    cut_color: vec4<f32>,
    bg_top: vec4<f32>,
    bg_bottom: vec4<f32>,
    // x: ambient, y: specular strength, z: shininess, w: rim strength
    shading: vec4<f32>,
    // x: rim power, y: viewport width px, z: viewport height px, w: unused
    misc: vec4<f32>,
};

struct LayerU {
    // rgb: linear base colour, a: opacity
    base_color: vec4<f32>,
    // x: scalar min, y: scalar max, z: colour map enabled, w: LUT row
    scalar: vec4<f32>,
    // x: clip enabled, y: contour level, z: contour width px (0 = off)
    flags: vec4<f32>,
    contour_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var lut_tex: texture_2d<f32>;
@group(0) @binding(2) var lut_sampler: sampler;
@group(1) @binding(0) var<uniform> layer: LayerU;

const LUT_ROWS: f32 = 4.0;

// True when the world-space point lies on the removed side of the clip plane of a clipped layer.
fn is_clipped(world: vec3<f32>) -> bool {
    return layer.flags.x > 0.5
        && g.clip_point.w > 0.5
        && dot(world - g.clip_point.xyz, g.clip_normal.xyz) > 0.0;
}

// Two-light Blinn-Phong plus a view-dependent rim. All inputs/outputs are linear light; the sRGB
// render target does the encoding.
fn shade(albedo: vec3<f32>, n: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    let v = normalize(g.camera_pos.xyz - world);
    var color = albedo * g.shading.x;
    for (var i = 0; i < 2; i++) {
        var l = g.key_dir;
        if (i == 1) {
            l = g.fill_dir;
        }
        let ldir = normalize(l.xyz);
        let ndl = max(dot(n, ldir), 0.0);
        let h = normalize(ldir + v);
        let spec = pow(max(dot(n, h), 0.0), g.shading.z) * g.shading.y * step(0.0001, ndl);
        color += (albedo * ndl + vec3<f32>(spec)) * l.w;
    }
    let rim = pow(1.0 - clamp(dot(n, v), 0.0, 1.0), g.misc.x) * g.shading.w;
    return color + albedo * rim + vec3<f32>(rim * 0.15);
}
