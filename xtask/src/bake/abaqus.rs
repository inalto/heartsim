//! A reader for the parts of an Abaqus input file (`.inp`, keyword format) that carry geometry:
//! `*NODE` blocks (with their `NSET=`), `*ELEMENT` blocks (with `TYPE=` and `ELSET=`), `*NSET` and
//! `*ELSET` lists (with `GENERATE`) and the first data value under each `*SOLID SECTION` and
//! `*SHELL SECTION` (a truss's cross-section area, a shell's thickness). Comments (`**`) and every
//! other keyword are skipped; material, step and boundary data are not read.

use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;

/// One element block's elements: `(element id, node ids)`.
pub type Elements = Vec<(u32, Vec<u32>)>;

/// The geometry of an Abaqus model.
#[derive(Debug, Default)]
pub struct Model {
    /// Node id → coordinates, in the file's order.
    pub nodes: Vec<(u32, [f64; 3])>,
    /// `ELSET=` of each `*ELEMENT` block → its type and elements, in file order.
    pub element_blocks: BTreeMap<String, (String, Elements)>,
    /// `*NSET` lists and the `NSET=` of node blocks → node ids.
    pub nsets: BTreeMap<String, Vec<u32>>,
    /// `*ELSET` lists → element ids (names of other sets in the list are not expanded).
    pub elsets: BTreeMap<String, Vec<u32>>,
    /// `ELSET=` of each `*SOLID SECTION` / `*SHELL SECTION` → the first value of its data line.
    pub sections: BTreeMap<String, f64>,
}

impl Model {
    /// Index of each node id in [`Self::nodes`].
    #[must_use]
    pub fn node_index(&self) -> BTreeMap<u32, usize> {
        self.nodes
            .iter()
            .enumerate()
            .map(|(i, (id, _))| (*id, i))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Block {
    Skip,
    Node(Option<String>),
    Element(String, String),
    Nset(String, bool),
    Elset(String, bool),
    Section(String),
}

/// Case-insensitive `KEY=value` options of a keyword line.
fn options(line: &str) -> BTreeMap<String, String> {
    line.split(',')
        .skip(1)
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            Some((k.trim().to_ascii_uppercase(), v.trim().to_string()))
        })
        .collect()
}

fn has_flag(line: &str, flag: &str) -> bool {
    line.split(',')
        .skip(1)
        .any(|p| p.trim().eq_ignore_ascii_case(flag))
}

/// Parse the text of an `.inp` file.
///
/// # Errors
/// A malformed number, an element block without `ELSET=`, or a node with fewer than three
/// coordinates.
pub fn parse(text: &str) -> Result<Model> {
    let mut m = Model::default();
    let mut block = Block::Skip;
    // data lines ending in a comma continue on the next line (long element records)
    let mut pending = String::new();
    for (lineno, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("**") {
            continue;
        }
        if line.starts_with('*') {
            pending.clear();
            let kw = line
                .split(',')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_uppercase();
            let o = options(line);
            block = match kw.as_str() {
                "*NODE" => Block::Node(o.get("NSET").cloned()),
                "*ELEMENT" => {
                    let set = o
                        .get("ELSET")
                        .cloned()
                        .with_context(|| format!("line {}: *ELEMENT without ELSET=", lineno + 1))?;
                    Block::Element(o.get("TYPE").cloned().unwrap_or_default(), set)
                }
                "*NSET" => Block::Nset(
                    o.get("NSET").cloned().unwrap_or_default(),
                    has_flag(line, "GENERATE"),
                ),
                "*ELSET" => Block::Elset(
                    o.get("ELSET").cloned().unwrap_or_default(),
                    has_flag(line, "GENERATE"),
                ),
                "*SOLID SECTION" | "*SHELL SECTION" => {
                    Block::Section(o.get("ELSET").cloned().unwrap_or_default())
                }
                _ => Block::Skip,
            };
            continue;
        }
        if matches!(block, Block::Element(..)) && line.ends_with(',') {
            pending.push_str(line);
            continue;
        }
        let data = if pending.is_empty() {
            line.to_string()
        } else {
            let mut d = std::mem::take(&mut pending);
            d.push_str(line);
            d
        };
        let fields: Vec<&str> = data.split(',').map(str::trim).filter(|f| !f.is_empty()).collect();
        let int = |f: &str| -> Result<u32> {
            f.parse::<u32>()
                .with_context(|| format!("line {}: `{f}` is not an id", lineno + 1))
        };
        match &block {
            Block::Skip => {}
            Block::Node(set) => {
                if fields.len() < 4 {
                    bail!("line {}: a node needs an id and three coordinates", lineno + 1);
                }
                let id = int(fields[0])?;
                let mut x = [0.0f64; 3];
                for (a, f) in x.iter_mut().zip(&fields[1..4]) {
                    *a = f
                        .parse()
                        .with_context(|| format!("line {}: `{f}` is not a number", lineno + 1))?;
                }
                m.nodes.push((id, x));
                if let Some(s) = set {
                    m.nsets.entry(s.clone()).or_default().push(id);
                }
            }
            Block::Element(ty, set) => {
                let id = int(fields[0])?;
                let nodes = fields[1..].iter().map(|f| int(f)).collect::<Result<Vec<_>>>()?;
                m.element_blocks
                    .entry(set.clone())
                    .or_insert_with(|| (ty.clone(), Vec::new()))
                    .1
                    .push((id, nodes));
            }
            Block::Nset(set, generate) | Block::Elset(set, generate) => {
                let ids: Vec<u32> = if *generate {
                    let v = fields.iter().map(|f| int(f)).collect::<Result<Vec<_>>>()?;
                    let (a, b, s) = (v[0], *v.get(1).unwrap_or(&v[0]), (*v.get(2).unwrap_or(&1)).max(1));
                    (a..=b).step_by(s as usize).collect()
                } else {
                    // names of other sets may appear in an ELSET list; they are not expanded
                    fields.iter().filter_map(|f| f.parse::<u32>().ok()).collect()
                };
                let target = if matches!(block, Block::Nset(..)) {
                    &mut m.nsets
                } else {
                    &mut m.elsets
                };
                target.entry(set.clone()).or_default().extend(ids);
            }
            Block::Section(set) => {
                if let Some(v) = fields.first().and_then(|f| f.parse::<f64>().ok()) {
                    m.sections.entry(set.clone()).or_insert(v);
                }
                block = Block::Skip;
            }
        }
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hand-written model in the keyword layout of the Texas TriValve files (two shells, one
    /// truss, the sets and a section); not taken from any dataset.
    const MODEL: &str = "**  a comment\n\
*PART, NAME=Leaflet\n\
*NODE, NSET=Leaflet_Nodes\n\
1, 0.0, 0.0, 0.0\n\
2, 1.0, 0.0, 0.0\n\
3, 1.0, 1.0, 0.0\n\
4, 0.0, 1.0, 0.0\n\
5, 2.0, 0.0, 0.0\n\
6, 2.0, 1.0, 0.0\n\
*NODE, NSET=Chordae_Nodes\n\
7, 1.5, 0.5, -3.0e+00\n\
*ELEMENT, TYPE=S4R, ELSET=Anterior_Leaflet_Elements\n\
1, 1, 2, 3, 4\n\
*ELEMENT, TYPE=S4R, ELSET=Septal_Leaflet_Elements\n\
2, 2, 5,\n\
6, 3\n\
*ELEMENT, TYPE=T3D2, ELSET=Chordae_Elements\n\
** a comment inside a block\n\
3, 7, 6\n\
*NSET, NSET=Annular_Nodes\n\
1,2,\n\
5,\n\
*NSET, NSET=Range, GENERATE\n\
2, 6, 2\n\
*ELSET, ELSET=Anterior_Chordae\n\
3,\n\
*ELSET, ELSET=LEAFLET_ELEMENTS\n\
Septal_Leaflet_Elements, Anterior_Leaflet_Elements\n\
*Solid Section, elset=Anterior_Chordae, material=Anterior_Chordae\n\
0.35, \n\
*End Part\n\
*Step, name=\"x\"\n\
*Boundary\n\
Leaflet_Instance.1,1,1,0.5\n";

    #[test]
    fn reads_nodes_elements_sets_and_sections() {
        let m = parse(MODEL).expect("parses");
        assert_eq!(m.nodes.len(), 7);
        assert_eq!(m.nodes[6], (7, [1.5, 0.5, -3.0]));
        assert_eq!(m.nsets["Leaflet_Nodes"], vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(m.nsets["Chordae_Nodes"], vec![7]);
        assert_eq!(m.nsets["Annular_Nodes"], vec![1, 2, 5]);
        assert_eq!(m.nsets["Range"], vec![2, 4, 6]);
        let (ty, a) = &m.element_blocks["Anterior_Leaflet_Elements"];
        assert_eq!((ty.as_str(), a.clone()), ("S4R", vec![(1, vec![1, 2, 3, 4])]));
        assert_eq!(
            m.element_blocks["Septal_Leaflet_Elements"].1,
            vec![(2, vec![2, 5, 6, 3])]
        );
        assert_eq!(m.element_blocks["Chordae_Elements"].1, vec![(3, vec![7, 6])]);
        assert_eq!(m.elsets["Anterior_Chordae"], vec![3]);
        assert!(
            m.elsets["LEAFLET_ELEMENTS"].is_empty(),
            "set names are not expanded"
        );
        assert!((m.sections["Anterior_Chordae"] - 0.35).abs() < 1e-12);
        assert_eq!(m.node_index()[&5], 4);
    }

    #[test]
    fn rejects_an_element_block_without_a_set_and_a_bad_number() {
        assert!(parse("*ELEMENT, TYPE=S4R\n1, 1, 2, 3, 4\n").is_err());
        assert!(parse("*NODE\n1, 0.0, x, 0.0\n").is_err());
    }
}
