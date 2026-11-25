//! The browser build falls back to WebGL2 when `navigator.gpu` is missing (brief B21 §3). wgpu's GLES
//! backend cannot run compute, read storage buffers in the vertex stage, or honour `@interpolate(linear)`,
//! so the shaders must not use them. This test needs no GPU: it checks the WGSL source.

use std::path::Path;

fn shaders() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/shaders");
    let mut out: Vec<_> = std::fs::read_dir(&dir)
        .expect("shader dir")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "wgsl"))
        .map(|p| {
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read_to_string(&p).expect("shader source"),
            )
        })
        .collect();
    out.sort();
    out
}

/// Source with `//` comments removed, so explanations that name a forbidden construct do not trip the check.
fn code(src: &str) -> String {
    src.lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn there_are_shaders_to_check() {
    assert!(shaders().len() >= 4);
}

#[test]
fn no_shader_uses_what_webgl2_lacks() {
    for (name, src) in shaders() {
        let code = code(&src);
        for (needle, why) in [
            (
                "var<storage",
                "storage buffers are not available in WebGL2 vertex shaders",
            ),
            ("@compute", "compute shaders do not exist in WebGL2"),
            (
                "@interpolate(linear",
                "linear interpolation sampling is not supported by GLSL ES 3.00",
            ),
            ("atomic<", "atomics need storage buffers"),
            ("textureStore", "storage textures are not available in WebGL2"),
            ("texture_storage", "storage textures are not available in WebGL2"),
        ] {
            assert!(!code.contains(needle), "{name}: `{needle}`: {why}");
        }
    }
}

/// The echo page's scan-conversion shader (briefs B31 and B32): one uniform block, two
/// non-filterable float textures (the label image and the grey image) read with `textureLoad`
/// (the grey image is interpolated by hand); no integer textures (both are `R8Unorm`), no
/// derivatives, no sampler, no storage of any kind. The generic test above covers the banned
/// constructs; this one pins what the shader does use.
#[test]
fn the_section_shader_is_covered_and_stays_inside_what_webgl2_has() {
    let all = shaders();
    let (_, src) = all
        .iter()
        .find(|(n, _)| n == "section.wgsl")
        .expect("section.wgsl is in the shader directory");
    let code = code(src);
    assert!(code.contains("var<uniform>"), "uniform block");
    assert!(
        code.contains("textureLoad"),
        "the labels are read without filtering"
    );
    for binding in [
        "@group(0) @binding(0) var<uniform> section",
        "@group(0) @binding(1) var labels: texture_2d<f32>",
        "@group(0) @binding(2) var greys: texture_2d<f32>",
    ] {
        assert!(code.contains(binding), "section.wgsl: `{binding}`");
    }
    assert_eq!(
        code.matches("textureLoad").count(),
        2,
        "one read of the labels, one helper that reads the grey image"
    );
    assert!(
        !code.contains("@binding(3)"),
        "no third resource: three bindings are what the layout declares"
    );
    for (needle, why) in [
        (
            "texture_2d<u32>",
            "integer textures are not part of the contract: the labels are R8Unorm",
        ),
        (
            "texture_2d<i32>",
            "integer textures are not part of the contract: the labels are R8Unorm",
        ),
        ("textureSample", "no filtered reads: labels are categories"),
        ("dpdx", "no derivatives"),
        ("dpdy", "no derivatives"),
        ("fwidth", "no derivatives"),
        ("sampler", "no sampler is bound"),
        ("textureDimensions", "the sizes come from the uniform block"),
    ] {
        assert!(!code.contains(needle), "section.wgsl: `{needle}`: {why}");
    }
    // the palette is a uniform array: far below the 16 KiB WebGL2 guarantees for a uniform block
    assert!(code.contains("array<vec4<f32>, 64>"));
}
