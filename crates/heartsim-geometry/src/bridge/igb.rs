//! IGB, openCARP's binary output (user's manual §"IGB file"): a 1024-byte ASCII header of
//! whitespace-separated `keyword:value` pairs, padded and ended by a form feed, then `x·y·z·t` samples.
//! For unstructured grids only the product `x·y·z` means anything: it is the node count.

use super::BridgeError;

const F: &str = "IGB";
/// Size of the header, bytes.
pub const HEADER_BYTES: usize = 1024;
const FORM_FEED: u8 = 12;

/// Sample types this reader understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleType {
    Byte,
    Short,
    Int,
    Float,
    Double,
}

impl SampleType {
    fn from_keyword(s: &str) -> Option<Self> {
        Some(match s {
            "byte" | "char" => Self::Byte,
            "short" => Self::Short,
            "int" | "long" => Self::Int,
            "float" => Self::Float,
            "double" => Self::Double,
            _ => return None,
        })
    }

    fn bytes(self) -> usize {
        match self {
            Self::Byte => 1,
            Self::Short => 2,
            Self::Int | Self::Float => 4,
            Self::Double => 8,
        }
    }
}

/// A decoded IGB series.
#[derive(Debug, Clone, PartialEq)]
pub struct IgbSeries {
    /// `x·y·z`: samples per frame (nodes).
    pub nodes: usize,
    /// Complete frames present.
    pub frames: usize,
    /// Frames the header announces (more than `frames` when a run stopped early).
    pub frames_announced: usize,
    pub org_t_ms: f64,
    /// Time between frames, ms (`dim_t / (t − 1)`, or `inc_t`).
    pub dt_ms: f64,
    /// `unites`: the unit of the samples (`mV` for V_m).
    pub units: String,
    /// `frames` × `nodes` values, frame after frame, with `facteur` and `zero` applied.
    pub data: Vec<f32>,
}

impl IgbSeries {
    /// Frame `k`.
    #[must_use]
    pub fn frame(&self, k: usize) -> &[f32] {
        &self.data[k * self.nodes..(k + 1) * self.nodes]
    }

    /// Time of frame `k`, ms.
    #[must_use]
    pub fn time_ms(&self, k: usize) -> f64 {
        self.org_t_ms + k as f64 * self.dt_ms
    }
}

/// Decode an IGB file. A file shorter than its header announces yields the complete frames it holds.
pub fn parse_igb(bytes: &[u8]) -> Result<IgbSeries, BridgeError> {
    if bytes.len() < HEADER_BYTES {
        return Err(BridgeError::new(
            F,
            format!("{} bytes is shorter than the header", bytes.len()),
        ));
    }
    let head = &bytes[..HEADER_BYTES];
    let end = head.iter().position(|b| *b == FORM_FEED).unwrap_or(HEADER_BYTES);
    let text = std::str::from_utf8(&head[..end]).map_err(|_| BridgeError::new(F, "header is not text"))?;
    let (mut dims, mut t) = ([1usize; 3], None);
    let (mut sample, mut little) = (None, true);
    let (mut org_t, mut dim_t, mut inc_t) = (0.0f64, None, None);
    let (mut facteur, mut zero) = (1.0f64, 0.0f64);
    let mut units = String::new();
    for token in text.split_ascii_whitespace() {
        let Some((key, value)) = token.split_once(':') else {
            continue;
        };
        let num = || {
            value
                .parse::<f64>()
                .map_err(|_| BridgeError::new(F, format!("bad value in `{token}`")))
        };
        let int = || {
            value
                .parse::<usize>()
                .map_err(|_| BridgeError::new(F, format!("bad value in `{token}`")))
        };
        match key {
            "x" => dims[0] = int()?,
            "y" => dims[1] = int()?,
            "z" => dims[2] = int()?,
            "t" => t = Some(int()?),
            "type" => {
                sample = Some(
                    SampleType::from_keyword(value)
                        .ok_or_else(|| BridgeError::new(F, format!("unsupported type `{value}`")))?,
                );
            }
            "systeme" => match value {
                "little_endian" => little = true,
                "big_endian" => little = false,
                other => return Err(BridgeError::new(F, format!("unsupported systeme `{other}`"))),
            },
            "org_t" => org_t = num()?,
            "dim_t" => dim_t = Some(num()?),
            "inc_t" => inc_t = Some(num()?),
            "facteur" => facteur = num()?,
            "zero" => zero = num()?,
            "unites" => units = value.to_string(),
            _ => {}
        }
    }
    let t = t.ok_or_else(|| BridgeError::new(F, "header has no `t`"))?;
    let sample = sample.ok_or_else(|| BridgeError::new(F, "header has no `type`"))?;
    let nodes = dims[0] * dims[1] * dims[2];
    if nodes == 0 {
        return Err(BridgeError::new(F, "x·y·z is zero"));
    }
    let dt_ms = inc_t
        .or_else(|| dim_t.filter(|_| t > 1).map(|d| d / (t - 1) as f64))
        .unwrap_or(1.0);
    let body = &bytes[HEADER_BYTES..];
    let frame_bytes = nodes * sample.bytes();
    let frames = (body.len() / frame_bytes).min(t);
    let mut data = Vec::with_capacity(frames * nodes);
    for c in body[..frames * frame_bytes].chunks_exact(sample.bytes()) {
        let raw = decode(sample, c, little);
        data.push((raw * facteur + zero) as f32);
    }
    Ok(IgbSeries {
        nodes,
        frames,
        frames_announced: t,
        org_t_ms: org_t,
        dt_ms,
        units,
        data,
    })
}

fn decode(sample: SampleType, c: &[u8], little: bool) -> f64 {
    let word = |n: usize| -> [u8; 8] {
        let mut b = [0u8; 8];
        for (i, v) in c.iter().take(n).enumerate() {
            b[if little { i } else { n - 1 - i }] = *v;
        }
        b
    };
    match sample {
        SampleType::Byte => f64::from(c[0]),
        SampleType::Short => {
            let b = word(2);
            f64::from(i16::from_le_bytes([b[0], b[1]]))
        }
        SampleType::Int => {
            let b = word(4);
            f64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        }
        SampleType::Float => {
            let b = word(4);
            f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        }
        SampleType::Double => f64::from_le_bytes(word(8)),
    }
}

/// Encode `frames` frames of `nodes` single-precision samples as little-endian IGB.
///
/// # Panics
/// If `data` is not `nodes · frames` long, or the header text outgrows its 1024 bytes (an absurdly long
/// `units` string).
#[must_use]
pub fn igb_bytes(nodes: usize, data: &[f32], org_t_ms: f64, dt_ms: f64, units: &str) -> Vec<u8> {
    let frames = data.len().checked_div(nodes).unwrap_or(0);
    assert_eq!(data.len(), nodes * frames, "whole frames only");
    let dim_t = dt_ms * frames.saturating_sub(1) as f64;
    let header = format!(
        "x:{nodes} y:1 z:1 t:{frames} type:float systeme:little_endian org_t:{org_t_ms} dim_t:{dim_t} \
         inc_t:{dt_ms} unites_t:ms unites:{units} facteur:1 zero:0\r\n"
    );
    assert!(header.len() < HEADER_BYTES, "IGB header too long");
    let mut out = Vec::with_capacity(HEADER_BYTES + data.len() * 4);
    out.extend_from_slice(header.as_bytes());
    out.resize(HEADER_BYTES - 1, b' ');
    out.push(FORM_FEED);
    for v in data {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A header built by hand from the manual's example, not by our writer.
    fn manual_file(extra: &str, body: &[u8]) -> Vec<u8> {
        let head = format!(
            "x:2 y:1 z:1 t:2 type:float systeme:little_endian org_t:5 dim_t:10 unites_x:um unites_y:um \
             unites_z:um unites_t:ms unites:mV facteur:1 zero:0 {extra}"
        );
        let mut b = head.into_bytes();
        b.resize(HEADER_BYTES, b' ');
        b.extend_from_slice(body);
        b
    }

    #[test]
    fn reads_the_manuals_header_and_frames() {
        let body: Vec<u8> = [-80.0f32, -75.0, 10.0, 20.0]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let s = parse_igb(&manual_file("", &body)).unwrap();
        assert_eq!((s.nodes, s.frames, s.units.as_str()), (2, 2, "mV"));
        assert_eq!(s.dt_ms, 10.0);
        assert_eq!(s.time_ms(1), 15.0);
        assert_eq!(s.frame(0), &[-80.0, -75.0]);
        assert_eq!(s.frame(1), &[10.0, 20.0]);
    }

    #[test]
    fn writer_round_trips_and_keeps_data_at_byte_1024() {
        let data = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0];
        let b = igb_bytes(3, &data, 2.0, 0.5, "mV");
        assert_eq!(b.len(), HEADER_BYTES + 24);
        assert_eq!(b[HEADER_BYTES - 1], 12);
        let s = parse_igb(&b).unwrap();
        assert_eq!(s.data, data);
        assert_eq!((s.org_t_ms, s.dt_ms), (2.0, 0.5));
    }

    #[test]
    fn scaling_and_byte_order_are_honoured() {
        let head = "x:1 y:1 z:1 t:2 type:short systeme:big_endian facteur:0.5 zero:-10";
        let mut b = head.as_bytes().to_vec();
        b.resize(HEADER_BYTES, b' ');
        b.extend_from_slice(&[0x00, 0x04, 0xff, 0xfe]); // 4, -2
        let s = parse_igb(&b).unwrap();
        assert_eq!(s.data, vec![-8.0, -11.0]);
    }

    #[test]
    fn a_truncated_run_gives_its_complete_frames() {
        let body: Vec<u8> = [1.0f32, 2.0, 3.0].iter().flat_map(|v| v.to_le_bytes()).collect();
        let s = parse_igb(&manual_file("", &body)).unwrap();
        assert_eq!((s.frames, s.frames_announced), (1, 2));
    }

    #[test]
    fn bad_files_are_errors() {
        assert!(parse_igb(&[0; 10]).is_err());
        assert!(parse_igb(&manual_file("type:vec9q", &[])).is_err());
        let mut no_t = b"x:1 type:float".to_vec();
        no_t.resize(HEADER_BYTES, b' ');
        assert!(parse_igb(&no_t).is_err());
    }
}
