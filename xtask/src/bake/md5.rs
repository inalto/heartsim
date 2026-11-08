//! MD5 (RFC 1321), used only to verify downloads against the checksums Zenodo publishes. A
//! dependency-free 60-line implementation beats pulling a crypto crate into the build tool for
//! a non-security use.

const S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5,
    9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15, 21, 6, 10, 15, 21, 6,
    10, 15, 21, 6, 10, 15, 21,
];

const K: [u32; 64] = [
    0xd76a_a478,
    0xe8c7_b756,
    0x2420_70db,
    0xc1bd_ceee,
    0xf57c_0faf,
    0x4787_c62a,
    0xa830_4613,
    0xfd46_9501,
    0x6980_98d8,
    0x8b44_f7af,
    0xffff_5bb1,
    0x895c_d7be,
    0x6b90_1122,
    0xfd98_7193,
    0xa679_438e,
    0x49b4_0821,
    0xf61e_2562,
    0xc040_b340,
    0x265e_5a51,
    0xe9b6_c7aa,
    0xd62f_105d,
    0x0244_1453,
    0xd8a1_e681,
    0xe7d3_fbc8,
    0x21e1_cde6,
    0xc337_07d6,
    0xf4d5_0d87,
    0x455a_14ed,
    0xa9e3_e905,
    0xfcef_a3f8,
    0x676f_02d9,
    0x8d2a_4c8a,
    0xfffa_3942,
    0x8771_f681,
    0x6d9d_6122,
    0xfde5_380c,
    0xa4be_ea44,
    0x4bde_cfa9,
    0xf6bb_4b60,
    0xbebf_bc70,
    0x289b_7ec6,
    0xeaa1_27fa,
    0xd4ef_3085,
    0x0488_1d05,
    0xd9d4_d039,
    0xe6db_99e5,
    0x1fa2_7cf8,
    0xc4ac_5665,
    0xf429_2244,
    0x432a_ff97,
    0xab94_23a7,
    0xfc93_a039,
    0x655b_59c3,
    0x8f0c_cc92,
    0xffef_f47d,
    0x8584_5dd1,
    0x6fa8_7e4f,
    0xfe2c_e6e0,
    0xa301_4314,
    0x4e08_11a1,
    0xf753_7e82,
    0xbd3a_f235,
    0x2ad7_d2bb,
    0xeb86_d391,
];

/// Streaming MD5 state.
pub struct Md5 {
    state: [u32; 4],
    buffer: Vec<u8>,
    length: u64,
}

impl Default for Md5 {
    fn default() -> Self {
        Self::new()
    }
}

impl Md5 {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476],
            buffer: Vec::with_capacity(64),
            length: 0,
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.length += data.len() as u64;
        self.buffer.extend_from_slice(data);
        let whole = self.buffer.len() / 64 * 64;
        for block in self.buffer[..whole].as_chunks::<64>().0 {
            compress(&mut self.state, block);
        }
        self.buffer.drain(..whole);
    }

    /// Lower-case hex digest.
    #[must_use]
    pub fn finish(mut self) -> String {
        let bits = self.length.wrapping_mul(8);
        self.buffer.push(0x80);
        while !self.buffer.len().is_multiple_of(64) {
            if self.buffer.len() % 64 == 56 {
                self.buffer.extend_from_slice(&bits.to_le_bytes());
            } else {
                self.buffer.push(0);
            }
        }
        let blocks = std::mem::take(&mut self.buffer);
        for block in blocks.as_chunks::<64>().0 {
            compress(&mut self.state, block);
        }
        let mut out = String::with_capacity(32);
        for word in self.state {
            for byte in word.to_le_bytes() {
                use std::fmt::Write as _;
                write!(out, "{byte:02x}").expect("string write");
            }
        }
        out
    }
}

fn compress(state: &mut [u32; 4], block: &[u8; 64]) {
    let mut m = [0u32; 16];
    for (i, w) in m.iter_mut().enumerate() {
        *w = u32::from_le_bytes([block[4 * i], block[4 * i + 1], block[4 * i + 2], block[4 * i + 3]]);
    }
    let [mut a, mut b, mut c, mut d] = *state;
    for i in 0..64 {
        let (f, g) = match i / 16 {
            0 => ((b & c) | (!b & d), i),
            1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
            2 => (b ^ c ^ d, (3 * i + 5) % 16),
            _ => (c ^ (b | !d), (7 * i) % 16),
        };
        let f = f.wrapping_add(a).wrapping_add(K[i]).wrapping_add(m[g]);
        a = d;
        d = c;
        c = b;
        b = b.wrapping_add(f.rotate_left(S[i]));
    }
    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
}

/// MD5 of a file, streamed.
pub fn file_md5(path: &std::path::Path) -> std::io::Result<String> {
    use std::io::Read as _;
    let mut f = std::fs::File::open(path)?;
    let mut h = Md5::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md5(s: &str) -> String {
        let mut h = Md5::new();
        h.update(s.as_bytes());
        h.finish()
    }

    #[test]
    fn rfc_1321_vectors() {
        assert_eq!(md5(""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5("abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            md5("12345678901234567890123456789012345678901234567890123456789012345678901234567890"),
            "57edf4a22be3c955ac49da2e2107b67a"
        );
        // split updates across a block boundary give the same digest
        let mut h = Md5::new();
        h.update(&[b'a'; 60]);
        h.update(&[b'a'; 40]);
        let mut one = Md5::new();
        one.update(&[b'a'; 100]);
        assert_eq!(h.finish(), one.finish());
    }
}
