//! B41 acceptance: every shipped case packs into a case file, reads back equal and resolves against
//! the shipped rhythm library.

use heartsim_casefile::{CasePackage, Manifest};
use heartsim_core::case::Case;
use std::path::{Path, PathBuf};

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn ron_stems(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "ron"))
        .collect();
    v.sort();
    v
}

#[test]
fn every_shipped_case_packs_reads_back_equal_and_resolves() {
    let library: Vec<String> = ron_stems(&assets().join("scenarios"))
        .iter()
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    let cases = ron_stems(&assets().join("cases"));
    assert!(cases.len() >= 9, "{} cases", cases.len());
    for path in cases {
        let case = Case::from_ron(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let manifest = Manifest::for_case(
            &case,
            vec!["heartsim authors".into()],
            "MIT OR Apache-2.0",
            "2026-11-27",
            "heartsim test",
        );
        let p = CasePackage::new(manifest, case, Vec::new());
        let bytes = p.write().unwrap();
        let back = CasePackage::read_for(&bytes, |id| library.iter().any(|l| l == id))
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(back, p, "{}", path.display());
        assert_eq!(back.write().unwrap(), bytes);
    }
}
