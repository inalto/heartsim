//! B41 acceptance: arbitrary bytes and mutated valid files never make the reader panic, and it never
//! allocates beyond its limits. A counting allocator records the largest single allocation of the
//! test process (thread-independent, so the tests may run in parallel): a reader that trusted a
//! size from a header would show up as one huge allocation.

mod common;

use heartsim_casefile::{CasePackage, MAX_ENTRY_BYTES, MAX_FILE_BYTES};
use proptest::prelude::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static LARGEST: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards to the system allocator; only records the requested size.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LARGEST.fetch_max(layout.size(), Ordering::Relaxed);
        // SAFETY: same contract as the caller's
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: same contract as the caller's
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LARGEST.fetch_max(new_size, Ordering::Relaxed);
        // SAFETY: same contract as the caller's
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// The most any one allocation of the reader may need: an entry read to one byte past its limit
/// grows its buffer to at most twice that, plus room for the input the test itself holds.
const ALLOCATION_BOUND: usize = 2 * (MAX_ENTRY_BYTES as usize + 1) + MAX_FILE_BYTES;

fn check(bytes: &[u8]) {
    if let Ok(p) = CasePackage::read(bytes) {
        let _ = p.resolve(common::builtin);
    }
    let largest = LARGEST.load(Ordering::Relaxed);
    assert!(largest <= ALLOCATION_BOUND, "an allocation of {largest} bytes");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..4096)) {
        check(&bytes);
    }

    #[test]
    fn mutated_valid_files_never_panic(
        flips in proptest::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..12),
        cut in proptest::option::of(any::<prop::sample::Index>()),
    ) {
        let mut bytes = common::package().write().unwrap();
        for (at, value) in flips {
            let i = at.index(bytes.len());
            bytes[i] = value;
        }
        if let Some(c) = cut {
            let n = c.index(bytes.len());
            bytes.truncate(n);
        }
        check(&bytes);
    }

    #[test]
    fn mutated_headers_never_panic(
        flips in proptest::collection::vec((0usize..46, any::<u8>()), 1..6),
        central in any::<bool>(),
    ) {
        // the header fields (sizes, methods, flags, name lengths) are where a crafted file lies
        let mut bytes = common::package().write().unwrap();
        let (local, central_at) = common::headers(&bytes);
        let base = if central { central_at[0] } else { local[0] };
        for (off, value) in flips {
            if base + off < bytes.len() {
                bytes[base + off] = value;
            }
        }
        check(&bytes);
    }
}

#[test]
fn a_large_bomb_stays_within_the_allocation_bound() {
    // 64 MiB of zeros in a file of well under 2 MiB, headers claiming 10 bytes; written in chunks
    // so the test itself never holds a large buffer
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    w.start_file(heartsim_casefile::MANIFEST_NAME, opts).unwrap();
    let chunk = vec![0u8; 64 * 1024];
    for _ in 0..1024 {
        std::io::Write::write_all(&mut w, &chunk).unwrap();
    }
    let mut bytes = w.finish().unwrap().into_inner();
    assert!(bytes.len() < MAX_FILE_BYTES);
    common::patch_uncompressed_size(&mut bytes, 10);
    check(&bytes);
    assert!(CasePackage::read(&bytes).is_err());
}
