//! Ischaemia panel (brief B14): a schematic coronary tree to pick a branch, severity and lesion
//! kind, and the teaching presets (anterior / inferior / lateral / posterior STEMI,
//! subendocardial ischaemia, pericarditis, old infarcts).
//!
//! Like the other panels it only produces `Action::Stenosis`s; the lesions shown come back from
//! the engine. The branch under the cursor (or the selected one) is reported so the app can
//! highlight its territory on the 3-D heart ([`territory_glyphs`]).

use crate::t;
use egui::{Color32, Pos2, Sense, Stroke, Ui, vec2};
use glam::Vec3;
use heartsim_core::Action;
use heartsim_render::GlyphInstance;
use heartsim_sim::ischaemia::{CoronaryBranch, IschaemiaPreset, Lesion, LesionKind};

/// Size of the coronary-tree drawing, points.
const TREE_SIZE: egui::Vec2 = vec2(230.0, 190.0);
/// A click within this distance of a branch selects it, points.
const PICK_RADIUS: f32 = 7.0;

/// Panel state owned by the app.
#[derive(Debug, Clone, PartialEq)]
pub struct IschaemiaPanelState {
    pub branch: CoronaryBranch,
    /// Severity to apply, 0 … 1.
    pub severity: f64,
    /// Apply as a healed infarct instead of acute ischaemia.
    pub infarct: bool,
}

impl Default for IschaemiaPanelState {
    fn default() -> Self {
        Self {
            branch: CoronaryBranch::Lad,
            severity: 0.9,
            infarct: false,
        }
    }
}

/// What the panel did this frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IschaemiaResponse {
    /// Actions to apply now.
    pub actions: Vec<Action>,
    /// Branch to highlight on the heart (hovered, else selected).
    pub highlight: Option<CoronaryBranch>,
}

/// Schematic course of each branch in the tree drawing, unit square (0,0 top-left), anterior
/// view: aortic root at the top centre, LAD down the front, LCx round to the left, RCA to the
/// right and down to the PDA.
fn branch_path(b: CoronaryBranch) -> &'static [(f32, f32)] {
    match b {
        CoronaryBranch::LeftMain => &[(0.5, 0.08), (0.62, 0.2)],
        CoronaryBranch::Lad => &[(0.62, 0.2), (0.6, 0.45), (0.55, 0.7), (0.5, 0.92)],
        CoronaryBranch::Lcx => &[(0.62, 0.2), (0.8, 0.28), (0.9, 0.42)],
        CoronaryBranch::ObtuseMarginal => &[(0.8, 0.28), (0.85, 0.5), (0.82, 0.68)],
        CoronaryBranch::Posterolateral => &[(0.9, 0.42), (0.94, 0.62), (0.9, 0.8)],
        CoronaryBranch::Rca => &[(0.42, 0.1), (0.25, 0.22), (0.12, 0.45), (0.2, 0.72)],
        CoronaryBranch::Pda => &[(0.2, 0.72), (0.35, 0.84), (0.48, 0.9)],
        CoronaryBranch::Global | CoronaryBranch::Pericardium => &[],
    }
}

/// Lesion of `b` (acute first, then infarct) among `lesions`.
fn lesion_of(lesions: &[Lesion], b: CoronaryBranch) -> Option<&Lesion> {
    lesions
        .iter()
        .filter(|l| l.branch == b)
        .min_by_key(|l| l.kind.is_necrotic())
}

fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    (a + t * ab - p).length()
}

/// The coronary tree: returns `(clicked, hovered)` branches.
pub fn coronary_tree(
    ui: &mut Ui,
    selected: CoronaryBranch,
    lesions: &[Lesion],
) -> (Option<CoronaryBranch>, Option<CoronaryBranch>) {
    let (rect, response) = ui.allocate_exact_size(TREE_SIZE, Sense::click());
    let painter = ui.painter_at(rect);
    let to_screen = |(x, y): (f32, f32)| rect.min + vec2(x * rect.width(), y * rect.height());
    // heart silhouette and aortic root
    painter.circle_stroke(
        to_screen((0.55, 0.55)),
        0.42 * rect.height(),
        Stroke::new(1.0, ui.visuals().weak_text_color()),
    );
    painter.circle_filled(to_screen((0.5, 0.08)), 6.0, Color32::from_rgb(200, 60, 60));
    let pointer = response.hover_pos();
    let mut hovered = None;
    let mut best = PICK_RADIUS;
    for b in CoronaryBranch::ALL {
        let path = branch_path(b);
        for w in path.windows(2) {
            if let Some(p) = pointer {
                let d = distance_to_segment(p, to_screen(w[0]), to_screen(w[1]));
                if d < best {
                    best = d;
                    hovered = Some(b);
                }
            }
        }
    }
    for b in CoronaryBranch::ALL {
        let path = branch_path(b);
        if path.is_empty() {
            continue;
        }
        let colour = match lesion_of(lesions, b) {
            Some(l) if l.kind.is_necrotic() => Color32::from_gray(110),
            Some(l) => Color32::from_rgb(220, (200.0 * (1.0 - l.severity)) as u8 + 30, 40),
            None => Color32::from_rgb(220, 120, 110),
        };
        let width = if b == selected || Some(b) == hovered {
            5.0
        } else {
            3.0
        };
        let points: Vec<Pos2> = path.iter().map(|&p| to_screen(p)).collect();
        painter.add(egui::Shape::line(points.clone(), Stroke::new(width, colour)));
        if let Some(l) = lesion_of(lesions, b) {
            // the lesion sits at the start of the branch
            let at = points[0] + 0.35 * (points[1] - points[0]);
            painter.circle_stroke(
                at,
                4.0 + 3.0 * l.severity as f32,
                Stroke::new(2.0, Color32::BLACK),
            );
        }
        let label_at = points[points.len() / 2] + vec2(6.0, -6.0);
        painter.text(
            label_at,
            egui::Align2::LEFT_BOTTOM,
            b.id(),
            egui::FontId::proportional(11.0),
            ui.visuals().text_color(),
        );
    }
    let response = match hovered {
        Some(b) => response.on_hover_text(crate::labels::branch_name(b)),
        None => response,
    };
    let clicked = if response.clicked() { hovered } else { None };
    (clicked, hovered)
}

/// Target string of a `Stenosis` action.
#[must_use]
pub fn stenosis_target(branch: CoronaryBranch, kind: LesionKind) -> String {
    match kind {
        LesionKind::Acute => branch.id().to_string(),
        LesionKind::Infarct => format!("{} infarct", branch.id()),
        LesionKind::Evolving => format!("{} evolving", branch.id()),
    }
}

/// The ischaemia panel. `lesions` are the lesions in effect (from the engine).
pub fn ischaemia_panel(
    ui: &mut Ui,
    state: &mut IschaemiaPanelState,
    lesions: &[Lesion],
) -> IschaemiaResponse {
    let mut r = IschaemiaResponse::default();
    ui.strong(t!("ischaemia-coronary-tree"));
    let (clicked, hovered) = coronary_tree(ui, state.branch, lesions);
    if let Some(b) = clicked {
        state.branch = b;
    }
    ui.horizontal_wrapped(|ui| {
        for b in [CoronaryBranch::Global, CoronaryBranch::Pericardium] {
            ui.selectable_value(&mut state.branch, b, crate::labels::branch_name(b));
        }
    });
    ui.label(t!(
        "ischaemia-selected",
        branch = crate::labels::branch_name(state.branch),
        id = state.branch.id()
    ));
    ui.add(egui::Slider::new(&mut state.severity, 0.0..=1.0).text(t!("ischaemia-severity")))
        .on_hover_text(t!("ischaemia-severity-hint"));
    ui.checkbox(&mut state.infarct, t!("ischaemia-infarct"));
    let kind = if state.infarct {
        LesionKind::Infarct
    } else {
        LesionKind::Acute
    };
    ui.horizontal(|ui| {
        if ui.button(t!("ischaemia-apply")).clicked() {
            r.actions.push(Action::Stenosis {
                branch: stenosis_target(state.branch, kind),
                severity: state.severity,
            });
        }
        if ui.button(t!("ischaemia-reperfuse")).clicked() {
            r.actions.push(Action::Stenosis {
                branch: stenosis_target(state.branch, kind),
                severity: 0.0,
            });
        }
        if ui.button(t!("ischaemia-clear-all")).clicked() {
            r.actions.push(Action::Stenosis {
                branch: "all".into(),
                severity: 0.0,
            });
        }
    });
    ui.separator();
    ui.strong(t!("ischaemia-presets"));
    ui.horizontal_wrapped(|ui| {
        for p in IschaemiaPreset::ALL {
            if ui
                .button(crate::labels::preset_name(p))
                .on_hover_text(p.description())
                .clicked()
            {
                r.actions.extend(p.actions());
            }
        }
    });
    if !lesions.is_empty() {
        ui.separator();
        ui.strong(t!("ischaemia-in-effect"));
        for l in lesions {
            let kind = match l.kind {
                LesionKind::Acute => t!("ischaemia-acute"),
                LesionKind::Infarct => t!("ischaemia-infarct-kind"),
                LesionKind::Evolving => t!("ischaemia-evolving-kind"),
            };
            ui.label(t!(
                "ischaemia-lesion-line",
                branch = crate::labels::branch_name(l.branch),
                kind = kind,
                severity = crate::i18n::number(l.severity, 2)
            ));
        }
    }
    r.highlight = hovered.or(Some(state.branch));
    r
}

/// Small spheres over the cells of `branch`'s territory (every `stride`-th cell whose weight is
/// above ½), for the 3-D heart. `weights` is `ischaemia::territory_weight` of the branch.
#[must_use]
pub fn territory_glyphs(
    positions: &[Vec3],
    weights: &[f32],
    stride: usize,
    radius_mm: f32,
) -> Vec<GlyphInstance> {
    let colour = [1.0, 0.85, 0.1, 0.8];
    positions
        .iter()
        .zip(weights)
        .enumerate()
        .filter(|(i, (_, w))| **w > 0.5 && i % stride.max(1) == 0)
        .map(|(_, (p, _))| GlyphInstance::sphere(*p, radius_mm, colour))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_name_the_lesion_kind() {
        assert_eq!(stenosis_target(CoronaryBranch::Lad, LesionKind::Acute), "LAD");
        assert_eq!(
            stenosis_target(CoronaryBranch::Rca, LesionKind::Infarct),
            "RCA infarct"
        );
    }

    #[test]
    fn every_tree_branch_has_a_course() {
        for b in CoronaryBranch::ALL {
            let n = branch_path(b).len();
            match b {
                CoronaryBranch::Global | CoronaryBranch::Pericardium => assert_eq!(n, 0),
                _ => assert!(n >= 2, "{b:?}"),
            }
        }
    }
}
