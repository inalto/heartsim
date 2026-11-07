//! B20 acceptance tests of the export formats on a real scenario run: the PDF parses and carries
//! 12 lead traces plus the rhythm strip, every format is deterministic, and what is exported
//! imports back.

use heartsim_core::Lead;
use heartsim_ecg::Tier0Scenario;
use heartsim_io::{
    EXPORT_RATE_HZ, EcgRecord, ExportInfo, ExportOptions, Paper, export_files, import_bytes, import_path,
};
use heartsim_ui::LeadOrder;
use std::path::Path;

fn scenario(id: &str) -> Tier0Scenario {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../assets/scenarios/{id}.ron"));
    Tier0Scenario::from_ron(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn info(s: &Tier0Scenario, paper: Paper) -> ExportInfo {
    ExportInfo {
        scenario_id: s.scenario.id.clone(),
        scenario_title: s.scenario.title.clone(),
        mm_per_s: 25.0,
        mm_per_mv: 10.0,
        date: "2026-11-03".into(),
        paper,
        trace_note: None,
    }
}

fn excerpt(s: &Tier0Scenario, run_s: f64, window_s: f64) -> EcgRecord {
    EcgRecord::from_recording(&s.record(run_s).unwrap(), window_s, EXPORT_RATE_HZ)
}

/// Stroked paths with more than `min_points` vertices on each page, from the decoded content stream.
fn long_paths(pdf: &[u8], min_points: usize) -> (usize, Vec<usize>) {
    let doc = lopdf::Document::load_mem(pdf).expect("PDF parses");
    let pages = doc.get_pages();
    let mut counts = Vec::new();
    for id in pages.values() {
        let content = lopdf::content::Content::decode(&doc.get_page_content(*id)).unwrap();
        let (mut points, mut long) = (0usize, 0usize);
        for op in &content.operations {
            match op.operator.as_str() {
                "m" => points = 1,
                "l" => points += 1,
                "S" | "s" | "B" | "b" | "f" | "n" => {
                    if points > min_points {
                        long += 1;
                    }
                    points = 0;
                }
                _ => {}
            }
        }
        counts.push(long);
    }
    (pages.len(), counts)
}

#[test]
fn pdf_has_twelve_lead_traces_and_a_rhythm_strip() {
    let s = scenario("sinus_normal");
    let rec = excerpt(&s, 12.0, 10.0);
    for paper in [Paper::A4, Paper::Letter] {
        let pdf = heartsim_io::pdf::write_pdf(&rec, &info(&s, paper), LeadOrder::Standard);
        assert!(pdf.starts_with(b"%PDF-"));
        let (pages, traces) = long_paths(&pdf, 100);
        assert_eq!(pages, 1);
        // 2.5 s per lead cell = 1250 samples; the 10 s rhythm strip is the 13th
        assert_eq!(traces, [13], "{paper:?}");
        let doc = lopdf::Document::load_mem(&pdf).unwrap();
        let page = *doc.get_pages().values().next().unwrap();
        let mediabox = doc
            .get_dictionary(page)
            .unwrap()
            .get(b"MediaBox")
            .unwrap()
            .as_array()
            .unwrap()
            .clone();
        let w_pt = mediabox[2].as_float().unwrap();
        let want_pt = paper.size_mm().0 / 25.4 * 72.0;
        assert!((w_pt - want_pt).abs() < 0.5, "{paper:?}: width {w_pt} pt");
    }
}

#[test]
fn pdf_header_names_the_scenario_but_no_patient() {
    let s = scenario("lbbb");
    let rec = excerpt(&s, 4.0, 4.0);
    let pdf = heartsim_io::pdf::write_pdf(&rec, &info(&s, Paper::A4), LeadOrder::Cabrera);
    let doc = lopdf::Document::load_mem(&pdf).unwrap();
    let page = *doc.get_pages().values().next().unwrap();
    let text = String::from_utf8_lossy(&doc.get_page_content(page)).into_owned();
    assert!(text.contains("Scenario: lbbb"), "{text}");
    assert!(text.contains("25 mm/s") && text.contains("10 mm/mV") && text.contains("2026-11-03"));
    assert!(text.contains("-aVR"), "Cabrera inverts aVR");
    for field in ["Patient", "Name:", "DOB", "ID:"] {
        assert!(!text.contains(field), "{field}");
    }
    // a 4 s excerpt fills 1.6 of the 4 columns: only the first two columns have samples
    let (_, traces) = long_paths(&pdf, 100);
    assert_eq!(traces, [7]);
}

#[test]
fn every_export_is_deterministic() {
    let s = scenario("avb_2nd_mobitz1");
    let rec = excerpt(&s, 11.0, 10.0);
    let a = export_files(&rec, &info(&s, Paper::A4), &s, &ExportOptions::default()).unwrap();
    let b = export_files(&rec, &info(&s, Paper::A4), &s, &ExportOptions::default()).unwrap();
    assert_eq!(a, b);
    let names: Vec<&str> = a.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "heartsim_avb_2nd_mobitz1.pdf",
            "heartsim_avb_2nd_mobitz1-12lead.png",
            "heartsim_avb_2nd_mobitz1-II.png",
            "heartsim_avb_2nd_mobitz1.csv",
            "heartsim_avb_2nd_mobitz1.hea",
            "heartsim_avb_2nd_mobitz1.dat",
            "heartsim_avb_2nd_mobitz1.ron",
            "heartsim_avb_2nd_mobitz1.zip",
        ]
    );
    // PNG sizes: 250 mm × 160 mm page and 250 mm × 40 mm strip at 5 px/mm
    let size = |bytes: &[u8]| {
        let img = image::load_from_memory(bytes).unwrap();
        (img.width(), img.height())
    };
    assert_eq!(size(&a[1].bytes), (1250, 800));
    assert_eq!(size(&a[2].bytes), (1250, 200));
    // 10 s at 500 Hz, 12 leads, 2 bytes per sample
    assert_eq!(a[5].bytes.len(), 5000 * 12 * 2);
}

#[test]
fn bundle_holds_every_file_and_a_readme_and_imports_back() {
    let s = scenario("wpw_left_lateral");
    let rec = excerpt(&s, 12.0, 10.0);
    let files = export_files(&rec, &info(&s, Paper::Letter), &s, &ExportOptions::default()).unwrap();
    let zip_bytes = &files.last().unwrap().bytes;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes.as_slice())).unwrap();
    let mut names: Vec<String> = zip.file_names().map(|n| n.unwrap().into_owned()).collect();
    names.sort();
    assert_eq!(names.len(), 1 + files.len() - 1);
    assert!(names.contains(&"README.txt".to_string()));
    let mut readme = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("README.txt").unwrap(), &mut readme).unwrap();
    assert!(
        readme.contains("wpw_left_lateral") && readme.contains("not a patient recording"),
        "{readme}"
    );
    for f in &files[..files.len() - 1] {
        let mut inner = Vec::new();
        std::io::Read::read_to_end(&mut zip.by_name(&f.name).unwrap(), &mut inner).unwrap();
        assert_eq!(inner, f.bytes, "{}", f.name);
    }

    let imp = import_bytes("bundle.zip", zip_bytes, |_| unreachable!()).unwrap();
    assert!(imp.label.contains("wpw_left_lateral"), "{}", imp.label);
    assert_eq!(imp.record.len(), rec.len());
    assert!((imp.record.start_ms - rec.start_ms).abs() < 1e-9);
    for l in Lead::ALL {
        let (a, b) = (rec.lead(l), imp.record.lead(l));
        assert!(
            a.iter().zip(b).all(|(x, y)| (x - y).abs() <= 0.000_5 + 1e-6),
            "{l:?}"
        );
    }
}

#[test]
fn exported_files_import_from_disk() {
    let s = scenario("sinus_normal");
    let rec = excerpt(&s, 6.0, 5.0);
    let files = export_files(&rec, &info(&s, Paper::A4), &s, &ExportOptions::default()).unwrap();
    let dir = std::env::temp_dir().join(format!("heartsim-io-import-{}", std::process::id()));
    heartsim_io::write_files(&dir, &files).unwrap();
    for name in [
        "heartsim_sinus_normal.hea",
        "heartsim_sinus_normal.dat",
        "heartsim_sinus_normal.csv",
    ] {
        let imp = import_path(&dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(imp.record.len(), 2500, "{name}");
        assert!((imp.record.sample_rate_hz - 500.0).abs() < 1e-9);
        let (a, b) = (rec.lead(Lead::V2), imp.record.lead(Lead::V2));
        assert!(
            a.iter().zip(b).all(|(x, y)| (x - y).abs() <= 0.000_5 + 1e-6),
            "{name}"
        );
    }
    assert!(import_path(&dir.join("heartsim_sinus_normal.pdf")).is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn scenario_export_reloads_and_reruns_identically() {
    let s = scenario("avnrt_150");
    let text = heartsim_io::scenario_file::scenario_ron(&s).unwrap();
    let back = Tier0Scenario::from_ron(&text).unwrap();
    assert_eq!(back.record(5.0).unwrap().leads, s.record(5.0).unwrap().leads);
}

#[test]
fn a_processed_trace_is_stated_in_every_file_that_carries_it() {
    let s = scenario("sinus_normal");
    let rec = excerpt(&s, 8.0, 6.0);
    let note = "Display filter: monitor 0.5-40 Hz. Artefacts added: 50 Hz mains (intensity 1.0, seed 7).";
    let clean = export_files(&rec, &info(&s, Paper::A4), &s, &ExportOptions::default()).unwrap();
    let mut noted_info = info(&s, Paper::A4);
    noted_info.trace_note = Some(note.into());
    let noted = export_files(&rec, &noted_info, &s, &ExportOptions::default()).unwrap();
    let find = |files: &[heartsim_io::ExportFile], ext: &str| -> Vec<u8> {
        files
            .iter()
            .find(|f| f.name.ends_with(ext))
            .unwrap_or_else(|| panic!("no {ext} file"))
            .bytes
            .clone()
    };

    // CSV: a comment line first, and it still imports to the same samples
    let csv = String::from_utf8(find(&noted, ".csv")).unwrap();
    assert!(
        csv.starts_with(&format!("# trace: {note}\nt_ms,")),
        "{}",
        &csv[..120]
    );
    assert!(!String::from_utf8(find(&clean, ".csv")).unwrap().contains("trace"));
    let back = import_bytes("x.csv", csv.as_bytes(), |_| unreachable!()).unwrap();
    assert_eq!(back.record.len(), rec.len());

    // WFDB header comment
    let hea = String::from_utf8(find(&noted, ".hea")).unwrap();
    assert!(hea.contains(&format!("# trace: {note}")), "{hea}");
    assert!(
        !String::from_utf8(find(&clean, ".hea"))
            .unwrap()
            .contains("trace:")
    );

    // PDF: the page text says it, the clean PDF does not
    let page_text = |pdf: &[u8]| -> String {
        let doc = lopdf::Document::load_mem(pdf).expect("PDF parses");
        let mut out = String::new();
        for id in doc.get_pages().values() {
            let content = lopdf::content::Content::decode(&doc.get_page_content(*id)).unwrap();
            for op in &content.operations {
                for operand in &op.operands {
                    let mut strings = Vec::new();
                    match operand {
                        lopdf::Object::String(b, _) => strings.push(b.clone()),
                        lopdf::Object::Array(items) => {
                            for item in items {
                                if let lopdf::Object::String(b, _) = item {
                                    strings.push(b.clone());
                                }
                            }
                        }
                        _ => {}
                    }
                    for b in strings {
                        out.push_str(&String::from_utf8_lossy(&b));
                    }
                }
            }
        }
        out
    };
    assert!(page_text(&find(&noted, ".pdf")).contains("Trace not as simulated"));
    assert!(!page_text(&find(&clean, ".pdf")).contains("Trace not as simulated"));

    // PNGs carry the caption (so they differ from the clean pictures); the scenario file does not change
    assert_ne!(find(&noted, "-12lead.png"), find(&clean, "-12lead.png"));
    assert_ne!(find(&noted, "-II.png"), find(&clean, "-II.png"));
    assert_eq!(find(&noted, ".ron"), find(&clean, ".ron"));

    // the bundle's README states it
    let zip_bytes = find(&noted, ".zip");
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes.as_slice())).unwrap();
    let mut readme = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("README.txt").unwrap(), &mut readme).unwrap();
    assert!(
        readme.contains("NOT the clean engine signal") && readme.contains(note),
        "{readme}"
    );
}

#[test]
fn a_clean_trace_exports_exactly_as_before() {
    // no note: no extra line anywhere, so existing exports and the CLI stay byte-identical
    let s = scenario("sinus_normal");
    let rec = excerpt(&s, 8.0, 6.0);
    let files = export_files(&rec, &info(&s, Paper::A4), &s, &ExportOptions::default()).unwrap();
    let csv = String::from_utf8(
        files
            .iter()
            .find(|f| {
                std::path::Path::new(&f.name)
                    .extension()
                    .is_some_and(|e| e == "csv")
            })
            .unwrap()
            .bytes
            .clone(),
    )
    .unwrap();
    assert!(csv.starts_with("t_ms,"));
}
