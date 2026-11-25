//! `heartsim-cli echo views` (brief B33): each standard echo view with its status in this anatomy,
//! its reference probe pose and what that pose scores. The numbers are rounded so that the text is
//! a regression snapshot, not a measurement.

use crate::content::assets_root;
use anyhow::{Context, Result, bail};
use heartsim_echo::{DesignedParams, EchoAnatomy, MotionModel, MotionParams, Reach, ViewId, ViewTargets};
use heartsim_geometry::AnatomyPack;
use std::fmt::Write as _;
use std::path::Path;

/// Build the anatomy and the view targets of the default pack under `assets`.
pub(crate) fn build(assets: &Path) -> Result<(EchoAnatomy, ViewTargets)> {
    let dir = assets.join("anatomy/default");
    let pack = AnatomyPack::load(&dir).with_context(|| format!("loading {}", dir.display()))?;
    let model = MotionModel::new(&pack, MotionParams::default()).context("the motion model")?;
    let anatomy = EchoAnatomy::new(&pack, &model, DesignedParams::default()).context("the echo anatomy")?;
    let targets = ViewTargets::new(&pack, &model, &anatomy).context("the view targets")?;
    Ok((anatomy, targets))
}

/// The text of `echo views`.
pub(crate) fn run(what: &str, assets_arg: Option<&Path>) -> Result<String> {
    if what != "views" {
        bail!("unknown echo report `{what}` (try `views`)");
    }
    let assets = assets_root(assets_arg)?;
    let (anatomy, targets) = build(&assets)?;
    let mut out = String::new();
    writeln!(
        out,
        "{:<15} {:<12} {:<15} {:>5} {:>6}  {:>16}  {:>5} {:>5} {:>5} {:>5}  {:<9} ASE pages",
        "view",
        "window",
        "status",
        "score",
        "hidden",
        "skin x,y,z (mm)",
        "tilt",
        "rock",
        "rot",
        "depth",
        "clock"
    )?;
    for id in ViewId::ALL {
        let t = targets.target(id);
        let r = targets.reference(&anatomy, id);
        let status = match (t.reach, r.reachable) {
            (Reach::NotReachable(why), _) => format!("no ({why})"),
            (Reach::Reachable, true) => "shipped".to_string(),
            (Reach::Reachable, false) => "no (best pose)".to_string(),
        };
        let p = r.pose;
        let c = id.citation();
        // a view the anatomy lacks has no meaningful score
        let score = if matches!(t.reach, Reach::Reachable) {
            format!("{:.2}", r.score.total)
        } else {
            "n/a".to_string()
        };
        writeln!(
            out,
            "{:<15} {:<12} {:<15} {:>5} {:>6.1}  {:>5.0},{:>4.0},{:>4.0}  {:>5.0} {:>5.0} {:>5.0} {:>5.0}  {:<9.1} {}{}",
            id.id(),
            format!("{:?}", id.window()).to_lowercase(),
            status,
            score,
            r.hidden_fraction,
            p.skin_mm.x,
            p.skin_mm.y,
            p.skin_mm.z,
            p.tilt_deg,
            p.rock_deg,
            p.rotation_deg,
            p.depth_mm,
            p.clock_hours(),
            c.pages,
            if c.verified { "" } else { " (page not verified)" },
        )?;
    }
    Ok(out)
}
