//! What every panel and dialog of the interface is made of: buttons, selectors and toasts, and the measures
//! and colors they share.

use super::App;
use crate::Result;
use crate::gfx;
use crate::platform::GlWindow;
use crate::renderer::FrostedPanel;
use crate::sound::Effect;
use crate::strings::{Str, tr};
use crate::text::{Font, FontLibrary, WEIGHT_NORMAL, WEIGHT_SEMIBOLD};
use crate::themes::{Theme, themes};
use crate::ui::{self, FontId, Painter, Rect, rect, rgb, rgba};

pub(super) const PANEL_WIDTH: f32 = 344.0;
pub(super) const PANEL_MARGIN: f32 = 16.0;
pub(super) const TEXT_COLOR: ui::Color = rgb(236, 239, 245);
pub(super) const MUTED_COLOR: ui::Color = rgb(150, 158, 176);

/// The widgets numbered above this one belong to the dialog that is open over everything else.
pub(super) const MODAL_WIDGETS: i32 = 100;

/// A line of news at the bottom of the window, gone after a few seconds.
pub(super) struct Toast {
    text: String,
    age: f32,
}

/// The body, heading and display fonts at a scale of the interface.
pub(super) fn build_fonts(library: &mut FontLibrary, scale: f32) -> Result<[Font; 3]> {
    Ok([
        Font::new(library, 15.0 * scale, WEIGHT_NORMAL)?,
        Font::new(library, 19.0 * scale, WEIGHT_SEMIBOLD)?,
        Font::new(library, 31.0 * scale, WEIGHT_SEMIBOLD)?,
    ])
}

/// The game's name in the middle of the window, while the scene is made ready.
pub(super) fn show_loading(gl: &GlWindow, painter: &mut Painter, title: &str) {
    let size = gl.window.inner_size();
    let (width, height) = (size.width as i32, size.height as i32);
    gfx::bind_window_framebuffer();
    gfx::viewport(width, height);
    gfx::clear_window(0.016, 0.017, 0.022);
    painter.begin(width, height);
    painter.text_centered(FontId::Display, rect(0.0, 0.0, width as f32, height as f32), title, TEXT_COLOR);
    painter.flush();
    gl.swap_buffers();
}

impl App {
    pub(super) fn t(&self, id: Str) -> &'static str {
        tr(id, self.language)
    }

    pub(super) fn theme(&self) -> &'static Theme {
        &themes()[self.theme]
    }

    /// The theme's highlight, as a color of the interface.
    pub(super) fn accent(&self) -> ui::Color {
        ui::Color::from_linear(self.theme().accent, 1.0)
    }

    pub(super) fn panel_rect(&self) -> Rect {
        let (width, margin) = (PANEL_WIDTH * self.scale, PANEL_MARGIN * self.scale);
        rect(self.width as f32 - (width + margin) * self.panel_slide, margin, width, self.height as f32 - 2.0 * margin)
    }

    /// The part of the window beside the panel.
    pub(super) fn free_area(&self) -> Rect {
        rect(0.0, 0.0, self.panel_rect().x.min(self.width as f32), self.height as f32)
    }

    /// The panel sliding in or out, and the room it leaves for the scene.
    pub(super) fn slide_panel(&mut self, seconds: f32) -> f32 {
        let target = if self.panel_visible { 1.0 } else { 0.0 };
        self.panel_slide += (target - self.panel_slide) * (1.0 - (-seconds * 12.0).exp());
        if (target - self.panel_slide).abs() < 0.001 {
            self.panel_slide = target;
        }
        let panel_space = (PANEL_WIDTH + PANEL_MARGIN) * self.scale * self.panel_slide;
        1.0 - panel_space / self.width.max(1) as f32
    }

    /// The glass behind a card of the interface, its corners rounded as the card's.
    pub(super) fn frosted(&self, area: Rect, radius: f32, opacity: f32) -> FrostedPanel {
        FrostedPanel { x: area.x, y: area.y, width: area.width, height: area.height, radius: radius * self.scale, opacity }
    }

    pub(super) fn show_toast(&mut self, text: impl Into<String>) {
        if self.toasts.len() >= 3 {
            self.toasts.remove(0);
        }
        self.toasts.push(Toast { text: text.into(), age: 0.0 });
    }

    pub(super) fn age_toasts(&mut self, seconds: f32) {
        for toast in &mut self.toasts {
            toast.age += seconds;
        }
        self.toasts.retain(|toast| toast.age <= 3.2);
    }

    pub(super) fn draw_toasts(&mut self) {
        let scale = self.scale;
        let free = self.free_area();
        let mut y = self.height as f32 - 40.0 * scale;
        for toast in self.toasts.iter().rev() {
            let fade = (toast.age * 6.0).min((2.8 - toast.age) * 3.0).min(1.0);
            let (width, height) = (self.painter.measure(FontId::Body, &toast.text) + 40.0 * scale, 36.0 * scale);
            let pill = rect(free.x + (free.width - width) / 2.0, y - height, width, height);
            self.painter.shadow(pill, 14.0 * scale, rgba(0, 0, 0, 110).fade(fade));
            self.painter.fill(pill, height / 2.0, rgba(20, 22, 30, 225).fade(fade));
            self.painter.text_centered(FontId::Body, pill, &toast.text, TEXT_COLOR.fade(fade));
            y -= height + 8.0 * scale;
        }
    }

    /// A button; true when it has been clicked. An accented one is tinted with the theme's highlight.
    pub(super) fn button(&mut self, id: i32, area: Rect, label: &str, enabled: bool, accent: bool) -> bool {
        // While a dialog is open, only its own widgets answer.
        let interactive = enabled && (!self.modal() || id > MODAL_WIDGETS);
        let hover = interactive && area.contains(self.pointer.x, self.pointer.y);
        if hover {
            self.cursor_hand = true;
        }
        if hover && self.pointer.left_pressed {
            self.active_widget = id;
            self.pointer.consumed = true;
        }
        let mut clicked = false;
        if self.pointer.left_released && self.active_widget == id {
            clicked = hover;
            self.active_widget = 0;
            self.pointer.consumed = true;
        }
        let pressed = hover && self.active_widget == id && self.pointer.left_down;
        let mut fill = rgba(
            255,
            255,
            255,
            if !enabled {
                7
            } else if pressed {
                52
            } else if hover {
                34
            } else {
                17
            },
        );
        if accent && enabled {
            fill = fill.mix(ui::Color::from_linear(self.theme().accent, f32::from(fill.a) / 255.0 * 2.2), 0.6);
        }
        self.painter.fill(area, 9.0 * self.scale, fill);
        self.painter.text_centered(FontId::Body, area, label, if enabled { TEXT_COLOR } else { TEXT_COLOR.fade(0.35) });
        if clicked {
            self.sound.play(Effect::Click);
        }
        clicked
    }

    /// A label with a value stepped by two arrows; returns -1 or 1 when one is clicked, else 0.
    pub(super) fn selector(&mut self, id: i32, area: Rect, label: &str, value: &str) -> i32 {
        let scale = self.scale;
        let label_top = area.y + (area.height - self.painter.line_height(FontId::Body)) / 2.0;
        self.painter.text(FontId::Body, area.x, label_top, label, MUTED_COLOR);
        let arrow = area.height;
        let right = rect(area.right() - arrow, area.y, arrow, area.height);
        let left = rect(area.x + area.width * 0.36 - arrow - 4.0 * scale, area.y, arrow, area.height);
        self.painter.fill(rect(left.right() + 2.0 * scale, area.y, right.x - left.right() - 4.0 * scale, area.height), 9.0 * scale, rgba(255, 255, 255, 9));
        self.painter.text_centered(FontId::Body, rect(left.right(), area.y, right.x - left.right(), area.height), value, TEXT_COLOR);
        let mut change = 0;
        if self.button(id, left, "", true, false) {
            change = -1;
        }
        if self.button(id + 1, right, "", true, false) {
            change = 1;
        }
        let size = 5.0 * scale;
        self.painter.arrow(left.x + left.width / 2.0, left.y + left.height / 2.0, size * 0.8, size, -1.0, TEXT_COLOR);
        self.painter.arrow(right.x + right.width / 2.0, right.y + right.height / 2.0, size * 0.8, size, 1.0, TEXT_COLOR);
        change
    }
}
