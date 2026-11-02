//! Texture read-back for screenshots and tests.

/// Copy `texture` (`Rgba8*`, `COPY_SRC`) into tightly packed RGBA bytes. Blocks until the GPU is
/// done, so it is for screenshots and tests, not for per-frame use. wgpu requires
/// `bytes_per_row` to be a multiple of 256, so rows are padded on the GPU side and unpadded here.
#[cfg(not(target_arch = "wasm32"))]
pub fn read_rgba8(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let (w, h) = (texture.width(), texture.height());
    let unpadded = w * 4;
    let padded = unpadded.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("heartsim screenshot readback"),
        size: u64::from(padded) * u64::from(h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("heartsim screenshot copy"),
    });
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("device poll while reading back the screenshot");
    rx.recv()
        .expect("map callback")
        .expect("map the screenshot buffer");
    let data = slice.get_mapped_range().expect("mapped range");
    let mut out = Vec::with_capacity((unpadded * h) as usize);
    for row in data.chunks_exact(padded as usize) {
        out.extend_from_slice(&row[..unpadded as usize]);
    }
    drop(data);
    buffer.unmap();
    out
}

/// Encode tightly packed RGBA8 pixels as a PNG file.
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .expect("PNG encoding of an in-memory RGBA buffer cannot fail");
    out
}
