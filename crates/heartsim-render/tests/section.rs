//! GPU smoke test of the echo cross-section's scan-conversion shader (brief B31 §12). Needs a
//! Vulkan/Metal/DX12 adapter; without one (no lavapipe) it prints a notice and returns, like
//! `gpu.rs`:
//!
//! ```text
//! WGPU_BACKEND=vulkan VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json \
//!     cargo test -p heartsim-render --test section
//! ```
//!
//! A uniform label image is drawn as a sector: the pixel column under the apex is inside the mask,
//! a corner is outside it, a hidden sample is dimmer than a visible one, and the marker side is the
//! right.

use glam::Vec2;
use heartsim_render::{SectionLayout, SectionMode, SectionRenderer};

const W: u32 = 128;
const H: u32 = 96;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const CLEAR: [u8; 4] = [0, 0, 255, 255];

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    {
        Ok(a) => a,
        Err(e) => {
            eprintln!("SKIP: no wgpu adapter ({e})");
            return None;
        }
    };
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        .map_err(|e| eprintln!("SKIP: cannot create device ({e})"))
        .ok()
}

/// Draw `texels` (`lines × samples`) with `palette` and read the frame back as RGBA8.
fn draw(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &SectionLayout,
    lines: u32,
    samples: u32,
    texels: &[u8],
    palette: &[[u8; 3]],
) -> Vec<u8> {
    draw_with(device, queue, layout, |r| {
        r.set_palette(palette);
        r.set_image(device, queue, lines, samples, texels);
    })
}

/// Let `setup` fill a renderer, set the layout, draw one frame and read it back as RGBA8.
fn draw_with(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &SectionLayout,
    setup: impl FnOnce(&mut SectionRenderer),
) -> Vec<u8> {
    let mut r = SectionRenderer::new(device, FORMAT);
    setup(&mut r);
    r.set_layout(queue, layout, 0.5, 6.0);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("section test target"),
        size: wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("section test readback"),
        size: u64::from(W * 4 * H),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("section test pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 1.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        r.draw(&mut pass);
    }
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(W * 4),
                rows_per_image: Some(H),
            },
        },
        wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([enc.finish()]);
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    rx.recv().expect("map callback").expect("map");
    let data = slice.get_mapped_range().expect("mapped range").to_vec();
    buffer.unmap();
    data
}

fn px(img: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

#[test]
fn the_sector_is_drawn_apex_up_with_a_mask() {
    let Some((device, queue)) = device() else { return };
    let layout = SectionLayout::fit(Vec2::new(W as f32, H as f32), 160.0, 75.0, [4.0, 4.0, 4.0, 4.0]);
    let (lines, samples) = (16u32, 32u32);
    // the left half of the lines label 5 (red), the right half label 6 (green); the deepest
    // quarter of every line is hidden
    let mut texels = vec![0u8; (lines * samples) as usize];
    for l in 0..lines {
        for s in 0..samples {
            let label = if l < lines / 2 { 5u8 } else { 6u8 };
            texels[(l * samples + s) as usize] = if s >= samples * 3 / 4 { label | 0x80 } else { label };
        }
    }
    let mut palette = vec![[0u8, 0, 0]; 64];
    palette[5] = [255, 0, 0];
    palette[6] = [0, 255, 0];
    let img = draw(&device, &queue, &layout, lines, samples, &texels, &palette);
    let ax = layout.apex_px.x as u32;
    let ay = layout.apex_px.y as u32;
    // the apex pixel column, a little below the apex: inside the mask (not the clear colour)
    let inside = px(&img, ax, ay + 12);
    assert_ne!(inside, CLEAR, "apex column is inside the sector: {inside:?}");
    // a corner of the widget: outside, the clear colour untouched
    assert_eq!(px(&img, 0, H - 1), CLEAR, "bottom-left corner is outside");
    assert_eq!(px(&img, W - 1, H - 1), CLEAR, "bottom-right corner is outside");
    assert_eq!(px(&img, 0, 0), CLEAR, "top-left corner is outside");
    // above the apex there is nothing
    assert_eq!(px(&img, ax, ay.saturating_sub(2)), CLEAR);
    // the marker side (the last lines) is the right of the screen: green there, red on the left
    let right = layout.point_of(14.0, lines as usize, 60.0);
    let left = layout.point_of(2.0, lines as usize, 60.0);
    let (r, l) = (
        px(&img, right.x as u32, right.y as u32),
        px(&img, left.x as u32, left.y as u32),
    );
    assert!(r[1] > 200 && r[0] < 60, "right is label 6 (green): {r:?}");
    assert!(l[0] > 200 && l[1] < 60, "left is label 5 (red): {l:?}");
    // a hidden sample is dimmer than a visible one of the same label
    let hidden = layout.point_of(2.0, lines as usize, 150.0);
    let h = px(&img, hidden.x as u32, hidden.y as u32);
    assert!(h[0] < l[0], "hidden {h:?} is dimmer than visible {l:?}");
    assert_ne!(h, CLEAR);
}

/// The grey image alone, and the colours over it (briefs B32): a left half of black lines and a
/// right half of white ones is drawn in each mode.
#[test]
fn the_grey_image_is_drawn_alone_and_under_the_colours() {
    let Some((device, queue)) = device() else { return };
    let layout = SectionLayout::fit(Vec2::new(W as f32, H as f32), 160.0, 75.0, [4.0, 4.0, 4.0, 4.0]);
    let (lines, samples) = (16u32, 32u32);
    let mut grey = vec![0u8; (lines * samples) as usize];
    let mut labels = vec![0u8; (lines * samples) as usize];
    for l in 0..lines {
        for s in 0..samples {
            let i = (l * samples + s) as usize;
            grey[i] = if l < lines / 2 { 40 } else { 220 };
            // label 5 (red) everywhere but the deepest quarter, which is label 0 (no tissue)
            labels[i] = if s >= samples * 3 / 4 { 0 } else { 5 };
        }
    }
    let mut palette = vec![[0u8, 0, 0]; 64];
    palette[5] = [255, 0, 0];
    let run = |mode: SectionMode, with_labels: bool| {
        draw_with(&device, &queue, &layout, |r| {
            r.set_palette(&palette);
            if with_labels {
                r.set_image(&device, &queue, lines, samples, &labels);
            }
            r.set_grey(&device, &queue, lines, samples, &grey);
            r.set_mode(mode, 0.5);
        })
    };
    let left = layout.point_of(3.0, lines as usize, 50.0);
    let right = layout.point_of(13.0, lines as usize, 50.0);
    let deep_right = layout.point_of(13.0, lines as usize, 150.0);
    let at = |img: &[u8], p: glam::Vec2| px(img, p.x as u32, p.y as u32);

    // the grey image alone: grey channels equal, dark on the left, light on the right (the target
    // is sRGB, so the stored brightness comes back out unchanged to within rounding)
    let img = run(SectionMode::Image, false);
    let (l, r) = (at(&img, left), at(&img, right));
    assert!(l[0] == l[1] && l[1] == l[2], "grey: {l:?}");
    assert!((i32::from(l[0]) - 40).abs() <= 3, "left is 40: {l:?}");
    assert!((i32::from(r[0]) - 220).abs() <= 3, "right is 220: {r:?}");
    assert_eq!(px(&img, 0, H - 1), CLEAR, "outside the sector nothing is drawn");

    // the colours alone ignore the grey image
    let img = run(SectionMode::Anatomy, true);
    let r = at(&img, right);
    assert!(r[0] > 200 && r[1] < 40, "anatomy is the label's red: {r:?}");

    // both: the colour over the grey (red raised, green and blue pulled toward the grey), and the
    // grey alone where the label is 0
    let img = run(SectionMode::Both, true);
    let (l, r, d) = (at(&img, left), at(&img, right), at(&img, deep_right));
    assert!(l[0] > l[1] + 30 && r[0] > r[1] + 30, "red over grey: {l:?} {r:?}");
    assert!(r[1] > l[1] + 40, "the lighter grey shows through: {l:?} {r:?}");
    assert!(
        d[0] == d[1] && d[1] == d[2] && d[0] > 200,
        "no tissue: the grey alone {d:?}"
    );

    // a mode that needs an image that was not uploaded draws nothing (image) or the colours (both)
    let img = run(SectionMode::Image, false);
    assert!(at(&img, right)[0] > 200);
    let none = draw_with(&device, &queue, &layout, |r| {
        r.set_palette(&palette);
        r.set_image(&device, &queue, lines, samples, &labels);
        r.set_mode(SectionMode::Image, 0.5);
    });
    assert_eq!(at(&none, right), CLEAR, "the grey image was never uploaded");
}
