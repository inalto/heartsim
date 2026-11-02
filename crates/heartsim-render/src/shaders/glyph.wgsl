// Instanced spheres and tapered tubes. `common.wgsl` is prepended.

struct UnitVertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct Instance {
    // xyz: centre / tube start, w: radius at start
    @location(2) a: vec4<f32>,
    // xyz: tube end, w: radius at end
    @location(3) b: vec4<f32>,
    @location(4) color: vec4<f32>,
};

struct GlyphOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
};

@vertex
fn vs_sphere(v: UnitVertex, i: Instance) -> GlyphOut {
    var out: GlyphOut;
    let world = i.a.xyz + v.position * i.a.w;
    out.clip = g.view_proj * vec4<f32>(world, 1.0);
    out.world = world;
    out.normal = v.normal;
    out.color = i.color.rgb;
    return out;
}

@vertex
fn vs_tube(v: UnitVertex, i: Instance) -> GlyphOut {
    var out: GlyphOut;
    let axis = i.b.xyz - i.a.xyz;
    let len = max(length(axis), 1e-6);
    let z = axis / len;
    var helper = vec3<f32>(0.0, 1.0, 0.0);
    if (abs(z.y) > 0.99) {
        helper = vec3<f32>(1.0, 0.0, 0.0);
    }
    let x = normalize(cross(helper, z));
    let y = cross(z, x);
    let radius = mix(i.a.w, i.b.w, v.position.z);
    let world = i.a.xyz + z * (v.position.z * len) + (x * v.position.x + y * v.position.y) * radius;
    // Side normals tilt along the axis on a cone: slope = (r_start - r_end) / length.
    let side = length(v.normal.xy);
    let n_local = vec3<f32>(v.normal.xy, v.normal.z + side * (i.a.w - i.b.w) / len);
    let n = normalize(x * n_local.x + y * n_local.y + z * n_local.z);
    out.clip = g.view_proj * vec4<f32>(world, 1.0);
    out.world = world;
    out.normal = n;
    out.color = i.color.rgb;
    return out;
}

@fragment
fn fs_glyph(in: GlyphOut) -> @location(0) vec4<f32> {
    if (is_clipped(in.world)) {
        discard;
    }
    return vec4<f32>(shade(in.color, normalize(in.normal), in.world), 1.0);
}
