//! The echo page's image on screen (briefs B31 §12 and B32): a polar **label image** (lines ×
//! samples of one tissue-label byte each, bit 7 set where the sample lies behind bone or lung)
//! and a polar **grey image** (the same layout, one display brightness per sample) scan-converted
//! by one fragment shader into a sector, apex up, index-marker side on the right. A
//! [`SectionMode`] picks the grey image, the flat palette colours of the labels (an anatomical
//! cross-section) or the colours blended over the grey image. The grey image is interpolated
//! bilinearly between the sample centres (by hand: no sampler), so the speckle is not blocky.
//!
//! [`SectionLayout`] is the geometry both the shader and the hover read-out use, so the pixel the
//! pointer is over and the sample the shader drew there cannot disagree. [`SectionRenderer`] owns
//! the pipeline, the two textures and the uniform block; it draws a full-screen triangle into the
//! caller's render pass (egui's), whose viewport and scissor are already the widget rectangle.
//! WebGL2-safe: no compute, no storage buffers, no filtering, `textureLoad` on `R8Unorm`
//! textures (`tests/webgl2_shaders.rs` checks the source).

use bytemuck::{Pod, Zeroable};
use glam::Vec2;

/// Palette slots of the shader: labels are `0..64` (the stored labels end at 34).
pub const PALETTE_SLOTS: usize = 64;
/// Default opacity of the colours over the grey image in [`SectionMode::Both`].
pub const DEFAULT_OVERLAY: f32 = 0.45;
/// Bit of a label texel set where the sample is hidden behind bone or lung.
pub const HIDDEN_BIT: u8 = 0x80;

/// What the sector shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SectionMode {
    /// Flat palette colours of the tissue labels (B31's anatomical cross-section).
    #[default]
    Anatomy,
    /// The grey image alone (B32's simulated B-mode appearance).
    Image,
    /// The colours blended over the grey image.
    Both,
}

impl SectionMode {
    fn code(self) -> f32 {
        match self {
            Self::Anatomy => 0.0,
            Self::Image => 1.0,
            Self::Both => 2.0,
        }
    }
}

/// Where the sector sits in its widget, and the scale: every length in logical pixels of the
/// widget, y down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectionLayout {
    /// Widget size, px.
    pub size_px: Vec2,
    /// The sector apex (the skin point), px from the widget's top left.
    pub apex_px: Vec2,
    /// Pixels per millimetre of depth.
    pub px_per_mm: f32,
    /// Imaging depth, mm.
    pub depth_mm: f32,
    /// Sector width, degrees.
    pub sector_deg: f32,
}

impl SectionLayout {
    /// The largest sector that fits `size_px` with `margin_px` (left, top, right, bottom) free,
    /// centred on the beam axis, apex at the top.
    #[must_use]
    pub fn fit(size_px: Vec2, depth_mm: f32, sector_deg: f32, margin_px: [f32; 4]) -> Self {
        let depth_mm = depth_mm.max(1.0);
        let half = (sector_deg.clamp(1.0, 179.0) * 0.5).to_radians();
        // the arc reaches `depth·sin(half)` to each side (the whole depth when half ≥ 90°)
        let half_width_mm = depth_mm
            * if half >= std::f32::consts::FRAC_PI_2 {
                1.0
            } else {
                half.sin()
            };
        let avail_w = (size_px.x - margin_px[0] - margin_px[2]).max(8.0);
        let avail_h = (size_px.y - margin_px[1] - margin_px[3]).max(8.0);
        let px_per_mm = (avail_w / (2.0 * half_width_mm))
            .min(avail_h / depth_mm)
            .max(1e-3);
        Self {
            size_px,
            apex_px: Vec2::new(margin_px[0] + avail_w * 0.5, margin_px[1]),
            px_per_mm,
            depth_mm,
            sector_deg,
        }
    }

    /// Half the sector angle, radians.
    #[must_use]
    pub fn half_angle(&self) -> f32 {
        (self.sector_deg.clamp(1.0, 179.0) * 0.5).to_radians()
    }

    /// The screen point of the scan line at fractional index `line_f` (0 = left edge of the
    /// first line, `lines` = right edge of the last) at depth `depth_mm`.
    #[must_use]
    pub fn point_of(&self, line_f: f32, lines: usize, depth_mm: f32) -> Vec2 {
        let half = self.half_angle();
        let theta = (line_f / lines.max(1) as f32 * 2.0 - 1.0) * half;
        self.apex_px + Vec2::new(theta.sin(), theta.cos()) * (depth_mm * self.px_per_mm)
    }

    /// The `(line, sample)` of the label image under the widget point `p_px`, with its depth in
    /// mm, or `None` outside the sector. Exactly the shader's mapping.
    #[must_use]
    pub fn locate(&self, p_px: Vec2, lines: usize, samples: usize) -> Option<(usize, usize, f32)> {
        let d = p_px - self.apex_px;
        if d.y <= 0.0 || lines == 0 || samples == 0 {
            return None;
        }
        let r_px = d.length();
        let depth_px = self.depth_mm * self.px_per_mm;
        let half = self.half_angle();
        let theta = d.x.atan2(d.y);
        if r_px > depth_px || theta.abs() > half {
            return None;
        }
        let line_f = ((theta + half) / (2.0 * half)).clamp(0.0, 0.999_999) * lines as f32;
        let sample_f = (r_px / depth_px).clamp(0.0, 0.999_999) * samples as f32;
        Some((line_f as usize, sample_f as usize, r_px / self.px_per_mm))
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SectionUniform {
    geom: [f32; 4],
    view: [f32; 4],
    tex: [f32; 4],
    grey: [f32; 4],
    palette: [[f32; 4]; PALETTE_SLOTS],
}

/// One `R8Unorm` image of `lines × samples` texels (line-major, samples along a row).
struct ImageTexture {
    lines: u32,
    samples: u32,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl ImageTexture {
    fn new(device: &wgpu::Device, label: &str, lines: u32, samples: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: samples,
                height: lines,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            lines,
            samples,
            texture,
            view,
        }
    }

    /// Write `texels` (padded with zeros, or cut, to the image size).
    fn write(&self, queue: &wgpu::Queue, texels: &[u8]) {
        let want = self.lines as usize * self.samples as usize;
        let data: std::borrow::Cow<'_, [u8]> = if texels.len() >= want {
            std::borrow::Cow::Borrowed(&texels[..want])
        } else {
            let mut v = texels.to_vec();
            v.resize(want, 0);
            std::borrow::Cow::Owned(v)
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.samples),
                rows_per_image: Some(self.lines),
            },
            wgpu::Extent3d {
                width: self.samples,
                height: self.lines,
                depth_or_array_layers: 1,
            },
        );
    }
}

/// Pipeline, label texture and uniforms of the cross-section. One per window.
pub struct SectionRenderer {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniforms: wgpu::Buffer,
    /// The label image and the grey image (either may be missing), and a 1 × 1 stand-in bound for
    /// a missing one.
    labels: Option<ImageTexture>,
    grey: Option<ImageTexture>,
    stand_in: ImageTexture,
    bind_group: Option<wgpu::BindGroup>,
    mode: SectionMode,
    overlay: f32,
    /// The target is sRGB: the shader's colours are linear and the hardware encodes them.
    srgb_target: bool,
    palette: [[f32; 4]; PALETTE_SLOTS],
    state: SectionUniform,
}

impl std::fmt::Debug for SectionRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SectionRenderer")
            .field("srgb_target", &self.srgb_target)
            .field("mode", &self.mode)
            .field("labels", &self.labels.as_ref().map(|t| (t.lines, t.samples)))
            .field("grey", &self.grey.as_ref().map(|t| (t.lines, t.samples)))
            .finish_non_exhaustive()
    }
}

impl SectionRenderer {
    /// A renderer drawing into passes of `target_format`.
    #[must_use]
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/section.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("heartsim section layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("heartsim section pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("heartsim section pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let state = SectionUniform::zeroed();
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("heartsim section uniforms"),
            size: size_of::<SectionUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            layout,
            uniforms,
            labels: None,
            grey: None,
            stand_in: ImageTexture::new(device, "heartsim section stand-in", 1, 1),
            bind_group: None,
            mode: SectionMode::default(),
            overlay: DEFAULT_OVERLAY,
            srgb_target: target_format.is_srgb(),
            palette: [[0.0; 4]; PALETTE_SLOTS],
            state,
        }
    }

    /// The palette: one sRGB colour per label byte (slots beyond the slice are black). Converted
    /// to what the target needs (linear for an sRGB target).
    pub fn set_palette(&mut self, srgb: &[[u8; 3]]) {
        for (i, slot) in self.palette.iter_mut().enumerate() {
            let c = srgb.get(i).copied().unwrap_or([0, 0, 0]);
            let f = |v: u8| {
                let v = f32::from(v) / 255.0;
                if self.srgb_target {
                    crate::layer::srgb_to_linear(v)
                } else {
                    v
                }
            };
            *slot = [f(c[0]), f(c[1]), f(c[2]), 1.0];
        }
    }

    /// Upload the label image: `texels` is `lines × samples` bytes, line-major, label in the low
    /// seven bits and [`HIDDEN_BIT`] set where hidden. Recreates the texture when the size
    /// changes. `texels` shorter than the image leaves the rest black.
    pub fn set_image(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        lines: u32,
        samples: u32,
        texels: &[u8],
    ) {
        let (lines, samples) = (lines.max(1), samples.max(1));
        if self
            .labels
            .as_ref()
            .is_none_or(|t| t.lines != lines || t.samples != samples)
        {
            self.labels = Some(ImageTexture::new(
                device,
                "heartsim section labels",
                lines,
                samples,
            ));
            self.bind_group = None;
        }
        if let Some(t) = &self.labels {
            t.write(queue, texels);
        }
        self.rebuild_bind_group(device);
    }

    /// Upload the grey image: `pixels` is `lines × samples` display brightnesses (0 = black,
    /// 255 = white, perceptual), line-major like the label image. Recreates the texture when the
    /// size changes; shorter data leaves the rest black.
    pub fn set_grey(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        lines: u32,
        samples: u32,
        pixels: &[u8],
    ) {
        let (lines, samples) = (lines.max(1), samples.max(1));
        if self
            .grey
            .as_ref()
            .is_none_or(|t| t.lines != lines || t.samples != samples)
        {
            self.grey = Some(ImageTexture::new(device, "heartsim section grey", lines, samples));
            self.bind_group = None;
        }
        if let Some(t) = &self.grey {
            t.write(queue, pixels);
        }
        self.rebuild_bind_group(device);
    }

    fn rebuild_bind_group(&mut self, device: &wgpu::Device) {
        if self.bind_group.is_some() {
            return;
        }
        let view_of = |t: &Option<ImageTexture>| -> wgpu::TextureView {
            t.as_ref()
                .map_or_else(|| self.stand_in.view.clone(), |t| t.view.clone())
        };
        let (labels, grey) = (view_of(&self.labels), view_of(&self.grey));
        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("heartsim section bind group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&labels),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&grey),
                },
            ],
        }));
    }

    /// What the sector shows, and (mode [`SectionMode::Both`]) how opaque the colours are over the
    /// grey image (`0..=1`). Takes effect with the next [`Self::set_layout`].
    pub fn set_mode(&mut self, mode: SectionMode, overlay_opacity: f32) {
        self.mode = mode;
        self.overlay = overlay_opacity.clamp(0.0, 1.0);
    }

    /// The mode as drawn: a mode that needs an image that was never uploaded falls back to the
    /// one that does not (colours over a missing grey image are the colours alone).
    fn effective_mode(&self) -> Option<SectionMode> {
        match (self.mode, self.labels.is_some(), self.grey.is_some()) {
            (SectionMode::Image, _, true) | (SectionMode::Both, false, true) => Some(SectionMode::Image),
            (SectionMode::Both, true, true) => Some(SectionMode::Both),
            (SectionMode::Both | SectionMode::Anatomy, true, _) => Some(SectionMode::Anatomy),
            _ => None,
        }
    }

    /// Set where and how the sector is drawn this frame (`hidden_brightness` scales the colour of
    /// hidden samples, `hatch_px` is the period of their diagonal hatch).
    pub fn set_layout(
        &mut self,
        queue: &wgpu::Queue,
        layout: &SectionLayout,
        hidden_brightness: f32,
        hatch_px: f32,
    ) {
        let (lines, samples) = self.labels.as_ref().map_or((1, 1), |t| (t.lines, t.samples));
        let (g_lines, g_samples) = self.grey.as_ref().map_or((1, 1), |t| (t.lines, t.samples));
        let mode = self.effective_mode().unwrap_or(self.mode);
        self.state = SectionUniform {
            geom: [
                layout.apex_px.x,
                layout.apex_px.y,
                layout.px_per_mm,
                layout.depth_mm,
            ],
            view: [layout.size_px.x, layout.size_px.y, layout.half_angle(), hatch_px],
            tex: [lines as f32, samples as f32, hidden_brightness, mode.code()],
            grey: [
                g_lines as f32,
                g_samples as f32,
                self.overlay,
                f32::from(u8::from(self.srgb_target)),
            ],
            palette: self.palette,
        };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&self.state));
    }

    /// Draw the sector into `pass` (viewport and scissor already the widget rectangle). Nothing
    /// until the image the mode needs was uploaded ([`Self::set_image`], [`Self::set_grey`]).
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if let (Some(bind_group), Some(_)) = (&self.bind_group, self.effective_mode()) {
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> SectionLayout {
        SectionLayout::fit(Vec2::new(400.0, 300.0), 160.0, 75.0, [10.0, 10.0, 10.0, 10.0])
    }

    #[test]
    fn the_sector_fits_inside_the_widget() {
        let l = layout();
        let half = l.half_angle();
        for line in [0.0, 64.0, 128.0] {
            for d in [0.0, 80.0, 160.0] {
                let p = l.point_of(line, 128, d);
                assert!(p.x >= 9.9 && p.x <= 390.1 && p.y >= 9.9 && p.y <= 290.1, "{p:?}");
            }
        }
        assert!((half.to_degrees() - 37.5).abs() < 1e-4);
        assert!((l.apex_px.x - 200.0).abs() < 1e-4, "centred");
    }

    #[test]
    fn locate_inverts_point_of_and_marker_side_is_the_right() {
        let l = layout();
        // the middle of the last line (marker side) is on the right of the apex
        let p = l.point_of(127.5, 128, 100.0);
        assert!(p.x > l.apex_px.x);
        let (line, sample, depth) = l.locate(p, 128, 512).expect("inside");
        assert_eq!(line, 127);
        assert!((depth - 100.0).abs() < 0.5, "{depth}");
        assert_eq!(sample, (100.0_f32 / 160.0 * 512.0) as usize);
        let first = l.point_of(0.5, 128, 100.0);
        assert!(first.x < l.apex_px.x);
        assert_eq!(l.locate(first, 128, 512).map(|(i, _, _)| i), Some(0));
    }

    #[test]
    fn outside_the_sector_is_none() {
        let l = layout();
        assert!(l.locate(Vec2::new(0.0, 299.0), 128, 512).is_none(), "corner");
        assert!(
            l.locate(l.apex_px - Vec2::new(0.0, 5.0), 128, 512).is_none(),
            "above the apex"
        );
        assert!(
            l.locate(l.apex_px + Vec2::new(0.0, 5.0), 128, 512).is_some(),
            "below the apex"
        );
        let beyond = l.apex_px + Vec2::new(0.0, l.depth_mm * l.px_per_mm + 3.0);
        assert!(l.locate(beyond, 128, 512).is_none());
    }

    #[test]
    fn a_wide_shallow_sector_is_limited_by_the_width() {
        let l = SectionLayout::fit(Vec2::new(200.0, 800.0), 100.0, 90.0, [0.0; 4]);
        let reach = l.depth_mm * 45.0_f32.to_radians().sin() * l.px_per_mm;
        assert!((reach - 100.0).abs() < 0.01, "{reach}");
    }
}
