struct Uniforms {
    view_proj: mat4x4<f32>,
    light_dir: vec4<f32>,
    base_color: vec4<f32>,
    // x: scalar min, y: scalar max, z: colour-map blend (0 = base colour only), w: unused
    scalar_range: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var lut_tex: texture_2d<f32>;
@group(0) @binding(2) var lut_sampler: sampler;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) scalar: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) scalar: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = u.view_proj * vec4<f32>(in.position, 1.0);
    out.normal = in.normal;
    out.scalar = in.scalar;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let l = normalize(u.light_dir.xyz);
    let diffuse = max(dot(n, l), 0.0);
    let ambient = 0.35;
    let t = clamp((in.scalar - u.scalar_range.x) / max(u.scalar_range.y - u.scalar_range.x, 1e-6), 0.0, 1.0);
    let mapped = textureSample(lut_tex, lut_sampler, vec2<f32>(t, 0.5)).rgb;
    let albedo = mix(u.base_color.rgb, mapped, u.scalar_range.z);
    let color = albedo * (ambient + 0.65 * diffuse) + vec3<f32>(0.08) * pow(diffuse, 16.0);
    return vec4<f32>(color, 1.0);
}
