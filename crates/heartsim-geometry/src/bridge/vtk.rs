//! Reader for unstructured VTK grids, the output of MonoAlg3D and `meshtool`: XML `.vtu` (ASCII, inline
//! base64 and appended raw or base64 payloads, optionally zlib-compressed), legacy `.vtk` (ASCII or
//! binary, file versions before and after 5.1), and `.pvd` time-series indexes. A small ASCII `.vtu`
//! writer serves mesh previews and tests.
//!
//! Finite-volume solvers store fields per cell (`CellData`), finite-element ones per node
//! (`PointData`), so [`VtkGrid`] keeps both and can give the cell centres to sample cell data at.
//!
//! Not supported, with a clear error: parallel `.pvtu`, structured datasets, LZ4 and LZMA compression.

use super::BridgeError;
use base64::Engine as _;
use std::fmt::Write as _;
use std::io::Read as _;

const VTU: &str = "VTU";
const LEGACY: &str = "legacy VTK";

/// A named data array; `data.len() == components · tuples`.
#[derive(Debug, Clone, PartialEq)]
pub struct VtkField {
    pub name: String,
    pub components: usize,
    pub data: Vec<f64>,
}

/// An unstructured grid. Lengths are in whatever unit the solver wrote.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VtkGrid {
    pub points: Vec<[f64; 3]>,
    /// Node lists: `connectivity[offsets[c]..offsets[c + 1]]` is cell `c`.
    pub connectivity: Vec<u32>,
    pub offsets: Vec<u32>,
    pub point_fields: Vec<VtkField>,
    pub cell_fields: Vec<VtkField>,
}

impl VtkGrid {
    /// Number of cells.
    #[must_use]
    pub fn cell_count(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    /// Mean of each cell's nodes: where a `CellData` value lives.
    #[must_use]
    pub fn cell_centres(&self) -> Vec<[f64; 3]> {
        self.offsets
            .windows(2)
            .map(|w| {
                let nodes = &self.connectivity[w[0] as usize..w[1] as usize];
                let mut c = [0.0f64; 3];
                for n in nodes {
                    for (a, b) in c.iter_mut().zip(self.points[*n as usize]) {
                        *a += b;
                    }
                }
                c.map(|v| v / nodes.len().max(1) as f64)
            })
            .collect()
    }

    fn check(&self, format: &'static str) -> Result<(), BridgeError> {
        if let Some(bad) = self
            .connectivity
            .iter()
            .find(|n| **n as usize >= self.points.len())
        {
            return Err(BridgeError::new(
                format,
                format!("cell node {bad} is past the {} points", self.points.len()),
            ));
        }
        if self.offsets.windows(2).any(|w| w[0] > w[1])
            || self
                .offsets
                .last()
                .is_some_and(|l| *l as usize != self.connectivity.len())
        {
            return Err(BridgeError::new(
                format,
                "cell offsets do not match the connectivity",
            ));
        }
        for (fields, tuples, what) in [
            (&self.point_fields, self.points.len(), "point"),
            (&self.cell_fields, self.cell_count(), "cell"),
        ] {
            for f in fields {
                if f.data.len() != f.components * tuples {
                    return Err(BridgeError::new(
                        format,
                        format!(
                            "{what} array `{}` has {} values for {tuples} × {}",
                            f.name,
                            f.data.len(),
                            f.components
                        ),
                    ));
                }
            }
        }
        Ok(())
    }
}

// --- binary helpers ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scalar {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
}

impl Scalar {
    fn from_vtu(s: &str) -> Option<Self> {
        Some(match s {
            "Int8" => Self::I8,
            "UInt8" => Self::U8,
            "Int16" => Self::I16,
            "UInt16" => Self::U16,
            "Int32" => Self::I32,
            "UInt32" => Self::U32,
            "Int64" => Self::I64,
            "UInt64" => Self::U64,
            "Float32" => Self::F32,
            "Float64" => Self::F64,
            _ => return None,
        })
    }

    fn from_legacy(s: &str) -> Option<Self> {
        Some(match s {
            "char" | "vtktypeint8" => Self::I8,
            "unsigned_char" | "bit" | "vtktypeuint8" => Self::U8,
            "short" | "vtktypeint16" => Self::I16,
            "unsigned_short" | "vtktypeuint16" => Self::U16,
            "int" | "vtktypeint32" => Self::I32,
            "unsigned_int" | "vtktypeuint32" => Self::U32,
            "long" | "vtktypeint64" | "vtkidtype" => Self::I64,
            "unsigned_long" | "vtktypeuint64" => Self::U64,
            "float" | "vtktypefloat32" => Self::F32,
            "double" | "vtktypefloat64" => Self::F64,
            _ => return None,
        })
    }

    fn bytes(self) -> usize {
        match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64 | Self::U64 | Self::F64 => 8,
        }
    }

    fn decode(self, b: &[u8], little: bool) -> f64 {
        let mut w = [0u8; 8];
        let n = self.bytes();
        for (i, v) in b.iter().take(n).enumerate() {
            w[if little { i } else { n - 1 - i }] = *v;
        }
        match self {
            Self::I8 => f64::from(w[0].cast_signed()),
            Self::U8 => f64::from(w[0]),
            Self::I16 => f64::from(i16::from_le_bytes([w[0], w[1]])),
            Self::U16 => f64::from(u16::from_le_bytes([w[0], w[1]])),
            Self::I32 => f64::from(i32::from_le_bytes([w[0], w[1], w[2], w[3]])),
            Self::U32 => f64::from(u32::from_le_bytes([w[0], w[1], w[2], w[3]])),
            Self::I64 => i64::from_le_bytes(w) as f64,
            Self::U64 => u64::from_le_bytes(w) as f64,
            Self::F32 => f64::from(f32::from_le_bytes([w[0], w[1], w[2], w[3]])),
            Self::F64 => f64::from_le_bytes(w),
        }
    }
}

/// Decode base64 that may be several padded runs back to back (a VTK compressed payload encodes its
/// header and its data separately), ignoring whitespace.
fn decode_base64_runs(text: &[u8]) -> Result<Vec<u8>, BridgeError> {
    let clean: Vec<u8> = text
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    let mut start = 0;
    while start < clean.len() {
        let mut end = start;
        while end < clean.len() && clean[end] != b'=' {
            end += 1;
        }
        while end < clean.len() && clean[end] == b'=' {
            end += 1;
        }
        base64::engine::general_purpose::STANDARD
            .decode_vec(&clean[start..end], &mut out)
            .map_err(|e| BridgeError::new(VTU, format!("bad base64: {e}")))?;
        start = end;
    }
    Ok(out)
}

/// VTU binary payload layout, from the `VTKFile` attributes.
#[derive(Debug, Clone, Copy)]
struct Layout {
    little: bool,
    header: Scalar,
    compressed: bool,
}

/// `[bytes][data]`, or `[blocks, block size, last block size, compressed sizes…][zlib blocks…]`.
fn unpack_payload(b: &[u8], l: Layout) -> Result<Vec<u8>, BridgeError> {
    let word = l.header.bytes();
    let read = |i: usize| -> Result<usize, BridgeError> {
        b.get(i * word..(i + 1) * word)
            .map(|w| l.header.decode(w, l.little) as usize)
            .ok_or_else(|| BridgeError::new(VTU, "binary payload shorter than its header"))
    };
    if !l.compressed {
        let n = read(0)?;
        return b
            .get(word..word + n)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| BridgeError::new(VTU, format!("payload announces {n} bytes, has fewer")));
    }
    let blocks = read(0)?;
    let mut pos = (3 + blocks) * word;
    let mut out = Vec::new();
    for i in 0..blocks {
        let size = read(3 + i)?;
        let chunk = b
            .get(pos..pos + size)
            .ok_or_else(|| BridgeError::new(VTU, "compressed block shorter than announced"))?;
        flate2::read::ZlibDecoder::new(chunk)
            .read_to_end(&mut out)
            .map_err(|e| BridgeError::new(VTU, format!("zlib: {e}")))?;
        pos += size;
    }
    Ok(out)
}

fn scalars(bytes: &[u8], ty: Scalar, little: bool) -> Vec<f64> {
    bytes
        .chunks_exact(ty.bytes())
        .map(|c| ty.decode(c, little))
        .collect()
}

// --- XML (.vtu) ---------------------------------------------------------------------------------

/// An appended blob: whether it is raw (not base64), and its bytes.
type Appended<'a> = (bool, &'a [u8]);

/// The appended blob of a VTU file and the XML with the blob removed (raw bytes are not valid XML).
fn split_appended(bytes: &[u8]) -> Result<(String, Option<Appended<'_>>), BridgeError> {
    let find = |needle: &[u8], from: usize| {
        bytes[from..]
            .windows(needle.len())
            .position(|w| w == needle)
            .map(|p| p + from)
    };
    let text = |b: &[u8]| {
        std::str::from_utf8(b)
            .map(str::to_string)
            .map_err(|_| BridgeError::new(VTU, "XML part is not UTF-8"))
    };
    let Some(start) = find(b"<AppendedData", 0) else {
        return Ok((text(bytes)?, None));
    };
    let open_end = find(b">", start).ok_or_else(|| BridgeError::new(VTU, "unterminated AppendedData"))?;
    let raw = text(&bytes[start..open_end])?.contains("raw");
    let underscore =
        find(b"_", open_end).ok_or_else(|| BridgeError::new(VTU, "AppendedData has no `_` marker"))?;
    let close = bytes
        .windows(b"</AppendedData>".len())
        .rposition(|w| w == b"</AppendedData>")
        .ok_or_else(|| BridgeError::new(VTU, "unterminated AppendedData"))?;
    let mut xml = text(&bytes[..start])?;
    xml.push_str("</VTKFile>");
    Ok((xml, Some((raw, &bytes[underscore + 1..close]))))
}

struct VtuContext<'a> {
    layout: Layout,
    appended: Option<Appended<'a>>,
    /// All appended offsets, ascending: where a base64 run ends.
    offsets: Vec<usize>,
}

impl VtuContext<'_> {
    fn array(&self, node: roxmltree::Node<'_, '_>) -> Result<VtkField, BridgeError> {
        let name = node.attribute("Name").unwrap_or_default().to_string();
        let ty_name = node.attribute("type").unwrap_or_default();
        let ty = Scalar::from_vtu(ty_name)
            .ok_or_else(|| BridgeError::new(VTU, format!("array `{name}`: unknown type `{ty_name}`")))?;
        let components = node
            .attribute("NumberOfComponents")
            .and_then(|c| c.parse().ok())
            .unwrap_or(1usize);
        let data = match node.attribute("format").unwrap_or("ascii") {
            "ascii" => node
                .text()
                .unwrap_or_default()
                .split_ascii_whitespace()
                .map(|w| {
                    w.parse::<f64>()
                        .map_err(|_| BridgeError::new(VTU, format!("array `{name}`: bad number `{w}`")))
                })
                .collect::<Result<Vec<_>, _>>()?,
            "binary" => {
                let decoded = decode_base64_runs(node.text().unwrap_or_default().as_bytes())?;
                scalars(&unpack_payload(&decoded, self.layout)?, ty, self.layout.little)
            }
            "appended" => {
                let (raw, blob) = self.appended.ok_or_else(|| {
                    BridgeError::new(
                        VTU,
                        format!("array `{name}` is appended but the file has no AppendedData"),
                    )
                })?;
                let offset: usize = node
                    .attribute("offset")
                    .and_then(|o| o.parse().ok())
                    .ok_or_else(|| BridgeError::new(VTU, format!("array `{name}`: bad offset")))?;
                let tail = blob
                    .get(offset..)
                    .ok_or_else(|| BridgeError::new(VTU, format!("array `{name}`: offset past the data")))?;
                let bytes = if raw {
                    unpack_payload(tail, self.layout)?
                } else {
                    let end = self
                        .offsets
                        .iter()
                        .find(|o| **o > offset)
                        .map_or(blob.len(), |o| (*o).min(blob.len()));
                    unpack_payload(&decode_base64_runs(&blob[offset..end])?, self.layout)?
                };
                scalars(&bytes, ty, self.layout.little)
            }
            other => {
                return Err(BridgeError::new(
                    VTU,
                    format!("array `{name}`: unknown format `{other}`"),
                ));
            }
        };
        Ok(VtkField {
            name,
            components,
            data,
        })
    }
}

fn child<'a, 'i>(node: roxmltree::Node<'a, 'i>, tag: &str) -> Option<roxmltree::Node<'a, 'i>> {
    node.children().find(|c| c.has_tag_name(tag))
}

fn arrays<'a, 'i>(node: roxmltree::Node<'a, 'i>) -> impl Iterator<Item = roxmltree::Node<'a, 'i>> {
    node.children().filter(|c| c.has_tag_name("DataArray"))
}

/// Parse an XML `.vtu` file (all pieces are concatenated).
pub fn parse_vtu(bytes: &[u8]) -> Result<VtkGrid, BridgeError> {
    let (xml, appended) = split_appended(bytes)?;
    let doc = roxmltree::Document::parse(&xml).map_err(|e| BridgeError::new(VTU, format!("XML: {e}")))?;
    let root = doc.root_element();
    let kind = root.attribute("type").unwrap_or_default();
    if kind != "UnstructuredGrid" {
        return Err(BridgeError::new(
            VTU,
            format!("dataset type `{kind}` (only UnstructuredGrid; merge .pvtu pieces first)"),
        ));
    }
    if let Some(c) = root.attribute("compressor")
        && c != "vtkZLibDataCompressor"
    {
        return Err(BridgeError::new(VTU, format!("unsupported compressor `{c}`")));
    }
    let header_name = root.attribute("header_type").unwrap_or("UInt32");
    let layout = Layout {
        little: root.attribute("byte_order").unwrap_or("LittleEndian") != "BigEndian",
        header: Scalar::from_vtu(header_name)
            .filter(|s| matches!(s, Scalar::U32 | Scalar::U64))
            .ok_or_else(|| BridgeError::new(VTU, format!("unsupported header_type `{header_name}`")))?,
        compressed: root.attribute("compressor").is_some(),
    };
    let mut offsets: Vec<usize> = doc
        .descendants()
        .filter(|n| n.has_tag_name("DataArray") && n.attribute("format") == Some("appended"))
        .filter_map(|n| n.attribute("offset").and_then(|o| o.parse().ok()))
        .collect();
    offsets.sort_unstable();
    let ctx = VtuContext {
        layout,
        appended,
        offsets,
    };
    let ug = child(root, "UnstructuredGrid")
        .ok_or_else(|| BridgeError::new(VTU, "no UnstructuredGrid element"))?;
    let mut grid = VtkGrid {
        offsets: vec![0],
        ..VtkGrid::default()
    };
    for piece in ug.children().filter(|c| c.has_tag_name("Piece")) {
        read_piece(piece, &ctx, &mut grid)?;
    }
    grid.check(VTU)?;
    Ok(grid)
}

fn read_piece(
    piece: roxmltree::Node<'_, '_>,
    ctx: &VtuContext<'_>,
    grid: &mut VtkGrid,
) -> Result<(), BridgeError> {
    let base = grid.points.len() as u32;
    let points = child(piece, "Points")
        .and_then(|p| arrays(p).next())
        .ok_or_else(|| BridgeError::new(VTU, "a Piece has no Points array"))?;
    let points = ctx.array(points)?;
    if points.components != 3 {
        return Err(BridgeError::new(VTU, "Points must have 3 components"));
    }
    grid.points.extend(points.data.as_chunks::<3>().0.iter().copied());
    if let Some(cells) = child(piece, "Cells") {
        let find = |name: &str| {
            arrays(cells)
                .find(|a| a.attribute("Name") == Some(name))
                .ok_or_else(|| BridgeError::new(VTU, format!("Cells has no `{name}` array")))
        };
        let connectivity = ctx.array(find("connectivity")?)?;
        let offsets = ctx.array(find("offsets")?)?;
        let start = grid.connectivity.len() as u32;
        grid.connectivity
            .extend(connectivity.data.iter().map(|n| *n as u32 + base));
        grid.offsets
            .extend(offsets.data.iter().map(|o| *o as u32 + start));
    }
    for (tag, fields) in [
        ("PointData", &mut grid.point_fields),
        ("CellData", &mut grid.cell_fields),
    ] {
        let Some(section) = child(piece, tag) else {
            continue;
        };
        for a in arrays(section) {
            let f = ctx.array(a)?;
            match fields.iter_mut().find(|x| x.name == f.name) {
                Some(existing) => existing.data.extend(f.data),
                None => fields.push(f),
            }
        }
    }
    Ok(())
}

/// Parse a `.pvd` collection: `(time, file)` per dataset, in time order.
pub fn parse_pvd(text: &str) -> Result<Vec<(f64, String)>, BridgeError> {
    const F: &str = "PVD";
    let doc = roxmltree::Document::parse(text).map_err(|e| BridgeError::new(F, format!("XML: {e}")))?;
    let mut out = Vec::new();
    for d in doc.descendants().filter(|n| n.has_tag_name("DataSet")) {
        let t = d
            .attribute("timestep")
            .and_then(|t| t.parse().ok())
            .ok_or_else(|| BridgeError::new(F, "DataSet without a numeric timestep"))?;
        let file = d
            .attribute("file")
            .ok_or_else(|| BridgeError::new(F, "DataSet without a file"))?;
        out.push((t, file.to_string()));
    }
    out.sort_by(|a: &(f64, String), b| a.0.total_cmp(&b.0));
    Ok(out)
}

// --- legacy (.vtk) --------------------------------------------------------------------------------

struct Cursor<'a> {
    b: &'a [u8],
    pos: usize,
    binary: bool,
}

impl<'a> Cursor<'a> {
    fn line(&mut self) -> &'a [u8] {
        let start = self.pos;
        while self.pos < self.b.len() && self.b[self.pos] != b'\n' {
            self.pos += 1;
        }
        let l = &self.b[start..self.pos];
        self.pos = (self.pos + 1).min(self.b.len());
        l
    }

    fn token(&mut self) -> Option<&'a str> {
        while self.pos < self.b.len() && self.b[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
        let start = self.pos;
        while self.pos < self.b.len() && !self.b[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
        (self.pos > start).then(|| std::str::from_utf8(&self.b[start..self.pos]).unwrap_or(""))
    }

    fn need(&mut self, what: &str) -> Result<&'a str, BridgeError> {
        self.token()
            .ok_or_else(|| BridgeError::new(LEGACY, format!("file ends where {what} was expected")))
    }

    fn count(&mut self, what: &str) -> Result<usize, BridgeError> {
        let w = self.need(what)?;
        w.parse()
            .map_err(|_| BridgeError::new(LEGACY, format!("{what}: expected a count, found `{w}`")))
    }

    fn ty(&mut self) -> Result<Scalar, BridgeError> {
        let w = self.need("a data type")?;
        Scalar::from_legacy(w).ok_or_else(|| BridgeError::new(LEGACY, format!("unknown data type `{w}`")))
    }

    /// `n` values of type `ty`. Binary legacy data are big-endian and start on the next line.
    fn values(&mut self, n: usize, ty: Scalar) -> Result<Vec<f64>, BridgeError> {
        if self.binary {
            self.line();
            let bytes = n * ty.bytes();
            let chunk = self
                .b
                .get(self.pos..self.pos + bytes)
                .ok_or_else(|| BridgeError::new(LEGACY, "binary block shorter than announced"))?;
            self.pos += bytes;
            Ok(scalars(chunk, ty, false))
        } else {
            (0..n)
                .map(|_| {
                    let w = self.need("a value")?;
                    w.parse()
                        .map_err(|_| BridgeError::new(LEGACY, format!("bad number `{w}`")))
                })
                .collect()
        }
    }
}

/// Parse a legacy `.vtk` file with an `UNSTRUCTURED_GRID` or `POLYDATA` dataset.
pub fn parse_legacy_vtk(bytes: &[u8]) -> Result<VtkGrid, BridgeError> {
    let mut c = Cursor {
        b: bytes,
        pos: 0,
        binary: false,
    };
    if !c.line().starts_with(b"# vtk DataFile") {
        return Err(BridgeError::new(LEGACY, "missing `# vtk DataFile` line"));
    }
    c.line(); // title
    c.binary = match c.need("ASCII or BINARY")? {
        "ASCII" => false,
        "BINARY" => true,
        other => return Err(BridgeError::new(LEGACY, format!("unknown encoding `{other}`"))),
    };
    if c.need("DATASET")? != "DATASET" {
        return Err(BridgeError::new(LEGACY, "expected DATASET"));
    }
    let kind = c.need("a dataset type")?;
    if kind != "UNSTRUCTURED_GRID" && kind != "POLYDATA" {
        return Err(BridgeError::new(
            LEGACY,
            format!("dataset `{kind}` is not supported"),
        ));
    }
    let mut g = VtkGrid {
        offsets: vec![0],
        ..VtkGrid::default()
    };
    // which attribute section the next arrays belong to: 0 = point, 1 = cell
    let mut section = 0usize;
    while let Some(key) = c.token() {
        match key {
            "POINTS" => {
                let n = c.count("POINTS")?;
                let ty = c.ty()?;
                let v = c.values(3 * n, ty)?;
                g.points = v.as_chunks::<3>().0.to_vec();
            }
            "CELLS" | "LINES" | "POLYGONS" | "VERTICES" | "TRIANGLE_STRIPS" => {
                read_legacy_cells(&mut c, &mut g)?;
            }
            "CELL_TYPES" => {
                let n = c.count("CELL_TYPES")?;
                c.values(n, Scalar::I32)?;
            }
            "POINT_DATA" => {
                c.count("POINT_DATA")?;
                section = 0;
            }
            "CELL_DATA" => {
                c.count("CELL_DATA")?;
                section = 1;
            }
            "SCALARS" => {
                let name = c.need("a SCALARS name")?.to_string();
                let ty = c.ty()?;
                // `numComp` is optional; the next token is either it or LOOKUP_TABLE
                let mut word = c.need("LOOKUP_TABLE")?;
                let mut components = 1;
                if let Ok(n) = word.parse::<usize>() {
                    components = n;
                    word = c.need("LOOKUP_TABLE")?;
                }
                if word != "LOOKUP_TABLE" {
                    return Err(BridgeError::new(
                        LEGACY,
                        format!("expected LOOKUP_TABLE, found `{word}`"),
                    ));
                }
                c.need("a table name")?;
                let tuples = legacy_tuples(&g, section);
                let data = c.values(components * tuples, ty)?;
                push_field(
                    &mut g,
                    section,
                    VtkField {
                        name,
                        components,
                        data,
                    },
                );
            }
            "VECTORS" | "NORMALS" => {
                let name = c.need("an array name")?.to_string();
                let ty = c.ty()?;
                let data = c.values(3 * legacy_tuples(&g, section), ty)?;
                push_field(
                    &mut g,
                    section,
                    VtkField {
                        name,
                        components: 3,
                        data,
                    },
                );
            }
            "FIELD" => {
                c.need("a FIELD name")?;
                let arrays = c.count("FIELD")?;
                for _ in 0..arrays {
                    let name = c.need("an array name")?.to_string();
                    let components = c.count("components")?;
                    let tuples = c.count("tuples")?;
                    let ty = c.ty()?;
                    let data = c.values(components * tuples, ty)?;
                    push_field(
                        &mut g,
                        section,
                        VtkField {
                            name,
                            components,
                            data,
                        },
                    );
                }
            }
            "METADATA" => {
                // informational lines, ended by a blank line
                c.line();
                while c.pos < c.b.len() && !c.line().iter().all(u8::is_ascii_whitespace) {}
            }
            other => return Err(BridgeError::new(LEGACY, format!("unknown section `{other}`"))),
        }
    }
    g.check(LEGACY)?;
    Ok(g)
}

fn legacy_tuples(g: &VtkGrid, section: usize) -> usize {
    if section == 0 {
        g.points.len()
    } else {
        g.cell_count()
    }
}

fn push_field(g: &mut VtkGrid, section: usize, f: VtkField) {
    if section == 0 {
        g.point_fields.push(f);
    } else {
        g.cell_fields.push(f);
    }
}

/// Connectivity in either layout: `n size` + `count id id …` per cell (before 5.1), or
/// `offsets connectivity` + `OFFSETS type …` + `CONNECTIVITY type …` (5.1 and later).
fn read_legacy_cells(c: &mut Cursor<'_>, g: &mut VtkGrid) -> Result<(), BridgeError> {
    let a = c.count("CELLS")?;
    let b = c.count("CELLS size")?;
    let saved = c.pos;
    if c.token() == Some("OFFSETS") {
        let ty = c.ty()?;
        let off = c.values(a, ty)?;
        if c.need("CONNECTIVITY")? != "CONNECTIVITY" {
            return Err(BridgeError::new(LEGACY, "expected CONNECTIVITY after OFFSETS"));
        }
        let ty = c.ty()?;
        let conn = c.values(b, ty)?;
        let start = g.connectivity.len() as u32;
        g.connectivity.extend(conn.iter().map(|n| *n as u32));
        g.offsets.extend(off.iter().skip(1).map(|o| *o as u32 + start));
        return Ok(());
    }
    c.pos = saved;
    let flat = c.values(b, Scalar::I32)?;
    let mut i = 0;
    for _ in 0..a {
        let count = *flat
            .get(i)
            .ok_or_else(|| BridgeError::new(LEGACY, "CELLS list shorter than announced"))?
            as usize;
        let nodes = flat
            .get(i + 1..i + 1 + count)
            .ok_or_else(|| BridgeError::new(LEGACY, "CELLS list shorter than announced"))?;
        g.connectivity.extend(nodes.iter().map(|n| *n as u32));
        g.offsets.push(g.connectivity.len() as u32);
        i += count + 1;
    }
    Ok(())
}

// --- writer ---------------------------------------------------------------------------------------

/// An ASCII `.vtu` of tetrahedra, with named scalar fields per point and per cell.
#[must_use]
pub fn vtu_text(
    points: &[[f64; 3]],
    tets: &[[u32; 4]],
    point_fields: &[(&str, &[f64])],
    cell_fields: &[(&str, &[f64])],
) -> String {
    let mut s = String::with_capacity(points.len() * 40 + tets.len() * 40 + 512);
    let _ = writeln!(
        s,
        "<?xml version=\"1.0\"?>\n<VTKFile type=\"UnstructuredGrid\" version=\"1.0\" byte_order=\"LittleEndian\" header_type=\"UInt32\">\n<UnstructuredGrid>\n<Piece NumberOfPoints=\"{}\" NumberOfCells=\"{}\">",
        points.len(),
        tets.len()
    );
    s.push_str("<Points>\n<DataArray type=\"Float64\" NumberOfComponents=\"3\" format=\"ascii\">\n");
    for p in points {
        let _ = writeln!(s, "{} {} {}", p[0], p[1], p[2]);
    }
    s.push_str("</DataArray>\n</Points>\n<Cells>\n<DataArray type=\"Int64\" Name=\"connectivity\" format=\"ascii\">\n");
    for t in tets {
        let _ = writeln!(s, "{} {} {} {}", t[0], t[1], t[2], t[3]);
    }
    s.push_str("</DataArray>\n<DataArray type=\"Int64\" Name=\"offsets\" format=\"ascii\">\n");
    for i in 1..=tets.len() {
        let _ = writeln!(s, "{}", i * 4);
    }
    s.push_str("</DataArray>\n<DataArray type=\"UInt8\" Name=\"types\" format=\"ascii\">\n");
    for _ in tets {
        s.push_str("10\n"); // VTK_TETRA
    }
    s.push_str("</DataArray>\n</Cells>\n");
    for (tag, fields) in [("PointData", point_fields), ("CellData", cell_fields)] {
        let _ = writeln!(s, "<{tag}>");
        for (name, data) in fields {
            let _ = writeln!(s, "<DataArray type=\"Float64\" Name=\"{name}\" format=\"ascii\">");
            for v in *data {
                let _ = writeln!(s, "{v}");
            }
            s.push_str("</DataArray>\n");
        }
        let _ = writeln!(s, "</{tag}>");
    }
    s.push_str("</Piece>\n</UnstructuredGrid>\n</VTKFile>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEGACY_ASCII_OLD: &str = "# vtk DataFile Version 3.0\ntitle\nASCII\nDATASET UNSTRUCTURED_GRID\n\
POINTS 4 float\n0 0 0 1 0 0 0 1 0 0 0 1\nCELLS 1 5\n4 0 1 2 3\nCELL_TYPES 1\n10\n\
POINT_DATA 4\nSCALARS Vm float 1\nLOOKUP_TABLE default\n-80 -70 -60 -50\n\
CELL_DATA 1\nSCALARS Scalars_ float\nLOOKUP_TABLE default\n12.5\n";

    #[test]
    fn writer_round_trips_through_the_reader() {
        let points = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.5]];
        let lat = [1.0, 2.0, 3.0, 4.5];
        let tag = [7.0];
        let text = vtu_text(&points, &[[0, 1, 2, 3]], &[("lat", &lat)], &[("tag", &tag)]);
        let g = parse_vtu(text.as_bytes()).unwrap();
        assert_eq!(g.points, points);
        assert_eq!(g.point_fields[0].data, lat);
        assert_eq!(g.cell_fields[0].data, tag);
        assert_eq!(g.cell_centres(), vec![[0.25, 0.25, 0.375]]);
    }

    #[test]
    fn legacy_ascii_holds_point_and_cell_data() {
        let g = parse_legacy_vtk(LEGACY_ASCII_OLD.as_bytes()).unwrap();
        assert_eq!(g.points.len(), 4);
        assert_eq!(g.connectivity, vec![0, 1, 2, 3]);
        assert_eq!(g.point_fields[0].name, "Vm");
        assert_eq!(g.point_fields[0].data, vec![-80.0, -70.0, -60.0, -50.0]);
        assert_eq!(g.cell_fields[0].data, vec![12.5]);
    }

    #[test]
    fn legacy_binary_is_big_endian() {
        let mut b =
            b"# vtk DataFile Version 3.0\nt\nBINARY\nDATASET UNSTRUCTURED_GRID\nPOINTS 2 float\n".to_vec();
        for v in [0.0f32, 0.0, 0.0, 1.0, 2.0, 3.0] {
            b.extend_from_slice(&v.to_be_bytes());
        }
        b.extend_from_slice(b"\nPOINT_DATA 2\nSCALARS v double 1\nLOOKUP_TABLE default\n");
        for v in [-1.5f64, 2.5] {
            b.extend_from_slice(&v.to_be_bytes());
        }
        b.push(b'\n');
        let g = parse_legacy_vtk(&b).unwrap();
        assert_eq!(g.points[1], [1.0, 2.0, 3.0]);
        assert_eq!(g.point_fields[0].data, vec![-1.5, 2.5]);
    }

    #[test]
    fn legacy_version_5_cell_layout() {
        let t = "# vtk DataFile Version 5.1\nt\nASCII\nDATASET UNSTRUCTURED_GRID\nPOINTS 4 double\n0 0 0 1 0 0 0 1 0 0 0 1\n\
CELLS 2 4\nOFFSETS vtktypeint64\n0 4\nCONNECTIVITY vtktypeint64\n3 2 1 0\nCELL_TYPES 1\n10\n";
        let g = parse_legacy_vtk(t.as_bytes()).unwrap();
        assert_eq!(g.connectivity, vec![3, 2, 1, 0]);
        assert_eq!(g.offsets, vec![0, 4]);
    }

    #[test]
    fn pvd_lists_files_in_time_order() {
        let t = r#"<VTKFile type="Collection"><Collection>
            <DataSet timestep="2.5" file="b.vtu"/><DataSet timestep="0" file="a.vtu"/></Collection></VTKFile>"#;
        assert_eq!(
            parse_pvd(t).unwrap(),
            vec![(0.0, "a.vtu".to_string()), (2.5, "b.vtu".to_string())]
        );
    }

    #[test]
    fn unsupported_files_say_why() {
        let e = parse_vtu(b"<VTKFile type=\"PUnstructuredGrid\"></VTKFile>").unwrap_err();
        assert!(e.to_string().contains("pvtu"), "{e}");
        let e =
            parse_vtu(b"<VTKFile type=\"UnstructuredGrid\" compressor=\"vtkLZ4DataCompressor\"></VTKFile>")
                .unwrap_err();
        assert!(e.to_string().contains("compressor"), "{e}");
        assert!(parse_legacy_vtk(b"nonsense").is_err());
        assert!(
            parse_legacy_vtk(b"# vtk DataFile Version 3.0\nt\nASCII\nDATASET STRUCTURED_POINTS\n").is_err()
        );
    }

    #[test]
    fn cell_data_length_must_match() {
        let text = vtu_text(
            &[[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            &[[0, 1, 2, 3]],
            &[("a", &[1.0])],
            &[],
        );
        assert!(parse_vtu(text.as_bytes()).is_err());
    }
}
