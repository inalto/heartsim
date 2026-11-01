//! Headless render tests (brief B6 acceptance). They need a Vulkan/Metal/DX12 adapter; on a box
//! without one (CI without lavapipe) every test prints a notice and returns. On the dev box:
//!
//! ```text
//! WGPU_BACKEND=vulkan VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json \
//!     cargo test -p heartsim-render --test gpu
//! ```
//!
//! `UPDATE_GOLDEN=1` rewrites `tests/golden/*.png`.

use glam::Vec3;
use heartsim_geometry::{TriMesh, primitives::icosphere};
use heartsim_render::{
    ClipPlane, Contour, GlyphInstance, LayerDesc, LayerId, LutId, ScalarMode, SceneOptions, SceneRenderer,
    best_msaa_samples, glyph, srgb_color,
};
use std::sync::{Arc, Mutex};

struct Gpu {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    errors: Arc<Mutex<Vec<String>>>,
}

fn gpu() -> Option<Gpu> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    {
        Ok(a) => a,
        Err(e) => {
            eprintln!("SKIP: no wgpu adapter ({e})");
            return None;
        }
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        .map_err(|e| eprintln!("SKIP: cannot create device ({e})"))
        .ok()?;
    let errors = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&errors);
    device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
        sink.lock().unwrap().push(e.to_string());
    }));
    Some(Gpu {
        adapter,
        device,
        queue,
        errors,
    })
}

macro_rules! gpu_or_skip {
    () => {
        match gpu() {
            Some(g) => g,
            None => return,
        }
    };
}

impl Gpu {
    fn assert_no_errors(&self) {
        let e = self.errors.lock().unwrap();
        assert!(e.is_empty(), "wgpu reported errors: {e:#?}");
    }
    fn scene(&self) -> SceneRenderer {
        SceneRenderer::new(&self.device, &self.queue, wgpu::TextureFormat::Rgba8UnormSrgb)
    }
}

struct Image {
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

impl Image {
    fn px(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.w + x) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }
    fn centre(&self) -> [u8; 4] {
        self.px(self.w / 2, self.h / 2)
    }
    /// Number of pixels that differ visibly from `bg`.
    fn count_differing(&self, bg: &Image) -> usize {
        self.rgba
            .as_chunks::<4>()
            .0
            .iter()
            .zip(bg.rgba.as_chunks::<4>().0.iter())
            .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 6))
            .count()
    }
    /// Mean absolute difference as a fraction of full scale.
    fn mean_abs_diff(&self, other: &Image) -> f64 {
        assert_eq!((self.w, self.h), (other.w, other.h));
        let sum: u64 = self
            .rgba
            .iter()
            .zip(&other.rgba)
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum();
        sum as f64 / (self.rgba.len() as f64 * 255.0)
    }
}

fn render(g: &Gpu, scene: &mut SceneRenderer, size: (u32, u32)) -> Image {
    let mut enc = g
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    scene.render_offscreen(&g.device, &g.queue, &mut enc, size);
    g.queue.submit([enc.finish()]);
    let (w, h, rgba) = scene.capture_rgba(&g.device, &g.queue).expect("rendered");
    Image { w, h, rgba }
}

fn sphere(radius: f32) -> TriMesh {
    let mut m = icosphere(4);
    for v in &mut m.vertices {
        v.position = (Vec3::from(v.position) * radius).to_array();
    }
    m
}

/// Unit sphere coloured by the V_m LUT, scalar = x mapped to 0–1 so the colour varies visibly.
fn vm_sphere(g: &Gpu, scene: &mut SceneRenderer) -> LayerId {
    let mesh = sphere(1.0);
    let mut desc = LayerDesc::new("ventricle", &mesh);
    desc.scalar_mode = ScalarMode::Lut(LutId::Voltage);
    let id = scene.add_layer(&g.device, &desc);
    let scalars: Vec<f32> = mesh.vertices.iter().map(|v| v.position[0] * 0.5 + 0.5).collect();
    scene.set_layer_scalars(&g.queue, id, &scalars);
    scene.frame_scene();
    id
}

fn load_png(bytes: &[u8]) -> Image {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .expect("decode png")
        .to_rgba8();
    Image {
        w: img.width(),
        h: img.height(),
        rgba: img.into_raw(),
    }
}

#[test]
fn golden_vm_sphere_256() {
    let g = gpu_or_skip!();
    let mut scene = g.scene();
    vm_sphere(&g, &mut scene);
    let img = render(&g, &mut scene, (256, 256));
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/sphere_vm_256.png");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(path, scene.capture_png(&g.device, &g.queue)).unwrap();
    }
    let golden = load_png(&std::fs::read(path).expect("golden PNG (run with UPDATE_GOLDEN=1 to create)"));
    let d = img.mean_abs_diff(&golden);
    assert!(d < 0.02, "mean abs diff {d:.4} ≥ 2 %");
    g.assert_no_errors();
}

#[test]
fn clip_plane_halves_visible_pixels_and_shows_cut_colour() {
    let g = gpu_or_skip!();
    let mut scene = g.scene();
    let id = vm_sphere(&g, &mut scene);
    let size = (256, 256);

    scene.layer_mut(id).unwrap().visible = false;
    let bg = render(&g, &mut scene, size);
    scene.layer_mut(id).unwrap().visible = true;
    let full = render(&g, &mut scene, size).count_differing(&bg);
    assert!(
        full > 10_000,
        "sphere should cover a good part of the frame ({full} px)"
    );

    scene.layer_mut(id).unwrap().clip = true;
    scene.clip = ClipPlane {
        enabled: true,
        point: Vec3::ZERO,
        normal: Vec3::X,
    };
    let half = render(&g, &mut scene, size).count_differing(&bg);
    let ratio = half as f64 / full as f64;
    assert!((0.45..=0.55).contains(&ratio), "clipped/unclipped = {ratio:.3}");

    // Remove the front half instead: we look into the hollow shell, which is the flat cut colour.
    scene.clip.normal = Vec3::Z;
    let inside = render(&g, &mut scene, size);
    let expected = scene.settings.cut_color.map(|c| {
        let s = if c <= 0.003_130_8 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0).round() as i32
    });
    let c = inside.centre();
    for k in 0..3 {
        assert!(
            (i32::from(c[k]) - expected[k]).abs() <= 2,
            "cut colour {c:?} vs {expected:?}"
        );
    }

    // Disabling the plane restores the full silhouette.
    scene.clip.enabled = false;
    let restored = render(&g, &mut scene, size).count_differing(&bg);
    assert_eq!(restored, full);
    g.assert_no_errors();
}

#[test]
fn pick_hits_sphere_centre_and_misses_background() {
    let g = gpu_or_skip!();
    let mut scene = g.scene();
    let id = vm_sphere(&g, &mut scene);
    render(&g, &mut scene, (256, 256));

    let hit = scene.pick((0.5, 0.5)).expect("centre ray hits the sphere");
    assert_eq!(hit.layer, id);
    assert!(
        (hit.position.length() - 1.0).abs() < 0.02,
        "|p| = {}",
        hit.position.length()
    );
    assert!(
        hit.normal.dot(scene.camera.eye() - hit.position) > 0.0,
        "normal faces the viewer"
    );
    assert!(
        hit.position.z > 0.9,
        "camera looks from +z so the near pole is hit"
    );
    assert!(scene.pick((0.02, 0.02)).is_none());

    scene.layer_mut(id).unwrap().pickable = false;
    assert!(
        scene.pick((0.5, 0.5)).is_none(),
        "non-pickable layers are transparent to picking"
    );
    scene.layer_mut(id).unwrap().pickable = true;
    scene.layer_mut(id).unwrap().visible = false;
    assert!(scene.pick((0.5, 0.5)).is_none(), "hidden layers cannot be picked");
    scene.layer_mut(id).unwrap().visible = true;

    // A clipped-away front surface is skipped; the ray lands on the back wall instead.
    scene.layer_mut(id).unwrap().clip = true;
    scene.clip = ClipPlane {
        enabled: true,
        point: Vec3::ZERO,
        normal: Vec3::Z,
    };
    let back = scene.pick((0.5, 0.5)).expect("back wall");
    assert!(back.position.z < -0.9);
}

#[test]
fn translucent_torso_still_shows_heart() {
    let g = gpu_or_skip!();
    let mut scene = g.scene();

    let heart = sphere(1.0);
    let mut hd = LayerDesc::new("heart", &heart);
    hd.base_color = srgb_color([0.9, 0.1, 0.1, 1.0])[..3].try_into().unwrap();
    scene.add_layer(&g.device, &hd);

    let torso_mesh = sphere(3.0);
    let mut td = LayerDesc::new("torso", &torso_mesh);
    td.base_color = srgb_color([0.1, 0.2, 0.9, 1.0])[..3].try_into().unwrap();
    td.alpha = 0.3;
    let torso = scene.add_layer(&g.device, &td);
    scene.frame_scene();

    let translucent = render(&g, &mut scene, (128, 128)).centre();
    assert!(
        translucent[0] > translucent[2],
        "heart (red) shows through a 0.3-alpha torso: {translucent:?}"
    );

    scene.layer_mut(torso).unwrap().alpha = 1.0;
    let opaque = render(&g, &mut scene, (128, 128)).centre();
    assert!(
        opaque[2] > opaque[0],
        "an opaque torso hides the heart: {opaque:?}"
    );

    scene.layer_mut(torso).unwrap().alpha = 0.3;
    scene.layer_mut(torso).unwrap().visible = false;
    let heart_only = render(&g, &mut scene, (128, 128)).centre();
    assert_ne!(translucent, heart_only, "the torso tints what lies behind it");
    g.assert_no_errors();
}

#[test]
fn transparent_layers_blend_back_to_front() {
    // Two concentric translucent shells: the nearer one must be composited over the farther one
    // regardless of insertion order. Compare both insertion orders; the images must match.
    let g = gpu_or_skip!();
    let build = |outer_first: bool| {
        let mut scene = g.scene();
        let (a, b) = (sphere(1.0), sphere(2.0));
        let mut inner = LayerDesc::new("inner", &a);
        inner.base_color = [1.0, 0.0, 0.0];
        inner.alpha = 0.5;
        let mut outer = LayerDesc::new("outer", &b);
        outer.base_color = [0.0, 0.0, 1.0];
        outer.alpha = 0.5;
        if outer_first {
            scene.add_layer(&g.device, &outer);
            scene.add_layer(&g.device, &inner);
        } else {
            scene.add_layer(&g.device, &inner);
            scene.add_layer(&g.device, &outer);
        }
        scene.frame_scene();
        render(&g, &mut scene, (96, 96))
    };
    let (x, y) = (build(true), build(false));
    // Both have identical centroids, so the order of equal keys may differ; allow a tiny diff.
    assert!(x.mean_abs_diff(&y) < 0.01);
    g.assert_no_errors();
}

#[test]
fn conduction_layer_is_solid_highlight_and_contour_draws_a_line() {
    let g = gpu_or_skip!();
    let mut scene = g.scene();
    let id = vm_sphere(&g, &mut scene);
    scene.layer_mut(id).unwrap().scalar_mode = ScalarMode::Lut(LutId::Conduction);
    let c = render(&g, &mut scene, (128, 128)).centre();
    assert!(c[1] > c[0] && c[2] > c[0], "green-cyan highlight, got {c:?}");

    scene.layer_mut(id).unwrap().scalar_mode = ScalarMode::Lut(LutId::Voltage);
    let plain = render(&g, &mut scene, (128, 128));
    scene.layer_mut(id).unwrap().contour = Some(Contour {
        level: 0.5,
        width_px: 3.0,
        color: [0.0, 0.0, 0.0],
    });
    let lined = render(&g, &mut scene, (128, 128));
    if let Some(dir) = std::env::var_os("HEARTSIM_DUMP_DIR") {
        let _ = std::fs::write(
            std::path::Path::new(&dir).join("contour.png"),
            scene.capture_png(&g.device, &g.queue),
        );
    }
    let dark = |img: &Image| {
        img.rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2]) < 90)
            .count()
    };
    assert!(
        dark(&lined) > dark(&plain) + 50,
        "iso-line at x = 0 adds dark pixels"
    );
    // The line is the plane x = 0: the centre column is dark, a column left of it is not.
    assert!(u32::from(lined.px(64, 64)[0]) < 60, "{:?}", lined.px(64, 64));
    assert!(u32::from(lined.px(40, 64)[0]) > 60);
    g.assert_no_errors();
}

#[test]
fn glyph_spheres_tubes_and_arrow_render_and_respect_clip() {
    let g = gpu_or_skip!();
    let mut scene = g.scene();
    scene.camera.frame(Vec3::splat(-2.0), Vec3::splat(2.0));
    let set = scene.add_glyph_set("markers");
    let red = [0.9, 0.05, 0.05, 1.0];
    scene.set_spheres(
        &g.device,
        &g.queue,
        set,
        &[GlyphInstance::sphere(Vec3::ZERO, 0.8, red)],
    );
    let bg = render(&g, &mut scene, (128, 128));
    scene.glyph_set_mut(set).unwrap().visible = false;
    let empty = render(&g, &mut scene, (128, 128));
    scene.glyph_set_mut(set).unwrap().visible = true;
    let c = bg.centre();
    assert!(c[0] > 100 && c[0] > c[1] * 3, "red sphere at the centre: {c:?}");
    assert!(bg.count_differing(&empty) > 300);

    // A tube along x, an arrow and a polyline in one go.
    let green = [0.05, 0.9, 0.05, 1.0];
    let mut tubes = vec![GlyphInstance::tube(
        Vec3::new(-1.5, 1.2, 0.0),
        Vec3::new(1.5, 1.2, 0.0),
        0.15,
        green,
    )];
    tubes.extend(glyph::arrow(
        Vec3::new(0.0, -1.5, 0.0),
        Vec3::new(1.5, 0.0, 0.0),
        0.1,
        0.3,
        green,
    ));
    let (pl_t, pl_s) = glyph::polyline(
        &[
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(-1.0, 0.0, 0.5),
            Vec3::new(-1.5, 0.5, 0.0),
        ],
        0.08,
        green,
    );
    tubes.extend(pl_t);
    scene.set_tubes(&g.device, &g.queue, set, &tubes);
    scene.set_spheres(&g.device, &g.queue, set, &pl_s);
    let with_tubes = render(&g, &mut scene, (128, 128));
    // Tube at world y = 1.2 shows up above the centre (screen y smaller).
    let (sx, sy) = scene.project_to_screen(Vec3::new(0.0, 1.2, 0.0)).unwrap();
    let p = with_tubes.px((sx * 128.0) as u32, (sy * 128.0) as u32);
    assert!(
        p[1] > p[0] && p[1] > 60,
        "green tube at its projected position: {p:?}"
    );

    // Many instances force the instance buffer to grow.
    let many: Vec<GlyphInstance> = (0..500)
        .map(|i| GlyphInstance::sphere(Vec3::new(i as f32 * 0.004 - 1.0, 0.0, 0.0), 0.02, red))
        .collect();
    scene.set_spheres(&g.device, &g.queue, set, &many);
    assert_eq!(scene.glyph_sets()[0].sphere_count(), 500);
    render(&g, &mut scene, (128, 128));

    // Clipped glyph set loses the sphere half on the removed side.
    scene.set_spheres(
        &g.device,
        &g.queue,
        set,
        &[GlyphInstance::sphere(Vec3::ZERO, 0.8, red)],
    );
    scene.set_tubes(&g.device, &g.queue, set, &[]);
    let full = render(&g, &mut scene, (128, 128)).count_differing(&empty);
    scene.glyph_set_mut(set).unwrap().clip = true;
    scene.clip = ClipPlane {
        enabled: true,
        point: Vec3::ZERO,
        normal: Vec3::X,
    };
    let half = render(&g, &mut scene, (128, 128)).count_differing(&empty);
    let ratio = half as f64 / full as f64;
    assert!((0.4..=0.6).contains(&ratio), "{ratio}");
    g.assert_no_errors();
}

#[test]
fn labels_project_and_follow_clip_and_visibility() {
    let g = gpu_or_skip!();
    let mut scene = g.scene();
    let id = vm_sphere(&g, &mut scene);
    render(&g, &mut scene, (200, 100));
    scene.layer_mut(id).unwrap().label = Some("LV".into());
    scene.layer_mut(id).unwrap().label_anchor = Vec3::ZERO;
    let labels = scene.label_anchors();
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].text, "LV");
    assert!((labels[0].u - 0.5).abs() < 1e-3 && (labels[0].v - 0.5).abs() < 1e-3);
    let (u, _) = scene.project_to_screen(Vec3::new(0.5, 0.0, 0.0)).unwrap();
    assert!(
        u > 0.5,
        "patient-left (+x) appears right of centre from the anterior view"
    );

    scene.layer_mut(id).unwrap().visible = false;
    assert_eq!(scene.label_anchors(), vec![]);
    scene.layer_mut(id).unwrap().visible = true;
    scene.layer_mut(id).unwrap().clip = true;
    scene.clip = ClipPlane {
        enabled: true,
        point: Vec3::new(-0.5, 0.0, 0.0),
        normal: Vec3::X,
    };
    assert!(
        scene.label_anchors().is_empty(),
        "anchor on the cut-away side is hidden"
    );
}

#[test]
fn screenshot_unpads_rows_and_roundtrips_png() {
    let g = gpu_or_skip!();
    let mut scene = g.scene();
    vm_sphere(&g, &mut scene);
    // 100 px × 4 B = 400 B per row → padded to 512 on the GPU side.
    let png = scene.render_to_png(&g.device, &g.queue, (100, 60));
    let img = load_png(&png);
    assert_eq!((img.w, img.h), (100, 60));
    let direct = render(&g, &mut scene, (100, 60));
    assert_eq!(img.rgba, direct.rgba, "PNG pixels equal the raw read-back");
    // Top-left is the background gradient's top colour, bottom-left is darker.
    assert!(u32::from(img.px(0, 0)[2]) > u32::from(img.px(0, 59)[2]));
    assert!(scene.capture_png(&g.device, &g.queue).len() > 100);
    g.assert_no_errors();
}

#[test]
fn msaa_matches_single_sample_and_smooths_edges() {
    let g = gpu_or_skip!();
    if best_msaa_samples(&g.adapter) < 4 {
        eprintln!("SKIP: adapter has no 4x MSAA");
        return;
    }
    let mut ms = SceneRenderer::with_options(
        &g.device,
        &g.queue,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        SceneOptions { msaa_samples: 4 },
    );
    assert_eq!(ms.msaa_samples(), 4);
    let mut ss = g.scene();
    vm_sphere(&g, &mut ms);
    vm_sphere(&g, &mut ss);
    let a = render(&g, &mut ms, (128, 128));
    let b = render(&g, &mut ss, (128, 128));
    assert!(a.mean_abs_diff(&b) < 0.02);
    // More distinct colours along the silhouette means anti-aliasing happened.
    let distinct = |i: &Image| {
        let mut s = std::collections::HashSet::new();
        for p in i.rgba.as_chunks::<4>().0 {
            s.insert([p[0], p[1], p[2]]);
        }
        s.len()
    };
    assert!(distinct(&a) > distinct(&b));
    g.assert_no_errors();
}

#[test]
fn hundred_frames_without_validation_errors() {
    let g = gpu_or_skip!();
    let mut scene = g.scene();
    let heart = sphere(1.0);
    let mut hd = LayerDesc::new("heart", &heart);
    hd.scalar_mode = ScalarMode::Lut(LutId::Voltage);
    let id = scene.add_layer(&g.device, &hd);
    let torso_mesh = sphere(3.0);
    let mut td = LayerDesc::new("torso", &torso_mesh);
    td.alpha = 0.3;
    let torso = scene.add_layer(&g.device, &td);
    scene.layer_mut(torso).unwrap().pickable = false;
    scene.frame_scene();
    let set = scene.add_glyph_set("electrodes");
    scene.set_spheres(
        &g.device,
        &g.queue,
        set,
        &[GlyphInstance::sphere(Vec3::new(2.0, 0.0, 0.0), 0.1, [1.0; 4])],
    );
    scene.set_tubes(
        &g.device,
        &g.queue,
        set,
        &glyph::arrow(
            Vec3::ZERO,
            Vec3::new(0.0, 1.0, 0.5),
            0.05,
            0.3,
            [1.0, 1.0, 0.0, 1.0],
        ),
    );
    scene.layer_mut(id).unwrap().clip = true;
    scene.layer_mut(id).unwrap().contour = Some(Contour::default());

    let mut scalars = vec![0.0f32; heart.vertices.len()];
    for frame in 0..100u32 {
        for (i, s) in scalars.iter_mut().enumerate() {
            *s = ((i as u32 + frame * 7) % 100) as f32 / 100.0;
        }
        scene.set_layer_scalars(&g.queue, id, &scalars);
        scene.camera.orbit(3.0, 0.5);
        scene.clip = ClipPlane {
            enabled: frame % 2 == 0,
            point: Vec3::ZERO,
            normal: Vec3::new(1.0, 0.3, 0.2),
        };
        if frame == 50 {
            // Adding layers mid-run regrows the dynamic-offset uniform buffer.
            let extra = sphere(0.2);
            for k in 0..20 {
                let mut d = LayerDesc::new(format!("extra{k}"), &extra);
                d.alpha = if k % 2 == 0 { 1.0 } else { 0.5 };
                scene.add_layer(&g.device, &d);
            }
        }
        let size = if frame < 60 { (96, 64) } else { (130, 77) };
        let mut enc = g
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        scene.render_offscreen(&g.device, &g.queue, &mut enc, size);
        g.queue.submit([enc.finish()]);
    }
    g.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(scene.layers().len(), 22);
    g.assert_no_errors();
}

#[test]
fn legacy_single_mesh_api_still_works() {
    let g = gpu_or_skip!();
    let mut scene = g.scene();
    let mesh = icosphere(3);
    scene.set_mesh(&g.device, &mesh);
    scene.set_scalars(&g.queue, &vec![0.7; mesh.vertices.len()]);
    let a = render(&g, &mut scene, (64, 64)).centre();
    // Replacing the mesh swaps the primary layer rather than stacking another one.
    scene.set_mesh(&g.device, &mesh);
    assert_eq!(scene.layers().len(), 1);
    assert!(a[0] > a[2], "0.7 on the voltage LUT is warm: {a:?}");
    g.assert_no_errors();
}

#[test]
fn default_anatomy_pack_renders_as_a_layered_scene() {
    use heartsim_geometry::{AnatomyPack, MeshKind};
    let g = gpu_or_skip!();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/anatomy/default");
    let pack = AnatomyPack::load(std::path::Path::new(dir)).expect("default pack");
    let mut scene = g.scene();
    let mut myo = Vec::new();
    for m in &pack.meshes {
        let mut d = LayerDesc::new(m.name.clone(), &m.mesh);
        match m.kind {
            MeshKind::Myocardium => d.scalar_mode = ScalarMode::Lut(LutId::Voltage),
            MeshKind::Vessel => d.base_color = srgb_color([0.55, 0.2, 0.25, 1.0])[..3].try_into().unwrap(),
            MeshKind::Valve => d.base_color = [0.8, 0.75, 0.6],
        }
        let id = scene.add_layer(&g.device, &d);
        if m.kind == MeshKind::Myocardium {
            let s: Vec<f32> = m
                .mesh
                .vertices
                .iter()
                .map(|v| (v.position[1] + 60.0) / 120.0)
                .collect();
            scene.set_layer_scalars(&g.queue, id, &s);
            scene.layer_mut(id).unwrap().clip = true;
            myo.push(id);
        }
    }
    let mut td = LayerDesc::new("torso", &pack.torso);
    td.alpha = 0.12;
    td.base_color = [0.7, 0.55, 0.45];
    let torso = scene.add_layer(&g.device, &td);
    scene.layer_mut(torso).unwrap().pickable = false;
    scene.frame_scene();
    scene.camera.yaw = 0.5;

    let electrodes = scene.add_glyph_set("electrodes");
    let spheres: Vec<GlyphInstance> = pack
        .electrodes
        .sites
        .values()
        .map(|p| GlyphInstance::sphere(Vec3::from(*p), 6.0, [0.9, 0.9, 0.1, 1.0]))
        .collect();
    scene.set_spheres(&g.device, &g.queue, electrodes, &spheres);
    let arrow = scene.add_glyph_set("heart vector");
    scene.set_tubes(
        &g.device,
        &g.queue,
        arrow,
        &glyph::arrow(
            Vec3::ZERO,
            Vec3::new(40.0, -30.0, 20.0),
            2.0,
            0.3,
            [0.1, 0.9, 0.9, 1.0],
        ),
    );

    let img = render(&g, &mut scene, (384, 384));
    if let Some(dir) = std::env::var_os("HEARTSIM_DUMP_DIR") {
        let _ = std::fs::write(
            std::path::Path::new(&dir).join("pack.png"),
            scene.capture_png(&g.device, &g.queue),
        );
    }
    // Aim at the heart through the translucent torso: the pick skips the torso layer.
    let c = myo
        .iter()
        .map(|id| scene.layer(*id).unwrap().mesh.centroid())
        .sum::<Vec3>()
        / myo.len() as f32;
    let uv = scene.project_to_screen(c).expect("in front of the camera");
    let hit = scene.pick(uv).expect("a heart surface under the heart centre");
    assert_ne!(hit.layer, torso, "torso is not pickable");
    assert!(
        img.centre() != img.px(2, 2),
        "scene differs from the bare background"
    );
    g.assert_no_errors();
}
