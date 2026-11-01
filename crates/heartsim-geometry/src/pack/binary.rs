//! Little-endian binary records of the anatomy pack. Every file starts with a 4-byte magic and a
//! `u32` version so a mismatched file fails fast instead of producing garbage geometry.

use crate::cloud::{CellCloud, K_NEIGHBOURS, Neighbour};
use crate::hood::Hood;
use glam::Vec3;
use heartsim_core::Region;

use super::PackError;

pub const BINARY_VERSION: u32 = 1;
pub const CELLS_MAGIC: &[u8; 4] = b"HSCL";
pub const NEIGHBOURS_MAGIC: &[u8; 4] = b"HSNB";
pub const HOODS_MAGIC: &[u8; 4] = b"HSHD";
pub const SEGMENTS_MAGIC: &[u8; 4] = b"HSSG";

/// Region ↔ byte: the index in `Region::ALL` (append-only; see `heartsim_core::Region`).
#[must_use]
pub fn region_to_u8(r: Region) -> u8 {
    Region::ALL
        .iter()
        .position(|x| *x == r)
        .expect("region listed in Region::ALL") as u8
}

pub fn region_from_u8(b: u8) -> Result<Region, PackError> {
    Region::ALL
        .get(usize::from(b))
        .copied()
        .ok_or_else(|| PackError::Format(format!("unknown region code {b}")))
}

struct Writer(Vec<u8>);

impl Writer {
    fn new(magic: [u8; 4], capacity: usize) -> Self {
        let mut v = Vec::with_capacity(capacity + 8);
        v.extend_from_slice(&magic);
        v.extend_from_slice(&BINARY_VERSION.to_le_bytes());
        Self(v)
    }
    fn u32(&mut self, x: u32) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
    fn f32s(&mut self, xs: impl IntoIterator<Item = f32>) {
        for x in xs {
            self.0.extend_from_slice(&x.to_le_bytes());
        }
    }
    fn u32s(&mut self, xs: impl IntoIterator<Item = u32>) {
        for x in xs {
            self.0.extend_from_slice(&x.to_le_bytes());
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    file: &'static str,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], magic: [u8; 4], file: &'static str) -> Result<Self, PackError> {
        if bytes.len() < 8 || bytes[0..4] != magic {
            return Err(PackError::Format(format!("{file}: bad magic")));
        }
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if version != BINARY_VERSION {
            return Err(PackError::Format(format!(
                "{file}: binary version {version}, expected {BINARY_VERSION}"
            )));
        }
        Ok(Self { bytes, pos: 8, file })
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], PackError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|e| *e <= self.bytes.len())
            .ok_or_else(|| PackError::Format(format!("{}: truncated", self.file)))?;
        let s = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, PackError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn f32s(&mut self, n: usize) -> Result<Vec<f32>, PackError> {
        let b = self.take(n * 4)?;
        Ok(b.as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect())
    }
    fn u32s(&mut self, n: usize) -> Result<Vec<u32>, PackError> {
        let b = self.take(n * 4)?;
        Ok(b.as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect())
    }
    fn finish(self) -> Result<(), PackError> {
        if self.pos == self.bytes.len() {
            Ok(())
        } else {
            Err(PackError::Format(format!("{}: trailing bytes", self.file)))
        }
    }
}

fn vec3s(flat: &[f32]) -> Vec<Vec3> {
    flat.as_chunks::<3>().0.iter().map(|c| Vec3::from(*c)).collect()
}

/// `cells.bin`: count, positions f32×3, fibres f32×3, regions u8, depth f32.
#[must_use]
pub fn write_cells(cloud: &CellCloud) -> Vec<u8> {
    let n = cloud.len();
    let mut w = Writer::new(*CELLS_MAGIC, 4 + n * 29);
    w.u32(n as u32);
    w.f32s(cloud.positions.iter().flat_map(Vec3::to_array));
    w.f32s(cloud.fibres.iter().flat_map(Vec3::to_array));
    w.0.extend(cloud.regions.iter().map(|r| region_to_u8(*r)));
    w.f32s(cloud.depth.iter().copied());
    w.0
}

/// Columns of `cells.bin`: positions, fibres, regions, depth.
pub type CellRecords = (Vec<Vec3>, Vec<Vec3>, Vec<Region>, Vec<f32>);

/// Inverse of [`write_cells`] (without the neighbour graph).
pub fn read_cells(bytes: &[u8]) -> Result<CellRecords, PackError> {
    let mut r = Reader::new(bytes, *CELLS_MAGIC, "cells.bin")?;
    let n = r.u32()? as usize;
    let positions = vec3s(&r.f32s(n * 3)?);
    let fibres = vec3s(&r.f32s(n * 3)?);
    let regions = r
        .take(n)?
        .iter()
        .map(|b| region_from_u8(*b))
        .collect::<Result<Vec<_>, _>>()?;
    let depth = r.f32s(n)?;
    r.finish()?;
    Ok((positions, fibres, regions, depth))
}

/// `neighbours.bin`: count, k, then per cell k × (index u32, distance f32, alignment f32).
#[must_use]
pub fn write_neighbours(cloud: &CellCloud) -> Vec<u8> {
    let n = cloud.len();
    let mut w = Writer::new(*NEIGHBOURS_MAGIC, 8 + n * K_NEIGHBOURS * 12);
    w.u32(n as u32);
    w.u32(K_NEIGHBOURS as u32);
    for nb in &cloud.neighbours {
        for e in nb {
            w.u32(e.index);
            w.f32s([e.distance_mm, e.fibre_alignment]);
        }
    }
    w.0
}

pub fn read_neighbours(
    bytes: &[u8],
    expected_cells: usize,
) -> Result<Vec<[Neighbour; K_NEIGHBOURS]>, PackError> {
    let mut r = Reader::new(bytes, *NEIGHBOURS_MAGIC, "neighbours.bin")?;
    let n = r.u32()? as usize;
    let k = r.u32()? as usize;
    if n != expected_cells || k != K_NEIGHBOURS {
        return Err(PackError::Format(format!(
            "neighbours.bin: {n} cells × {k}, expected {expected_cells} × {K_NEIGHBOURS}"
        )));
    }
    let mut out = Vec::with_capacity(n);
    let empty = Neighbour {
        index: u32::MAX,
        distance_mm: f32::INFINITY,
        fibre_alignment: 0.0,
    };
    for _ in 0..n {
        let mut nb = [empty; K_NEIGHBOURS];
        for e in &mut nb {
            let index = r.u32()?;
            let d = r.f32s(2)?;
            *e = Neighbour {
                index,
                distance_mm: d[0],
                fibre_alignment: d[1],
            };
        }
        out.push(nb);
    }
    r.finish()?;
    Ok(out)
}

/// `hoods/<mesh>.bin`: vertex count, k, then per vertex k × u32 cell indices and k × f32 weights.
#[must_use]
pub fn write_hoods(hoods: &[Hood]) -> Vec<u8> {
    let mut w = Writer::new(*HOODS_MAGIC, 8 + hoods.len() * K_NEIGHBOURS * 8);
    w.u32(hoods.len() as u32);
    w.u32(K_NEIGHBOURS as u32);
    for h in hoods {
        w.u32s(h.cells);
        w.f32s(h.weights);
    }
    w.0
}

pub fn read_hoods(bytes: &[u8], expected_vertices: usize) -> Result<Vec<Hood>, PackError> {
    let mut r = Reader::new(bytes, *HOODS_MAGIC, "hoods")?;
    let n = r.u32()? as usize;
    let k = r.u32()? as usize;
    if n != expected_vertices || k != K_NEIGHBOURS {
        return Err(PackError::Format(format!(
            "hoods: {n} vertices × {k}, expected {expected_vertices} × {K_NEIGHBOURS}"
        )));
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let cells = r.u32s(K_NEIGHBOURS)?;
        let weights = r.f32s(K_NEIGHBOURS)?;
        out.push(Hood {
            cells: cells.try_into().expect("k entries"),
            weights: weights.try_into().expect("k entries"),
        });
    }
    r.finish()?;
    Ok(out)
}

/// `segments.bin`: count, AHA segment u8 per cell.
#[must_use]
pub fn write_segments(segments: &[u8]) -> Vec<u8> {
    let mut w = Writer::new(*SEGMENTS_MAGIC, 4 + segments.len());
    w.u32(segments.len() as u32);
    w.0.extend_from_slice(segments);
    w.0
}

pub fn read_segments(bytes: &[u8], expected_cells: usize) -> Result<Vec<u8>, PackError> {
    let mut r = Reader::new(bytes, *SEGMENTS_MAGIC, "segments.bin")?;
    let n = r.u32()? as usize;
    if n != expected_cells {
        return Err(PackError::Format(format!(
            "segments.bin: {n} cells, expected {expected_cells}"
        )));
    }
    let out = r.take(n)?.to_vec();
    r.finish()?;
    Ok(out)
}
