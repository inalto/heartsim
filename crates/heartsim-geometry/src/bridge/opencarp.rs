//! openCARP's ASCII mesh and activation files (openCARP user's manual, "Input files"):
//!
//! * `.pts` — node count, then `x y z` per node, in **micrometres**;
//! * `.elem` — element count, then `Tt n0 n1 n2 n3 tag` per element, 0-indexed;
//! * `.lon` — fibre vectors per element (1 = fibre only), then one line per element;
//! * `.vtx` — node count, `intra` or `extra`, then one node per line;
//! * activation `.dat` — one time per node, ms; openCARP writes −1 for a node never activated.

use super::tetra::TetMesh;
use super::{BridgeError, UM_PER_MM};
use glam::Vec3;
use std::fmt::Write as _;

/// `.pts` text: coordinates converted from mm to µm.
#[must_use]
pub fn pts_text(points_mm: &[Vec3]) -> String {
    let mut s = String::with_capacity(points_mm.len() * 32 + 16);
    let _ = writeln!(s, "{}", points_mm.len());
    for p in points_mm {
        let [x, y, z] = p.to_array().map(|v| f64::from(v) * UM_PER_MM);
        let _ = writeln!(s, "{x:.3} {y:.3} {z:.3}");
    }
    s
}

/// `.elem` text. Tetrahedra only; each line carries its region tag.
#[must_use]
pub fn elem_text(mesh: &TetMesh) -> String {
    let mut s = String::with_capacity(mesh.tets.len() * 40 + 16);
    let _ = writeln!(s, "{}", mesh.tets.len());
    for (t, tag) in mesh.tets.iter().zip(&mesh.tags) {
        let _ = writeln!(s, "Tt {} {} {} {} {tag}", t[0], t[1], t[2], t[3]);
    }
    s
}

/// `.lon` text with one fibre vector per element.
#[must_use]
pub fn lon_text(fibres: &[Vec3]) -> String {
    let mut s = String::with_capacity(fibres.len() * 30 + 4);
    s.push_str("1\n");
    for f in fibres {
        let _ = writeln!(s, "{:.6} {:.6} {:.6}", f.x, f.y, f.z);
    }
    s
}

/// `.vtx` text of an intracellular (transmembrane) electrode.
#[must_use]
pub fn vtx_text(nodes: &[u32]) -> String {
    let mut s = String::with_capacity(nodes.len() * 8 + 16);
    let _ = writeln!(s, "{}\nintra", nodes.len());
    for n in nodes {
        let _ = writeln!(s, "{n}");
    }
    s
}

fn header_count(lines: &mut std::str::Lines<'_>, format: &'static str) -> Result<usize, BridgeError> {
    let first = lines
        .next()
        .ok_or_else(|| BridgeError::new(format, "empty file"))?;
    first
        .split_whitespace()
        .next()
        .and_then(|w| w.parse().ok())
        .ok_or_else(|| BridgeError::new(format, format!("bad header `{first}`")))
}

fn number<T: std::str::FromStr>(
    word: Option<&str>,
    format: &'static str,
    line: usize,
) -> Result<T, BridgeError> {
    word.and_then(|w| w.parse().ok())
        .ok_or_else(|| BridgeError::new(format, format!("line {line}: expected a number")))
}

/// Parse `.pts`, returning coordinates in mm.
pub fn parse_pts(text: &str) -> Result<Vec<Vec3>, BridgeError> {
    const F: &str = ".pts";
    let mut lines = text.lines();
    let n = header_count(&mut lines, F)?;
    let mut out = Vec::with_capacity(n);
    for (i, line) in lines.enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let mut w = line.split_whitespace();
        let mut c = [0.0f64; 3];
        for v in &mut c {
            *v = number(w.next(), F, i + 2)?;
        }
        out.push(Vec3::new(
            (c[0] / UM_PER_MM) as f32,
            (c[1] / UM_PER_MM) as f32,
            (c[2] / UM_PER_MM) as f32,
        ));
    }
    if out.len() != n {
        return Err(BridgeError::new(
            F,
            format!("header says {n} nodes, found {}", out.len()),
        ));
    }
    Ok(out)
}

/// One element of a parsed `.elem` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    /// Type specifier (`Tt`, `Hx`, …).
    pub kind: String,
    pub nodes: Vec<u32>,
    pub tag: Option<u32>,
}

/// Node count of an element type specifier (manual table "element types").
#[must_use]
pub fn element_nodes(kind: &str) -> Option<usize> {
    Some(match kind {
        "Ln" => 2,
        "Tr" => 3,
        "Qd" | "Tt" => 4,
        "Py" => 5,
        "Pr" => 6,
        "Hx" => 8,
        _ => return None,
    })
}

/// Parse `.elem` (any element type).
pub fn parse_elem(text: &str) -> Result<Vec<Element>, BridgeError> {
    const F: &str = ".elem";
    let mut lines = text.lines();
    let n = header_count(&mut lines, F)?;
    let mut out = Vec::with_capacity(n);
    for (i, line) in lines.enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let mut w = line.split_whitespace();
        let kind = w.next().unwrap_or_default().to_string();
        let count = element_nodes(&kind)
            .ok_or_else(|| BridgeError::new(F, format!("line {}: unknown element `{kind}`", i + 2)))?;
        let mut nodes = Vec::with_capacity(count);
        for _ in 0..count {
            nodes.push(number(w.next(), F, i + 2)?);
        }
        let tag = w.next().map(|t| number(Some(t), F, i + 2)).transpose()?;
        out.push(Element { kind, nodes, tag });
    }
    if out.len() != n {
        return Err(BridgeError::new(
            F,
            format!("header says {n} elements, found {}", out.len()),
        ));
    }
    Ok(out)
}

/// Parse `.lon`, returning the fibre direction per element (a sheet direction, if present, is ignored).
pub fn parse_lon(text: &str) -> Result<Vec<Vec3>, BridgeError> {
    const F: &str = ".lon";
    let mut lines = text.lines();
    let per_element = header_count(&mut lines, F)?;
    if !(1..=2).contains(&per_element) {
        return Err(BridgeError::new(F, format!("{per_element} vectors per element")));
    }
    let mut out = Vec::new();
    for (i, line) in lines.enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let mut w = line.split_whitespace();
        let mut c = [0.0f32; 3];
        for v in &mut c {
            *v = number(w.next(), F, i + 2)?;
        }
        out.push(Vec3::from(c));
    }
    Ok(out)
}

/// Parse `.vtx`: the node list (the `intra`/`extra` line is optional in older files).
pub fn parse_vtx(text: &str) -> Result<Vec<u32>, BridgeError> {
    const F: &str = ".vtx";
    let mut lines = text.lines();
    let n = header_count(&mut lines, F)?;
    let mut out = Vec::with_capacity(n);
    for (i, line) in lines.enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let word = line.split_whitespace().next();
        if out.is_empty() && matches!(word, Some("intra" | "extra")) {
            continue;
        }
        out.push(number(word, F, i + 2)?);
    }
    if out.len() != n {
        return Err(BridgeError::new(
            F,
            format!("header says {n} nodes, found {}", out.len()),
        ));
    }
    Ok(out)
}

/// Parse an activation `.dat`: one time per node, ms (first column of each line); negative values
/// (openCARP's −1) become `NaN`, "never activated".
pub fn parse_activation_dat(text: &str) -> Result<Vec<f32>, BridgeError> {
    const F: &str = "activation .dat";
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| {
            let t: f32 = number(l.split_whitespace().next(), F, i + 1)?;
            Ok(if t < 0.0 { f32::NAN } else { t })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::tetra::box_mesh;
    use super::*;

    #[test]
    fn pts_are_micrometres() {
        let text = pts_text(&[Vec3::new(1.0, 2.5, -3.0)]);
        assert_eq!(text, "1\n1000.000 2500.000 -3000.000\n");
        assert_eq!(parse_pts(&text).unwrap(), vec![Vec3::new(1.0, 2.5, -3.0)]);
    }

    #[test]
    fn files_match_the_manual_examples() {
        // the examples printed in the openCARP manual's format tables
        let elems = parse_elem("2\nTt 0 1 2 150 1\nTt 123 124 125 210 10\n").unwrap();
        assert_eq!(elems[1].nodes, vec![123, 124, 125, 210]);
        assert_eq!(elems[1].tag, Some(10));
        let lon = parse_lon("2\n0.831 0.549 0.077 0.473 -0.775 0.417\n0.0 0.0 0.0 0.0 0.0 0.0\n").unwrap();
        assert_eq!(lon[0], Vec3::new(0.831, 0.549, 0.077));
        assert_eq!(lon[1], Vec3::ZERO);
        assert_eq!(parse_vtx("2\nintra\n0\n123\n").unwrap(), vec![0, 123]);
        let pts = parse_pts("1\n1000.00 1000.00 300.00\n").unwrap();
        assert_eq!(pts, vec![Vec3::new(1.0, 1.0, 0.3)]);
    }

    #[test]
    fn mesh_round_trips_through_text() {
        let m = box_mesh([3, 2, 2], 0.5, Vec3::ZERO, Vec3::X, 7);
        let elems = parse_elem(&elem_text(&m)).unwrap();
        assert_eq!(elems.len(), m.tets.len());
        assert!(elems.iter().all(|e| e.kind == "Tt" && e.tag == Some(7)));
        assert_eq!(elems[3].nodes, m.tets[3].to_vec());
        assert_eq!(parse_lon(&lon_text(&m.fibres)).unwrap(), m.fibres);
        assert_eq!(parse_pts(&pts_text(&m.points_mm)).unwrap(), m.points_mm);
        assert_eq!(parse_vtx(&vtx_text(&[4, 9])).unwrap(), vec![4, 9]);
    }

    #[test]
    fn inconsistent_counts_and_bad_lines_are_errors() {
        assert!(parse_pts("2\n0 0 0\n").is_err());
        assert!(parse_elem("1\nZz 0 1\n").is_err());
        assert!(parse_elem("1\nTt 0 1 2\n").is_err());
        assert!(parse_vtx("2\nintra\n1\n").is_err());
        assert!(parse_lon("3\n0 0 1\n").is_err());
    }

    #[test]
    fn unactivated_nodes_become_nan() {
        let t = parse_activation_dat("12.5\n-1\n3\n").unwrap();
        assert_eq!(t[0], 12.5);
        assert!(t[1].is_nan());
    }
}
