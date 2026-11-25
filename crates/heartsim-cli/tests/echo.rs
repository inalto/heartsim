//! Brief B33: `heartsim-cli echo views` (output snapshot) and the echo-views tutorial (it validates,
//! and each "view found" check passes at the view's reference pose and fails at the starting pose).
//!
//! Regenerate the snapshot with `UPDATE_SNAPSHOTS=1 cargo test -p heartsim-cli --test echo`.

use heartsim_core::tutorial::{Step, Tutorial, UiAction};
use heartsim_echo::{
    DesignedParams, EchoAnatomy, EchoWindow, MotionModel, MotionParams, ViewId, ViewTargets,
};
use heartsim_geometry::AnatomyPack;
use std::path::{Path, PathBuf};
use std::process::Command;

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

#[test]
fn echo_views_output_matches_the_snapshot() {
    let out = Command::new(env!("CARGO_BIN_EXE_heartsim-cli"))
        .args(["echo", "views"])
        .output()
        .expect("runs heartsim-cli");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/echo_views.txt");
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&path, &text).expect("write the snapshot");
    }
    let want = std::fs::read_to_string(&path)
        .expect("tests/snapshots/echo_views.txt (UPDATE_SNAPSHOTS=1 writes it)");
    assert_eq!(
        text, want,
        "echo views changed; review it, then UPDATE_SNAPSHOTS=1"
    );
    assert_eq!(text.lines().count(), 1 + ViewId::ALL.len());
}

#[test]
fn the_echo_views_tutorial_validates_and_its_view_checks_pass_at_the_reference_and_fail_at_the_start() {
    let text =
        std::fs::read_to_string(assets().join("tutorials/13_echo_views/tutorial.ron")).expect("tutorial");
    let t = Tutorial::from_ron(&text).expect("parses");
    assert_eq!(t.content_problems(), Vec::<String>::new());
    let pack = AnatomyPack::load(&assets().join("anatomy/default")).expect("pack");
    let model = MotionModel::new(&pack, MotionParams::default()).expect("model");
    let anatomy = EchoAnatomy::new(&pack, &model, DesignedParams::default()).expect("anatomy");
    let views = ViewTargets::new(&pack, &model, &anatomy).expect("views");
    let found = views.tolerances.found_threshold;
    let mut checked = 0;
    for (n, page) in t.pages.iter().enumerate() {
        let Some(vc) = &page.view_check else { continue };
        let id =
            ViewId::from_id(&vc.view).unwrap_or_else(|| panic!("page {}: unknown view {}", n + 1, vc.view));
        let r = views.reference(&anatomy, id);
        assert!(
            r.reachable,
            "page {}: {id:?} is not reachable, a tutorial cannot ask for it",
            n + 1
        );
        assert!(
            r.score.total >= found,
            "page {}: reference {}",
            n + 1,
            r.score.total
        );
        // the window the page starts the probe from
        let window = page
            .actions
            .iter()
            .find_map(|s| match s {
                Step::Ui(UiAction::ProbeAtWindow(w)) => Some(match w {
                    heartsim_core::tutorial::ProbeWindow::Parasternal => EchoWindow::Parasternal,
                    heartsim_core::tutorial::ProbeWindow::Apical => EchoWindow::Apical,
                    heartsim_core::tutorial::ProbeWindow::Subcostal => EchoWindow::Subcostal,
                    heartsim_core::tutorial::ProbeWindow::Suprasternal => EchoWindow::Suprasternal,
                }),
                _ => None,
            })
            .expect("the page starts the probe at a window");
        let start = anatomy.window_pose(window);
        let s = views.score(&anatomy, id, &start).total;
        assert!(s < found, "page {}: the starting pose already scores {s}", n + 1);
        checked += 1;
    }
    assert!(checked >= 8, "{checked} view checks");
}

/// The tutorial teaches the views in the order of `ViewId::ALL`, which is the guideline's (Mitchell
/// et al. 2019: the RV outflow view before the RV inflow view, Table 2 rows 2.5 and 2.6).
#[test]
fn the_echo_views_tutorial_follows_the_guideline_order_of_view_id_all() {
    let text =
        std::fs::read_to_string(assets().join("tutorials/13_echo_views/tutorial.ron")).expect("tutorial");
    let t = Tutorial::from_ron(&text).expect("parses");
    let order: Vec<usize> = t
        .pages
        .iter()
        .filter_map(|p| p.view_check.as_ref())
        .map(|vc| {
            let id = ViewId::from_id(&vc.view).expect("known view");
            ViewId::ALL.iter().position(|v| *v == id).expect("in ALL")
        })
        .collect();
    assert!(order.len() >= 8);
    assert!(
        order.windows(2).all(|w| w[0] < w[1]),
        "the tutorial's views are not in ViewId::ALL order: {order:?}"
    );
    let outflow = ViewId::ALL.iter().position(|v| *v == ViewId::RvOutflow);
    let inflow = ViewId::ALL.iter().position(|v| *v == ViewId::RvInflow);
    assert!(outflow < inflow, "RV outflow before RV inflow");
}
