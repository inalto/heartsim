//! Label volumes: a regular grid of tissue labels (`u8`) in the heart frame, the anatomy the echo
//! cross-section samples (`docs/anatomy-pack.md`, *Label volumes*).
//!
//! On disk a volume is `HSVL` v1, little-endian:
//!
//! ```text
//! magic "HSVL" · version u32 = 1 · CRC-32 u32 of every byte after it ·
//! origin_mm 3×f32 (corner of voxel (0,0,0)) · spacing_mm f32 · dims 3×u32 (nx, ny, nz) ·
//! run count u32 · runs (label u8, length u16 ≥ 1) …
//! ```
//!
//! (The CRC sits in the header, not at the end: a file ending in the CRC of its own bytes always
//! has the same whole-file CRC-32, which would make the manifest's checksum useless.)
//!
//! Voxel `(i, j, k)` covers `origin + [i, i+1) × spacing` (and so on per axis); the runs list the
//! voxels with `i` fastest, then `j`, then `k`. A run longer than 65 535 voxels is split. The CRC
//! makes a file self-checking even outside a pack (the pack manifest carries its own CRC too).

use glam::Vec3;

/// Magic of a label-volume file.
pub const VOLUME_MAGIC: &[u8; 4] = b"HSVL";
/// Record version of [`VOLUME_MAGIC`] files.
pub const VOLUME_VERSION: u32 = 1;

/// Tissue label of a voxel or a cross-section sample. **The numeric values are stable** (they are
/// stored in pack files and may be uploaded to a GPU texture): append, never renumber.
///
/// Labels 1–12 are baked into the heart box from the Rodero heart, 30–34 into the thorax box from
/// the KCL torso; 20–26 are never stored, they are the designed parts `heartsim-echo` evaluates
/// on top of the volumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Label {
    /// Air outside the body, or nothing known (outside both boxes).
    Outside = 0,
    LvMyocardium = 1,
    RvMyocardium = 2,
    LaMyocardium = 3,
    RaMyocardium = 4,
    LvBlood = 5,
    RvBlood = 6,
    LaBlood = 7,
    RaBlood = 8,
    AortaWall = 9,
    AortaLumen = 10,
    PulmonaryArteryWall = 11,
    PulmonaryArteryLumen = 12,
    /// Designed: a valve leaflet or semilunar cusp.
    ValveLeaflet = 20,
    /// Designed: an LV papillary muscle.
    PapillaryMuscle = 21,
    /// Designed: the RV moderator band.
    ModeratorBand = 22,
    /// Designed: the pericardium, a thin shell outside the epicardium.
    Pericardium = 23,
    /// Ribs, sternum, costal cartilage, spine (one KCL tag).
    Bone = 30,
    Lung = 31,
    Liver = 32,
    /// Every other tissue inside the skin (muscle, fat, other organs, the KCL subject's heart
    /// where the pack heart does not reach).
    SoftTissue = 33,
    /// Blood of the KCL subject's own great vessels outside the pack heart (descending aorta,
    /// pulmonary arteries, caval veins).
    ThoracicBlood = 34,
}

impl Label {
    /// Every label, in numeric order.
    pub const ALL: [Self; 22] = [
        Self::Outside,
        Self::LvMyocardium,
        Self::RvMyocardium,
        Self::LaMyocardium,
        Self::RaMyocardium,
        Self::LvBlood,
        Self::RvBlood,
        Self::LaBlood,
        Self::RaBlood,
        Self::AortaWall,
        Self::AortaLumen,
        Self::PulmonaryArteryWall,
        Self::PulmonaryArteryLumen,
        Self::ValveLeaflet,
        Self::PapillaryMuscle,
        Self::ModeratorBand,
        Self::Pericardium,
        Self::Bone,
        Self::Lung,
        Self::Liver,
        Self::SoftTissue,
        Self::ThoracicBlood,
    ];

    /// The label of a stored byte, `None` for an unassigned value.
    #[must_use]
    pub const fn from_u8(b: u8) -> Option<Self> {
        Some(match b {
            0 => Self::Outside,
            1 => Self::LvMyocardium,
            2 => Self::RvMyocardium,
            3 => Self::LaMyocardium,
            4 => Self::RaMyocardium,
            5 => Self::LvBlood,
            6 => Self::RvBlood,
            7 => Self::LaBlood,
            8 => Self::RaBlood,
            9 => Self::AortaWall,
            10 => Self::AortaLumen,
            11 => Self::PulmonaryArteryWall,
            12 => Self::PulmonaryArteryLumen,
            20 => Self::ValveLeaflet,
            21 => Self::PapillaryMuscle,
            22 => Self::ModeratorBand,
            23 => Self::Pericardium,
            30 => Self::Bone,
            31 => Self::Lung,
            32 => Self::Liver,
            33 => Self::SoftTissue,
            34 => Self::ThoracicBlood,
            _ => return None,
        })
    }

    /// The stored byte.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Stable English identifier (snake case), for logs, tests and locale keys.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Outside => "outside",
            Self::LvMyocardium => "lv_myocardium",
            Self::RvMyocardium => "rv_myocardium",
            Self::LaMyocardium => "la_myocardium",
            Self::RaMyocardium => "ra_myocardium",
            Self::LvBlood => "lv_blood",
            Self::RvBlood => "rv_blood",
            Self::LaBlood => "la_blood",
            Self::RaBlood => "ra_blood",
            Self::AortaWall => "aorta_wall",
            Self::AortaLumen => "aorta_lumen",
            Self::PulmonaryArteryWall => "pulmonary_artery_wall",
            Self::PulmonaryArteryLumen => "pulmonary_artery_lumen",
            Self::ValveLeaflet => "valve_leaflet",
            Self::PapillaryMuscle => "papillary_muscle",
            Self::ModeratorBand => "moderator_band",
            Self::Pericardium => "pericardium",
            Self::Bone => "bone",
            Self::Lung => "lung",
            Self::Liver => "liver",
            Self::SoftTissue => "soft_tissue",
            Self::ThoracicBlood => "thoracic_blood",
        }
    }

    /// Blood of a chamber or vessel.
    #[must_use]
    pub const fn is_blood(self) -> bool {
        matches!(
            self,
            Self::LvBlood
                | Self::RvBlood
                | Self::LaBlood
                | Self::RaBlood
                | Self::AortaLumen
                | Self::PulmonaryArteryLumen
                | Self::ThoracicBlood
        )
    }

    /// Working myocardium of one of the four chambers.
    #[must_use]
    pub const fn is_myocardium(self) -> bool {
        matches!(
            self,
            Self::LvMyocardium | Self::RvMyocardium | Self::LaMyocardium | Self::RaMyocardium
        )
    }

    /// Part of the pack heart (heart-box labels and the designed parts).
    #[must_use]
    pub const fn is_heart(self) -> bool {
        matches!(self as u8, 1..=23)
    }

    /// One of the designed parts (never stored in a file).
    #[must_use]
    pub const fn is_designed(self) -> bool {
        matches!(
            self,
            Self::ValveLeaflet | Self::PapillaryMuscle | Self::ModeratorBand | Self::Pericardium
        )
    }

    /// Bone or aerated lung: what stops a geometric line of sight in the cross-section.
    #[must_use]
    pub const fn occludes(self) -> bool {
        matches!(self, Self::Bone | Self::Lung)
    }
}

/// Why a label-volume file was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VolumeError {
    #[error("not a label volume (magic {0:?})")]
    Magic([u8; 4]),
    #[error("label volume version {0} is not supported (this build reads {VOLUME_VERSION})")]
    Version(u32),
    #[error("label volume truncated")]
    Truncated,
    #[error("label volume checksum mismatch (stored {stored:08x}, computed {computed:08x})")]
    Checksum { stored: u32, computed: u32 },
    #[error("label volume runs cover {found} voxels, the dimensions need {expected}")]
    RunLength { expected: u64, found: u64 },
    #[error("label volume has invalid geometry: {0}")]
    Geometry(String),
    #[error("label volume holds unknown label {0}")]
    UnknownLabel(u8),
}

/// A dense `u8` label grid (see the module docs for the layout and file format).
#[derive(Debug, Clone, PartialEq)]
pub struct LabelVolume {
    /// Heart-frame corner of voxel `(0, 0, 0)`, mm.
    pub origin_mm: Vec3,
    /// Edge of a (cubic) voxel, mm.
    pub spacing_mm: f32,
    /// Voxels along x, y, z.
    pub dims: [u32; 3],
    /// Label bytes, `i + nx·(j + ny·k)`.
    pub labels: Vec<u8>,
}

impl LabelVolume {
    /// A volume of `fill` voxels.
    ///
    /// # Panics
    /// If a dimension is zero or the voxel count exceeds `usize`.
    #[must_use]
    pub fn new(origin_mm: Vec3, spacing_mm: f32, dims: [u32; 3], fill: Label) -> Self {
        assert!(dims.iter().all(|d| *d > 0), "empty label volume");
        let n = dims.iter().map(|d| *d as usize).product();
        Self {
            origin_mm,
            spacing_mm,
            dims,
            labels: vec![fill as u8; n],
        }
    }

    /// Number of voxels.
    #[must_use]
    pub fn len(&self) -> usize {
        self.labels.len()
    }

    /// Whether the volume has no voxel (never for a constructed volume).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.labels.is_empty()
    }

    /// Linear index of voxel `(i, j, k)`.
    #[must_use]
    pub fn index(&self, i: u32, j: u32, k: u32) -> usize {
        let [nx, ny, _] = self.dims;
        i as usize + nx as usize * (j as usize + ny as usize * k as usize)
    }

    /// Voxel coordinates of linear index `idx`.
    #[must_use]
    pub fn coords(&self, idx: usize) -> [u32; 3] {
        let nx = self.dims[0] as usize;
        let ny = self.dims[1] as usize;
        [
            (idx % nx) as u32,
            ((idx / nx) % ny) as u32,
            (idx / (nx * ny)) as u32,
        ]
    }

    /// Heart-frame centre of voxel `(i, j, k)`, mm.
    #[must_use]
    pub fn centre(&self, i: u32, j: u32, k: u32) -> Vec3 {
        self.origin_mm + (Vec3::new(i as f32, j as f32, k as f32) + 0.5) * self.spacing_mm
    }

    /// Far corner of the box, mm.
    #[must_use]
    pub fn max_mm(&self) -> Vec3 {
        self.origin_mm
            + Vec3::new(self.dims[0] as f32, self.dims[1] as f32, self.dims[2] as f32) * self.spacing_mm
    }

    /// The voxel containing `p_mm`, `None` outside the box (and for a non-finite point).
    #[must_use]
    pub fn voxel_of(&self, p_mm: Vec3) -> Option<[u32; 3]> {
        let q = (p_mm - self.origin_mm) / self.spacing_mm;
        let n = Vec3::new(self.dims[0] as f32, self.dims[1] as f32, self.dims[2] as f32);
        // NaN fails both comparisons and is refused here
        if !(q.cmpge(Vec3::ZERO).all() && q.cmplt(n).all()) {
            return None;
        }
        Some([
            (q.x as u32).min(self.dims[0] - 1),
            (q.y as u32).min(self.dims[1] - 1),
            (q.z as u32).min(self.dims[2] - 1),
        ])
    }

    /// Raw label byte at `p_mm` by nearest-voxel lookup, `None` outside the box.
    #[must_use]
    pub fn get(&self, p_mm: Vec3) -> Option<u8> {
        self.voxel_of(p_mm)
            .map(|[i, j, k]| self.labels[self.index(i, j, k)])
    }

    /// Label at `p_mm` (nearest voxel); [`Label::Outside`] outside the box.
    #[must_use]
    pub fn label_at(&self, p_mm: Vec3) -> Label {
        self.get(p_mm).and_then(Label::from_u8).unwrap_or(Label::Outside)
    }

    /// Number of voxels holding `label`.
    #[must_use]
    pub fn count(&self, label: Label) -> usize {
        let b = label as u8;
        self.labels.iter().fold(0usize, |n, x| n + usize::from(*x == b))
    }

    /// Volume of one voxel, mm³.
    #[must_use]
    pub fn voxel_volume_mm3(&self) -> f64 {
        f64::from(self.spacing_mm).powi(3)
    }

    /// The `HSVL` v1 file (see the module docs).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut runs: Vec<(u8, u16)> = Vec::new();
        for &b in &self.labels {
            match runs.last_mut() {
                Some((l, n)) if *l == b && *n < u16::MAX => *n += 1,
                _ => runs.push((b, 1)),
            }
        }
        let mut out = Vec::with_capacity(40 + runs.len() * 3);
        out.extend_from_slice(VOLUME_MAGIC);
        out.extend_from_slice(&VOLUME_VERSION.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        for a in 0..3 {
            out.extend_from_slice(&self.origin_mm[a].to_le_bytes());
        }
        out.extend_from_slice(&self.spacing_mm.to_le_bytes());
        for d in self.dims {
            out.extend_from_slice(&d.to_le_bytes());
        }
        out.extend_from_slice(&(runs.len() as u32).to_le_bytes());
        for (l, n) in runs {
            out.push(l);
            out.extend_from_slice(&n.to_le_bytes());
        }
        let crc = crc32fast::hash(&out[12..]);
        out[8..12].copy_from_slice(&crc.to_le_bytes());
        out
    }

    /// Read an `HSVL` v1 file: magic, version, checksum, geometry, run total and every label are
    /// checked, so a volume that decodes is complete and holds only known labels.
    ///
    /// # Errors
    /// See [`VolumeError`].
    pub fn decode(bytes: &[u8]) -> Result<Self, VolumeError> {
        const HEADER: usize = 4 + 4 + 4 + 12 + 4 + 12 + 4;
        if bytes.len() < 8 {
            return Err(VolumeError::Truncated);
        }
        let magic: [u8; 4] = bytes[0..4].try_into().map_err(|_| VolumeError::Truncated)?;
        if &magic != VOLUME_MAGIC {
            return Err(VolumeError::Magic(magic));
        }
        let u32_at = |o: usize| -> Result<u32, VolumeError> {
            bytes
                .get(o..o + 4)
                .and_then(|s| s.try_into().ok())
                .map(u32::from_le_bytes)
                .ok_or(VolumeError::Truncated)
        };
        let f32_at = |o: usize| u32_at(o).map(f32::from_bits);
        let version = u32_at(4)?;
        if version != VOLUME_VERSION {
            return Err(VolumeError::Version(version));
        }
        let stored = u32_at(8)?;
        let computed = crc32fast::hash(bytes.get(12..).unwrap_or_default());
        if stored != computed {
            return Err(VolumeError::Checksum { stored, computed });
        }
        if bytes.len() < HEADER {
            return Err(VolumeError::Truncated);
        }
        let body = bytes;
        let origin_mm = Vec3::new(f32_at(12)?, f32_at(16)?, f32_at(20)?);
        let spacing_mm = f32_at(24)?;
        let dims = [u32_at(28)?, u32_at(32)?, u32_at(36)?];
        let runs = u32_at(40)? as usize;
        if !(origin_mm.is_finite() && spacing_mm.is_finite() && spacing_mm > 0.0) {
            return Err(VolumeError::Geometry(format!(
                "origin {origin_mm}, spacing {spacing_mm}"
            )));
        }
        let expected: u64 = dims.iter().map(|d| u64::from(*d)).product();
        if expected == 0 || expected > 1 << 31 {
            return Err(VolumeError::Geometry(format!("dimensions {dims:?}")));
        }
        if body.len() != HEADER + runs * 3 {
            return Err(VolumeError::Truncated);
        }
        let mut labels = Vec::with_capacity(expected as usize);
        let mut known = [false; 256];
        for l in Label::ALL {
            known[l as usize] = true;
        }
        let mut found = 0u64;
        for r in body[HEADER..].as_chunks::<3>().0 {
            let (l, n) = (r[0], u16::from_le_bytes([r[1], r[2]]));
            if !known[l as usize] {
                return Err(VolumeError::UnknownLabel(l));
            }
            found += u64::from(n);
            if n == 0 || found > expected {
                return Err(VolumeError::RunLength { expected, found });
            }
            labels.resize(labels.len() + n as usize, l);
        }
        if found != expected {
            return Err(VolumeError::RunLength { expected, found });
        }
        Ok(Self {
            origin_mm,
            spacing_mm,
            dims,
            labels,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> LabelVolume {
        let mut v = LabelVolume::new(Vec3::new(-10.0, -5.0, 2.0), 1.5, [7, 5, 4], Label::Outside);
        for idx in 0..v.len() {
            let [i, j, k] = v.coords(idx);
            v.labels[idx] = Label::ALL[((i * 3 + j * 5 + k) % 22) as usize] as u8;
        }
        // one long run past u16::MAX is exercised separately
        v
    }

    #[test]
    fn round_trip() {
        let v = sample();
        let back = LabelVolume::decode(&v.encode()).expect("decodes");
        assert_eq!(back, v);
        // a run longer than a u16 is split and re-joined
        let big = LabelVolume::new(Vec3::ZERO, 1.0, [300, 300, 2], Label::Lung);
        assert_eq!(LabelVolume::decode(&big.encode()).expect("big"), big);
    }

    #[test]
    fn corruption_is_refused() {
        let v = sample();
        let mut bytes = v.encode();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0x40;
        assert!(matches!(
            LabelVolume::decode(&bytes),
            Err(VolumeError::Checksum { .. })
        ));
        let bytes = v.encode();
        assert!(matches!(
            LabelVolume::decode(&bytes[..bytes.len() - 7]),
            Err(VolumeError::Checksum { .. })
        ));
        assert_eq!(LabelVolume::decode(&bytes[..6]), Err(VolumeError::Truncated));
        // two different volumes give different whole-file checksums (the manifest relies on it)
        let other = LabelVolume::new(Vec3::ZERO, 1.0, [3, 3, 3], Label::Lung);
        assert_ne!(crc32fast::hash(&bytes), crc32fast::hash(&other.encode()));
        let mut bad = bytes.clone();
        bad[0] = b'X';
        assert!(matches!(LabelVolume::decode(&bad), Err(VolumeError::Magic(_))));
    }

    #[test]
    fn lookup_and_bounds() {
        let v = sample();
        assert_eq!(v.label_at(Vec3::new(-100.0, 0.0, 3.0)), Label::Outside);
        assert_eq!(v.label_at(Vec3::NAN), Label::Outside);
        assert_eq!(v.get(v.max_mm()), None);
        let c = v.centre(3, 2, 1);
        assert_eq!(v.voxel_of(c), Some([3, 2, 1]));
        assert_eq!(v.get(c), Some(v.labels[v.index(3, 2, 1)]));
        assert_eq!(v.voxel_of(v.origin_mm), Some([0, 0, 0]));
    }

    #[test]
    fn labels_are_stable_and_unique() {
        for l in Label::ALL {
            assert_eq!(Label::from_u8(l.to_u8()), Some(l));
        }
        assert_eq!(Label::LvBlood as u8, 5);
        assert_eq!(Label::Bone as u8, 30);
        assert_eq!(Label::from_u8(13), None);
        assert!(Label::Pericardium.is_heart() && !Label::Bone.is_heart());
    }
}
