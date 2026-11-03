//! Priority queue of pending arrivals: a calendar of fixed-width time buckets; the bucket being
//! settled is kept sorted.
//!
//! **Why not one `BinaryHeap`.** A beat pushes a few arrivals per cell, almost all of them a
//! couple of milliseconds ahead of the front, so a global heap holds the whole band of pending
//! arrivals and every push and pop pays its full depth. Here a push into a future bucket is a
//! `Vec::push`, a bucket is sorted once when it becomes current, and a pop is a `Vec::pop`.
//!
//! **Same order as a global heap.** [`Front`]'s order is total (time, then cell, layer, probe,
//! origin, packed in one integer key), the current bucket holds every arrival whose bucket is
//! not later than the current one and bucket indices increase with time, so arrivals pop in
//! exactly the order of a single priority queue, whatever the bucket width: results do not
//! depend on it, only speed does.

use crate::engine::Origin;

/// Bucket width, ms: about a third of the shortest ventricular edge delay, so a settle's own
/// pushes land in a later bucket.
const BUCKET_MS: f64 = 0.25;
/// Buckets in the ring (horizon `RING · BUCKET_MS` ≈ 1 s); later arrivals wait in an overflow
/// list.
const RING: i64 = 4096;

/// A pending arrival: time plus the identity of the front, packed for a cheap total order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Front {
    /// Arrival time, ms.
    pub at: f64,
    /// `cell << 32 | layer << 24 | src_probe << 16 | origin rank`.
    tie: u64,
}

/// Rank of an origin (16 bits, order of the tie-break).
fn origin_rank(o: Origin) -> u16 {
    match o {
        Origin::Sinus => 0,
        Origin::Focus(id) => 0x100 | u16::from(id),
        Origin::AccessoryPathway => 0x200,
        Origin::Reentry => 0x300,
        Origin::Escape => 0x400,
        Origin::Pacer(ch) => 0x500 | u16::from(ch),
        Origin::External => 0x600,
    }
}

fn origin_of_rank(r: u16) -> Origin {
    let low = (r & 0xff) as u8;
    match r >> 8 {
        0 => Origin::Sinus,
        1 => Origin::Focus(low),
        2 => Origin::AccessoryPathway,
        3 => Origin::Reentry,
        4 => Origin::Escape,
        5 => Origin::Pacer(low),
        _ => Origin::External,
    }
}

impl Front {
    pub fn new(at: f64, cell: u32, layer: u8, src_probe: u8, origin: Origin) -> Self {
        Self {
            at,
            tie: u64::from(cell) << 32
                | u64::from(layer) << 24
                | u64::from(src_probe) << 16
                | u64::from(origin_rank(origin)),
        }
    }

    /// The same front arriving at `cell` at `at`.
    pub fn to(self, at: f64, cell: u32) -> Self {
        Self {
            at,
            tie: u64::from(cell) << 32 | (self.tie & 0xffff_ffff),
        }
    }

    pub fn cell(self) -> u32 {
        (self.tie >> 32) as u32
    }

    /// Fast endocardial layer this front may use (`0` = none).
    pub fn layer(self) -> u8 {
        (self.tie >> 24) as u8
    }

    /// Probe the front started in (it does not trigger that probe again).
    pub fn src_probe(self) -> u8 {
        (self.tie >> 16) as u8
    }

    pub fn origin(self) -> Origin {
        origin_of_rank(self.tie as u16)
    }

    /// Total order: earlier first, ties by lower cell index, then layer, probe and origin.
    fn key(self) -> u128 {
        // `f64::total_cmp` order as an unsigned integer
        let b = self.at.to_bits();
        let t = if b >> 63 == 1 { !b } else { b | 1 << 63 };
        u128::from(t) << 64 | u128::from(self.tie)
    }
}

/// Min-queue of [`Front`]s by arrival time (see the module documentation).
#[derive(Debug, Clone)]
pub(super) struct FrontQueue {
    /// Every queued front whose bucket is ≤ `cur`, sorted latest first.
    current: Vec<Front>,
    /// Bucket of `heap`.
    cur: i64,
    /// Fronts of buckets `cur + 1 ..= cur + RING`, at `bucket mod RING`.
    ring: Vec<Vec<Front>>,
    in_ring: usize,
    /// Fronts beyond the ring's horizon, unsorted.
    overflow: Vec<Front>,
    overflow_min: i64,
}

fn bucket(at: f64) -> i64 {
    // saturating float → int: ±∞ and huge times land in the first / last bucket
    (at * (1.0 / BUCKET_MS)).floor() as i64
}

fn slot(b: i64) -> usize {
    b.rem_euclid(RING) as usize
}

impl FrontQueue {
    pub fn new() -> Self {
        Self {
            current: Vec::with_capacity(1024),
            cur: i64::MIN,
            ring: vec![Vec::new(); RING as usize],
            in_ring: 0,
            overflow: Vec::new(),
            overflow_min: i64::MAX,
        }
    }

    pub fn clear(&mut self) {
        self.current.clear();
        self.cur = i64::MIN;
        if self.in_ring > 0 {
            self.ring.iter_mut().for_each(Vec::clear);
        }
        self.in_ring = 0;
        self.overflow.clear();
        self.overflow_min = i64::MAX;
    }

    pub fn push(&mut self, f: Front) {
        let b = bucket(f.at);
        if b <= self.cur {
            let key = f.key();
            let pos = self.current.partition_point(|x| x.key() > key);
            self.current.insert(pos, f);
        } else if b.saturating_sub(self.cur) <= RING {
            self.ring[slot(b)].push(f);
            self.in_ring += 1;
        } else {
            self.overflow_min = self.overflow_min.min(b);
            self.overflow.push(f);
        }
    }

    /// Pop the earliest front if it arrives at or before `until`.
    pub fn pop_until(&mut self, until: f64) -> Option<Front> {
        let last = bucket(until);
        loop {
            if let Some(top) = self.current.last() {
                return if top.at <= until { self.current.pop() } else { None };
            }
            if self.cur >= last {
                return None;
            }
            // The current bucket is empty: move on to the next bucket that holds fronts, never past
            // `until` (later pushes into skipped buckets would otherwise all be sorted inserts).
            let next = if self.in_ring > 0 {
                self.cur.saturating_add(1)
            } else if self.overflow.is_empty() {
                last
            } else {
                self.overflow_min.min(last)
            };
            self.set_cur(next);
        }
    }

    /// Make `b` (> `cur`) the current bucket; only called with an empty current bucket, and
    /// either `b = cur + 1` or an empty ring.
    fn set_cur(&mut self, b: i64) {
        self.cur = b;
        let s = slot(b);
        if !self.ring[s].is_empty() {
            self.in_ring -= self.ring[s].len();
            // swap buffers (keeping both allocations), then sort latest first
            std::mem::swap(&mut self.current, &mut self.ring[s]);
            self.current.sort_unstable_by_key(|f| std::cmp::Reverse(f.key()));
        }
        if self.overflow_min <= b.saturating_add(RING) {
            // (fronts landing in bucket `b` are sorted inserts)
            let pending = std::mem::take(&mut self.overflow);
            self.overflow_min = i64::MAX;
            for f in pending {
                self.push(f);
            }
        }
    }
}
