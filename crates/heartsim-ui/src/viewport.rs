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

#[derive(Debug, Clone, Copy, Default)]
pub struct Viewport3d {
    pub yaw_delta: f32,
    pub pitch_delta: f32,
    pub zoom_delta: f32,
}

impl Viewport3d {
    /// Allocate the widget, gather interaction and queue the paint callback.
    pub fn ui(ui: &mut Ui, size: egui::Vec2) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let drag = response.drag_delta();
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        let hovered = response.hovered();
        let ppp = ui.ctx().pixels_per_point();
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            ViewportCallback {
                rect,
                ppp,
                drag: [drag.x, drag.y],
                zoom: if hovered { scroll / 40.0 } else { 0.0 },
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
