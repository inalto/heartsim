//! Tutorial highlights: a tutorial page names a control by a stable tool id and the control draws
//! a pulsing outline around itself while that id is active. Controls opt in with [`mark`]; the
//! active id lives in egui's temporary memory, so widgets need no extra state.

use egui::{Color32, Context, Id, LayerId, Order, Response, Stroke, StrokeKind};

/// Tool ids understood by the application's controls.
pub const TOOLS: [&str; 7] = [
    "rhythm_picker",
    "pause",
    "speed",
    "pacemaker",
    "lead_selector",
    "camera_views",
    "timeline_editor",
];

const OUTLINE: Color32 = Color32::from_rgb(255, 196, 0);

fn key() -> Id {
    Id::unique("heartsim_tutorial_highlight")
}

/// Make `tool` the highlighted control (`None` clears it).
pub fn set(ctx: &Context, tool: Option<&str>) {
    ctx.data_mut(|d| match tool {
        Some(t) => {
            d.insert_temp(key(), t.to_owned());
        }
        None => d.remove::<String>(key()),
    });
}

/// The highlighted tool id, if any.
#[must_use]
pub fn current(ctx: &Context) -> Option<String> {
    ctx.data(|d| d.get_temp::<String>(key()))
}

/// Outline `response`'s rect if `tool` is the highlighted control.
pub fn mark(response: &Response, tool: &str) {
    if current(&response.ctx).as_deref() == Some(tool) {
        let t = response.ctx.input(|i| i.time);
        let alpha = 0.55 + 0.45 * (t * 4.0).sin().abs();
        let painter = response.ctx.layer_painter(LayerId::new(Order::Foreground, key()));
        painter.rect_stroke(
            response.rect.expand(3.0),
            4.0,
            Stroke::new(2.5, OUTLINE.gamma_multiply(alpha as f32)),
            StrokeKind::Outside,
        );
        response.ctx.request_repaint();
    }
}
