//! Minimal glTF 2.0 binary (GLB) writer and reader for render meshes: one mesh, one primitive,
//! `POSITION` + `NORMAL` float accessors and `u32` indices. Files open in Blender / three.js for
//! inspection and are read back with the `gltf` crate (so the writer is validated by an independent
//! parser).

use crate::mesh::{TriMesh, Vertex};
use glam::Vec3;

use super::PackError;

const GLB_MAGIC: u32 = 0x4654_6C67;
const CHUNK_JSON: u32 = 0x4E4F_534A;
const CHUNK_BIN: u32 = 0x004E_4942;

/// Serialise `mesh` as a GLB with node/mesh name `name`.
#[must_use]
pub fn write_glb(mesh: &TriMesh, name: &str) -> Vec<u8> {
    let n = mesh.vertex_count();
    let pos_len = n * 12;
    let idx_len = mesh.indices.len() * 4;
    let mut bin = Vec::with_capacity(pos_len * 2 + idx_len);
    for v in &mesh.vertices {
        for x in v.position {
            bin.extend_from_slice(&x.to_le_bytes());
        }
    }
    for v in &mesh.vertices {
        for x in v.normal {
            bin.extend_from_slice(&x.to_le_bytes());
        }
    }
    for i in &mesh.indices {
        bin.extend_from_slice(&i.to_le_bytes());
    }
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let (lo, hi) = mesh.bounds();
    let (lo, hi) = if n == 0 {
        (Vec3::ZERO, Vec3::ZERO)
    } else {
        (lo, hi)
    };
    let json = serde_json::json!({
        "asset": { "version": "2.0", "generator": "heartsim-geometry" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0, "name": name }],
        "meshes": [{ "name": name, "primitives": [{ "attributes": { "POSITION": 0, "NORMAL": 1 }, "indices": 2, "mode": 4 }] }],
        "buffers": [{ "byteLength": bin.len() }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": 0, "byteLength": pos_len, "target": 34962 },
            { "buffer": 0, "byteOffset": pos_len, "byteLength": pos_len, "target": 34962 },
            { "buffer": 0, "byteOffset": pos_len * 2, "byteLength": idx_len, "target": 34963 }
        ],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": n, "type": "VEC3", "min": lo.to_array(), "max": hi.to_array() },
            { "bufferView": 1, "componentType": 5126, "count": n, "type": "VEC3" },
            { "bufferView": 2, "componentType": 5125, "count": mesh.indices.len(), "type": "SCALAR" }
        ]
    });
    let mut json_bytes = serde_json::to_vec(&json).expect("static json");
    while !json_bytes.len().is_multiple_of(4) {
        json_bytes.push(b' ');
    }
    let total = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&GLB_MAGIC.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_BIN.to_le_bytes());
    out.extend_from_slice(&bin);
    out
}

/// Read every triangle primitive of a GLB into one `TriMesh` (normals recomputed if absent).
pub fn read_glb(bytes: &[u8]) -> Result<TriMesh, PackError> {
    let g = gltf::Gltf::from_slice(bytes).map_err(|e| PackError::Format(format!("glb: {e}")))?;
    let blob = g.blob.as_deref();
    let mut mesh = TriMesh::default();
    let mut need_normals = false;
    for m in g.meshes() {
        for prim in m.primitives() {
            let reader = prim.reader(|buffer| match buffer.source() {
                gltf::buffer::Source::Bin => blob,
                gltf::buffer::Source::Uri(_) => None,
            });
            let positions = reader
                .read_positions()
                .ok_or_else(|| PackError::Format("glb: primitive without POSITION".into()))?;
            let base = mesh.vertices.len() as u32;
            let normals: Option<Vec<[f32; 3]>> = reader.read_normals().map(Iterator::collect);
            if normals.is_none() {
                need_normals = true;
            }
            for (i, p) in positions.enumerate() {
                mesh.vertices.push(Vertex {
                    position: p,
                    normal: normals.as_ref().map_or([0.0; 3], |n| n[i]),
                });
            }
            match reader.read_indices() {
                Some(idx) => mesh.indices.extend(idx.into_u32().map(|i| i + base)),
                None => mesh.indices.extend(base..mesh.vertices.len() as u32),
            }
        }
    }
    if need_normals {
        mesh.recompute_normals();
    }
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::icosphere;

    #[test]
    fn glb_roundtrip_is_exact() {
        let m = icosphere(2);
        let bytes = write_glb(&m, "test");
        assert_eq!(&bytes[0..4], b"glTF");
        let back = read_glb(&bytes).unwrap();
        assert_eq!(back, m);
    }
}
