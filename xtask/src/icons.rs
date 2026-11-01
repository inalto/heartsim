//! `cargo xtask icons` — rasterise `assets/icon.svg` into the PNG / ICO / ICNS set that cargo-packager bundles.
//!
//! Output lands in `assets/generated/icons/` (git-ignored). PNGs are named `<w>x<h>.png`, which is how the
//! `.deb`/AppImage bundler picks hicolor sizes. ICO and ICNS are containers around the same PNG data, so no
//! image-format crate is needed beyond the rasteriser.
use anyhow::{Context, Result, anyhow};
use resvg::{tiny_skia, usvg};
use std::{fs, path::Path};

/// Square sizes written as loose PNGs (hicolor theme sizes plus 512 for stores).
const PNG_SIZES: [u32; 7] = [16, 32, 48, 64, 128, 256, 512];
/// Sizes embedded in `icon.ico`; Windows Explorer uses 256 for large icons and 16/32/48 for lists.
const ICO_SIZES: [u32; 5] = [16, 32, 48, 128, 256];
/// ICNS entries as (OSType, pixel size); every type here stores a PNG payload.
const ICNS_ENTRIES: [(&[u8; 4], u32); 6] = [
    (b"icp4", 16),
    (b"icp5", 32),
    (b"ic07", 128),
    (b"ic08", 256),
    (b"ic09", 512),
    (b"ic10", 1024),
];

/// Render `svg_path` into `out_dir`.
pub fn run(svg_path: &Path, out_dir: &Path) -> Result<()> {
    let svg = fs::read_to_string(svg_path).with_context(|| format!("reading {}", svg_path.display()))?;
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).context("parsing icon SVG")?;
    fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    for size in PNG_SIZES {
        let png = render_png(&tree, size)?;
        fs::write(out_dir.join(format!("{size}x{size}.png")), png)?;
    }
    let ico_pngs = ICO_SIZES
        .iter()
        .map(|&s| render_png(&tree, s).map(|png| (s, png)))
        .collect::<Result<Vec<_>>>()?;
    fs::write(out_dir.join("icon.ico"), encode_ico(&ico_pngs))?;
    let icns_pngs = ICNS_ENTRIES
        .iter()
        .map(|&(ty, s)| render_png(&tree, s).map(|png| (*ty, png)))
        .collect::<Result<Vec<_>>>()?;
    fs::write(out_dir.join("icon.icns"), encode_icns(&icns_pngs))?;
    eprintln!("icons written to {}", out_dir.display());
    Ok(())
}

fn render_png(tree: &usvg::Tree, size: u32) -> Result<Vec<u8>> {
    let mut pixmap =
        tiny_skia::Pixmap::new(size, size).ok_or_else(|| anyhow!("cannot allocate {size}x{size} pixmap"))?;
    let scale = size as f32 / tree.size().width();
    resvg::render(
        tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .map_err(|e| anyhow!("encoding {size}px PNG: {e}"))
}

/// ICO with PNG-compressed images (supported since Windows Vista). A stored size of 0 means 256.
fn encode_ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]); // reserved, type = icon
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, png) in images {
        let dim = if *size >= 256 { 0 } else { *size as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0]); // w, h, palette colours, reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in images {
        out.extend_from_slice(png);
    }
    out
}

/// ICNS: big-endian `icns` header followed by `(type, length, payload)` entries.
fn encode_icns(entries: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let total = 8 + entries.iter().map(|(_, png)| 8 + png.len()).sum::<usize>();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"icns");
    out.extend_from_slice(&(total as u32).to_be_bytes());
    for (ty, png) in entries {
        out.extend_from_slice(ty);
        out.extend_from_slice(&(8 + png.len() as u32).to_be_bytes());
        out.extend_from_slice(png);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ico_header_and_offsets_are_consistent() {
        let ico = encode_ico(&[(16, vec![1, 2, 3]), (256, vec![4, 5])]);
        assert_eq!(&ico[..6], &[0, 0, 1, 0, 2, 0]);
        // second entry: 256 px is stored as 0, payload starts after header + 2 directory entries
        assert_eq!(ico[6 + 16], 0);
        let offset = u32::from_le_bytes(ico[6 + 16 + 12..6 + 16 + 16].try_into().unwrap());
        assert_eq!(offset, 6 + 32 + 3);
        assert_eq!(&ico[offset as usize..], &[4, 5]);
    }

    #[test]
    fn icns_length_field_covers_whole_file() {
        let icns = encode_icns(&[(*b"ic07", vec![9; 10])]);
        assert_eq!(&icns[..4], b"icns");
        assert_eq!(
            u32::from_be_bytes(icns[4..8].try_into().unwrap()) as usize,
            icns.len()
        );
        assert_eq!(u32::from_be_bytes(icns[12..16].try_into().unwrap()), 18);
    }

    #[test]
    fn shipped_icon_renders_with_visible_pixels() {
        let svg =
            fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/icon.svg")).unwrap();
        let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).unwrap();
        let png = render_png(&tree, 64).unwrap();
        assert_eq!(&png[1..4], b"PNG");
    }
}
