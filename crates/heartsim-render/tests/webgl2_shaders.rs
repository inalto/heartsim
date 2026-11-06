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
