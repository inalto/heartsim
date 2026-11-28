//! 3-D viewport widget: wraps `heartsim_render::SceneRenderer` in an `egui_wgpu` paint callback.
//! The renderer lives in egui-wgpu's `CallbackResources` (type-map) so the callback can reach it.

use egui::{Rect, Sense, Ui};
use egui_wgpu::{CallbackResources, CallbackTrait, ScreenDescriptor};
use heartsim_render::SceneRenderer;

/// Stored once in `RenderState::renderer.write().callback_resources`.
#[derive(Debug)]
pub struct ViewportResources {
    pub scene: SceneRenderer,
    /// Scalars to upload this frame (set by the app before `ui`), or `None` to keep the previous.
    pub pending_scalars: Option<Vec<f32>>,
}

/// What the pointer and fingers ask of a zoomable view this frame (see [`zoom_input`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoomInput {
    /// Mouse-wheel notches, positive = zoom in (one notch is 40 points of smooth scroll). Zero
    /// while Ctrl (⌘ on macOS) is held: egui then reports the wheel as [`Self::factor`].
    pub wheel: f32,
    /// Multiplicative zoom of the frame, above 1 = zoom in. Egui combines a two-finger touch
    /// pinch, a trackpad magnify gesture and Ctrl/⌘ + wheel in it. 1 when nothing zooms.
    pub factor: f32,
    /// Two or more fingers are down: a drag then belongs to the gesture, not to orbiting the
    /// camera or to dragging an object in the scene.
    pub pinching: bool,
}

impl Default for ZoomInput {
    fn default() -> Self {
        Self {
            wheel: 0.0,
            factor: 1.0,
            pinching: false,
        }
    }
}

/// Reads the zoom of this frame from `input`. `hovered` says the pointer (or the centre of a
/// pinch) is over the view: a view that is not hovered gets no wheel and no factor, so a pinch on
/// the ECG strip does not also move the 3-D heart beside it. Pure, so it is tested without a GPU.
#[must_use]
pub fn zoom_input(input: &egui::InputState, hovered: bool) -> ZoomInput {
    let pinching = input.multi_touch().is_some();
    if !hovered {
        return ZoomInput {
            pinching,
            ..ZoomInput::default()
        };
    }
    let factor = input.zoom_delta();
    ZoomInput {
        wheel: input.smooth_scroll_delta.y / 40.0,
        factor: if factor.is_finite() && factor > 0.0 {
            factor
        } else {
            1.0
        },
        pinching,
    }
}

/// The pointer is over `rect`, or the centre of a two-finger gesture is (a touch screen reports no
/// hover once the finger has landed elsewhere).
#[must_use]
pub fn pointer_or_pinch_over(ui: &Ui, rect: Rect) -> bool {
    ui.rect_contains_pointer(rect)
        || ui.input(|i| i.multi_touch().is_some_and(|t| rect.contains(t.center_pos)))
}

/// Two fingers are down this frame (a pinch is going on): callers that start an object drag in a
/// 3-D view must not start one then.
#[must_use]
pub fn pinching(ui: &Ui) -> bool {
    ui.input(|i| i.multi_touch().is_some())
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Viewport3d {
    pub yaw_delta: f32,
    pub pitch_delta: f32,
    pub zoom_delta: f32,
}

impl Viewport3d {
    /// Allocate the widget, gather interaction and queue the paint callback.
    pub fn ui(ui: &mut Ui, size: egui::Vec2) -> egui::Response {
        Self::ui_with_orbit(ui, size, true)
    }

    /// [`Viewport3d::ui`], with dragging orbiting the camera only when `orbit` is set (off while
    /// the user drags an object in the scene, e.g. an electrode).
    pub fn ui_with_orbit(ui: &mut Ui, size: egui::Vec2, orbit: bool) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let hovered = response.hovered() || pointer_or_pinch_over(ui, rect);
        let zoom = ui.input(|i| zoom_input(i, hovered));
        // two fingers are a zoom, not an orbit
        let drag = if orbit && !zoom.pinching {
            response.drag_delta()
        } else {
            egui::Vec2::ZERO
        };
        let ppp = ui.ctx().pixels_per_point();
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            ViewportCallback {
                rect,
                ppp,
                drag: [drag.x, drag.y],
                zoom: zoom.wheel,
                zoom_factor: zoom.factor,
            },
        ));
        response
    }
}

struct ViewportCallback {
    rect: Rect,
    ppp: f32,
    drag: [f32; 2],
    zoom: f32,
    zoom_factor: f32,
}

impl CallbackTrait for ViewportCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(res) = resources.get_mut::<ViewportResources>() {
            res.scene.camera.orbit(self.drag[0], self.drag[1]);
            if self.zoom != 0.0 {
                res.scene.camera.zoom(self.zoom);
            }
            if (self.zoom_factor - 1.0).abs() > 1e-6 {
                res.scene.camera.zoom_by(self.zoom_factor);
            }
            if let Some(s) = res.pending_scalars.take() {
                res.scene.set_scalars(queue, &s);
            }
            let size = (
                (self.rect.width() * self.ppp) as u32,
                (self.rect.height() * self.ppp) as u32,
            );
            res.scene.render_offscreen(device, queue, encoder, size);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        if let Some(res) = resources.get::<ViewportResources>() {
            res.scene.blit(pass);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Pos2, RawInput, TouchDeviceId, TouchId, TouchPhase};

    /// The `ZoomInput` egui reports for `events` over a few frames (touch gestures and the wheel's
    /// smoothing need more than one), as seen by the last frame.
    fn read(frames: Vec<Vec<Event>>, hovered: bool) -> ZoomInput {
        let ctx = egui::Context::default();
        let mut out = ZoomInput::default();
        for (k, events) in frames.into_iter().enumerate() {
            let input = RawInput {
                time: Some(k as f64 / 60.0),
                events,
                ..RawInput::default()
            };
            let mut full = ctx.run_ui(input, |ui| {
                out = ui.input(|i| zoom_input(i, hovered));
            });
            full.textures_delta.clear();
        }
        out
    }

    fn touch(id: u64, phase: TouchPhase, x: f32) -> Event {
        Event::Touch {
            device_id: TouchDeviceId(0),
            id: TouchId(id),
            phase,
            pos: Pos2::new(x, 100.0),
            force: None,
        }
    }

    #[test]
    fn a_zoom_event_over_the_view_gives_a_factor_above_one() {
        let z = read(vec![vec![Event::Zoom(1.25)]], true);
        assert!((z.factor - 1.25).abs() < 1e-6, "{z:?}");
        assert!(!z.pinching, "a trackpad magnify is not a two-finger touch");
    }

    #[test]
    fn no_zoom_without_the_pointer_over_the_view() {
        let z = read(vec![vec![Event::Zoom(1.25)]], false);
        assert_eq!(z, ZoomInput::default());
    }

    #[test]
    fn a_plain_wheel_gives_the_wheel_and_no_factor() {
        let wheel = Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 80.0),
            phase: TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
            modifiers: egui::Modifiers::NONE,
        };
        let mut frames = vec![vec![wheel]];
        frames.extend((0..40).map(|_| vec![]));
        let mut total = 0.0;
        let ctx = egui::Context::default();
        for (k, events) in frames.into_iter().enumerate() {
            let input = RawInput {
                time: Some(k as f64 / 60.0),
                events,
                ..RawInput::default()
            };
            let mut full = ctx.run_ui(input, |ui| {
                let z = ui.input(|i| zoom_input(i, true));
                assert!((z.factor - 1.0).abs() < 1e-6, "{z:?}");
                total += z.wheel;
            });
            full.textures_delta.clear();
        }
        assert!(
            (total - 2.0).abs() < 1e-3,
            "80 points are two notches, got {total}"
        );
    }

    #[test]
    fn two_fingers_moving_apart_zoom_in_and_count_as_pinching() {
        let down = vec![
            touch(1, TouchPhase::Start, 100.0),
            touch(2, TouchPhase::Start, 140.0),
        ];
        let apart = vec![
            touch(1, TouchPhase::Move, 80.0),
            touch(2, TouchPhase::Move, 160.0),
        ];
        // egui starts a gesture only once it knows where the pointer is, as it does on a touch screen
        let z = read(
            vec![vec![Event::PointerMoved(Pos2::new(120.0, 100.0))], down, apart],
            true,
        );
        assert!(z.pinching, "{z:?}");
        assert!(z.factor > 1.0, "{z:?}");
        let one = read(vec![vec![], vec![touch(1, TouchPhase::Start, 100.0)]], true);
        assert!(!one.pinching, "one finger is not a pinch");
    }
}
