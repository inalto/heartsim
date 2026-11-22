//! Brief B29 acceptance for `heartsim-cli calc`: `calc list` lists the registry, `calc <id>` prints
//! the library's outcome (text and JSON), and bad input exits non-zero with the field's message.

use heartsim_calc::{ALL, Inputs, by_id};
use std::process::{Command, Output};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_heartsim-cli"))
        .args(args)
        .output()
        .expect("runs heartsim-cli")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn list_prints_every_calculator() {
    let o = cli(&["calc", "list"]);
    assert!(o.status.success());
    let text = stdout(&o);
    assert_eq!(text.lines().count(), ALL.len());
    for c in ALL {
        assert!(text.contains(c.id), "{}", c.id);
    }
    let o = cli(&["calc", "list", "--json", "--lang", "it"]);
    let rows: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), ALL.len());
    assert_eq!(
        rows[0]["name"],
        "Area valvolare aortica (equazione di continuità)"
    );
}

#[test]
fn one_calculator_equals_the_library_outcome() {
    let o = cli(&[
        "calc",
        "aortic-valve-area",
        "lvot_d_cm=2.0",
        "lvot_vti_cm=20",
        "av_vti_cm=80",
        "bsa_m2=1.8",
        "--json",
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let got: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    let c = by_id("aortic-valve-area").unwrap();
    let want = c
        .evaluate(
            &Inputs::new()
                .with("lvot_d_cm", 2.0)
                .with("lvot_vti_cm", 20.0)
                .with("av_vti_cm", 80.0)
                .with("bsa_m2", 1.8),
        )
        .unwrap();
    let values = got["values"].as_array().unwrap();
    assert_eq!(values.len(), want.values.len());
    for (g, w) in values.iter().zip(&want.values) {
        assert_eq!(g["id"], w.id);
        assert!((g["value"].as_f64().unwrap() - w.value).abs() <= 1e-12 * w.value.abs().max(1.0));
        assert_eq!(g["unit"], w.unit.symbol());
    }
    assert_eq!(got["findings"].as_array().unwrap().len(), want.findings.len());
    assert_eq!(got["findings"][0]["criterion"], want.findings[0].criterion);
    assert_eq!(got["review"], "verified");

    // the text form carries the substituted formula, the finding with its locator and the note
    let o = cli(&[
        "calc",
        "aortic-valve-area",
        "lvot_d_cm=2.0",
        "lvot_vti_cm=20",
        "av_vti_cm=80",
    ]);
    let text = stdout(&o);
    assert!(text.contains("AVA = CSA × VTI(LVOT) ÷ VTI(AV)"), "{text}");
    assert!(text.contains("= 0.79 cm²"), "{text}");
    assert!(text.contains("Table 3, p. 383"), "{text}");
    assert!(text.contains("not a medical device"), "{text}");
}

#[test]
fn every_worked_example_runs_from_the_command_line() {
    for c in ALL {
        let o = cli(&["calc", c.id, "--example", "--json"]);
        assert!(
            o.status.success(),
            "{}: {}",
            c.id,
            String::from_utf8_lossy(&o.stderr)
        );
        let got: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        let want = c.evaluate(&c.example_inputs()).unwrap();
        // JSON keeps 15–17 significant digits: equal to the last few ulps
        let (g, w) = (got["values"][0]["value"].as_f64().unwrap(), want.values[0].value);
        assert!((g - w).abs() <= 1e-12 * w.abs().max(1.0), "{}: {g} vs {w}", c.id);
    }
}

#[test]
fn bad_input_exits_non_zero_with_the_message() {
    let o = cli(&[
        "calc",
        "aortic-valve-area",
        "lvot_d_cm=9",
        "lvot_vti_cm=20",
        "av_vti_cm=80",
    ]);
    assert!(!o.status.success());
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(
        err.contains("lvot_d_cm: Outside the plausible range 1–4"),
        "{err}"
    );
    let o = cli(&["calc", "aortic-valve-area", "lvot_d_cm=2"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("lvot_vti_cm: Required"));
    let o = cli(&[
        "calc",
        "ejection-fraction",
        "edv_ml=100",
        "esv_ml=120",
        "--lang",
        "it",
    ]);
    assert!(!o.status.success());
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("esv_ml: Il volume telesistolico deve essere minore")
    );
    let o = cli(&["calc", "ejection-fraction", "edv_ml=abc"]);
    assert!(String::from_utf8_lossy(&o.stderr).contains("edv_ml: Not a number"));
    let o = cli(&["calc", "no-such-thing"]);
    assert!(!o.status.success());
    let o = cli(&["calc", "ejection-fraction", "nonsense=3"]);
    assert!(String::from_utf8_lossy(&o.stderr).contains("unknown input `nonsense`"));
}

#[test]
fn a_decision_tool_prints_its_path_and_result() {
    // Figure 4: E/A 110/90 = 1.22 (0.8-1.8), IVRT 68 < 80 ms → elevated LAP
    let args = ["calc", "diastolic-mac", "e_cm_s=110", "a_cm_s=90", "ivrt_ms=68"];
    let o = cli(&[&args[..], &["--json"]].concat());
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let got: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    let want = by_id("diastolic-mac")
        .unwrap()
        .evaluate(
            &Inputs::new()
                .with("e_cm_s", 110.0)
                .with("a_cm_s", 90.0)
                .with("ivrt_ms", 68.0),
        )
        .unwrap();
    let findings = got["findings"].as_array().unwrap();
    assert_eq!(findings.len(), want.findings.len());
    for (g, w) in findings.iter().zip(&want.findings) {
        assert_eq!(g["text"], w.text.en);
        assert_eq!(g["criterion"], w.criterion);
        assert_eq!(g["locator"], w.source.locator);
    }
    let last = findings.last().unwrap();
    assert_eq!(last["severity"], "abnormal");
    assert!(
        last["text"]
            .as_str()
            .unwrap()
            .starts_with("Result: meets the figure's criteria for elevated LAP")
    );
    // an indeterminate result in text form
    let o = cli(&["calc", "diastolic-mac", "e_cm_s=100", "a_cm_s=100"]);
    assert!(o.status.success());
    let text = stdout(&o);
    assert!(
        text.contains("[indeterminate] LAP indeterminate with these measurements"),
        "{text}"
    );
    assert!(text.contains("Figure 4, p. 558"), "{text}");
}
