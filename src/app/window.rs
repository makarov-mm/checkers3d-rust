//! The window's side of the game: opening it, its events, the pace of the frames, and the housekeeping of its
//! place on the screen, the full screen and the clipboard.

use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as LogicalKey, KeyCode, NamedKey, PhysicalKey};
use winit::window::{CursorIcon, Fullscreen, Window, WindowId};

use super::{App, DEFAULT_SIZE, MIN_SIZE, TITLE, widgets};
use crate::Result;
use crate::platform::{self, GlWindow};
use crate::settings::WindowPlacement;

/// What the mouse did since the last frame.
pub(super) struct Pointer {
    pub x: f32,
    pub y: f32,
    pub left_down: bool,
    pub right_down: bool,
    pub left_pressed: bool,
    pub left_released: bool,
    pub right_pressed: bool,
    pub right_released: bool,
    pub wheel: f32,
    pub consumed: bool, // taken by the interface
}

impl Default for Pointer {
    fn default() -> Pointer {
        Pointer {
            x: -1.0,
            y: -1.0,
            left_down: false,
            right_down: false,
            left_pressed: false,
            left_released: false,
            right_pressed: false,
            right_released: false,
            wheel: 0.0,
            consumed: false,
        }
    }
}

/// A key pressed, as the game's shortcuts name it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Key {
    Character(char), // a letter, in upper case, or a digit
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Tab,
    Escape,
    Enter,
    Space,
    F1,
    F11,
}

impl Key {
    fn of(event: &KeyEvent) -> Option<Key> {
        match &event.logical_key {
            LogicalKey::Named(NamedKey::Backspace) => return Some(Key::Backspace),
            LogicalKey::Named(NamedKey::Delete) => return Some(Key::Delete),
            LogicalKey::Named(NamedKey::ArrowLeft) => return Some(Key::Left),
            LogicalKey::Named(NamedKey::ArrowRight) => return Some(Key::Right),
            LogicalKey::Named(NamedKey::ArrowUp) => return Some(Key::Up),
            LogicalKey::Named(NamedKey::ArrowDown) => return Some(Key::Down),
            LogicalKey::Named(NamedKey::Home) => return Some(Key::Home),
            LogicalKey::Named(NamedKey::End) => return Some(Key::End),
            LogicalKey::Named(NamedKey::Tab) => return Some(Key::Tab),
            LogicalKey::Named(NamedKey::Escape) => return Some(Key::Escape),
            LogicalKey::Named(NamedKey::Enter) => return Some(Key::Enter),
            LogicalKey::Named(NamedKey::Space) => return Some(Key::Space),
            LogicalKey::Named(NamedKey::F1) => return Some(Key::F1),
            LogicalKey::Named(NamedKey::F11) => return Some(Key::F11),
            LogicalKey::Character(text) => {
                let mut characters = text.chars();
                if let (Some(character), None) = (characters.next(), characters.next()) {
                    if character.is_ascii_alphanumeric() {
                        return Some(Key::Character(character.to_ascii_uppercase()));
                    }
                }
            }
            _ => {}
        }

        // A key that types something else, as in a Cyrillic layout: the letter at its place on a QWERTY keyboard.
        #[rustfmt::skip]
        const LETTERS: [KeyCode; 26] = [
            KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC, KeyCode::KeyD, KeyCode::KeyE, KeyCode::KeyF, KeyCode::KeyG, KeyCode::KeyH, KeyCode::KeyI,
            KeyCode::KeyJ, KeyCode::KeyK, KeyCode::KeyL, KeyCode::KeyM, KeyCode::KeyN, KeyCode::KeyO, KeyCode::KeyP, KeyCode::KeyQ, KeyCode::KeyR,
            KeyCode::KeyS, KeyCode::KeyT, KeyCode::KeyU, KeyCode::KeyV, KeyCode::KeyW, KeyCode::KeyX, KeyCode::KeyY, KeyCode::KeyZ,
        ];
        #[rustfmt::skip]
        const DIGITS: [(KeyCode, KeyCode); 10] = [
            (KeyCode::Digit0, KeyCode::Numpad0), (KeyCode::Digit1, KeyCode::Numpad1), (KeyCode::Digit2, KeyCode::Numpad2),
            (KeyCode::Digit3, KeyCode::Numpad3), (KeyCode::Digit4, KeyCode::Numpad4), (KeyCode::Digit5, KeyCode::Numpad5),
            (KeyCode::Digit6, KeyCode::Numpad6), (KeyCode::Digit7, KeyCode::Numpad7), (KeyCode::Digit8, KeyCode::Numpad8),
            (KeyCode::Digit9, KeyCode::Numpad9),
        ];
        let PhysicalKey::Code(code) = event.physical_key else { return None };
        let letter = LETTERS.iter().position(|key| *key == code).map(|index| char::from(b'A' + index as u8));
        let digit = DIGITS.iter().position(|(row, pad)| *row == code || *pad == code).map(|index| char::from(b'0' + index as u8));
        letter.or(digit).map(Key::Character)
    }
}

/// Where the window opens: where it was, if that is still on a screen, or in the middle of the main screen.
fn initial_placement(event_loop: &ActiveEventLoop, saved: Option<WindowPlacement>) -> (PhysicalSize<u32>, Option<PhysicalPosition<i32>>) {
    if let Some(saved) = saved {
        let on_screen = |monitor: winit::monitor::MonitorHandle| {
            let (position, size) = (monitor.position(), monitor.size());
            let across = saved.left < position.x + size.width as i32 && saved.left + saved.width > position.x;
            let down = saved.top < position.y + size.height as i32 && saved.top + saved.height > position.y;
            across && down
        };
        if event_loop.available_monitors().any(on_screen) || event_loop.available_monitors().next().is_none() {
            return (PhysicalSize::new(saved.width as u32, saved.height as u32), Some(PhysicalPosition::new(saved.left, saved.top)));
        }
    }
    // A screen whose size is not known is no guide.
    let monitor = event_loop.primary_monitor().or_else(|| event_loop.available_monitors().next());
    let Some(monitor) = monitor.filter(|monitor| monitor.size().width > 0 && monitor.size().height > 0) else {
        return (PhysicalSize::new(DEFAULT_SIZE.0 as u32, DEFAULT_SIZE.1 as u32), None);
    };
    let (scale, size, position) = (monitor.scale_factor(), monitor.size(), monitor.position());
    let width = ((DEFAULT_SIZE.0 * scale) as u32).min(size.width * 92 / 100);
    let height = ((DEFAULT_SIZE.1 * scale) as u32).min(size.height * 92 / 100);
    let corner = PhysicalPosition::new(position.x + (size.width as i32 - width as i32) / 2, position.y + (size.height as i32 - height as i32) / 2);
    (PhysicalSize::new(width, height), Some(corner))
}

/// Opens the game's window, hidden until there is something to show in it.
pub(super) fn open(event_loop: &ActiveEventLoop, saved: Option<WindowPlacement>, maximized: bool) -> Result<GlWindow> {
    let (size, position) = initial_placement(event_loop, saved);
    let mut attributes = Window::default_attributes()
        .with_title(TITLE)
        .with_inner_size(size)
        .with_min_inner_size(LogicalSize::new(MIN_SIZE.0, MIN_SIZE.1))
        .with_window_icon(platform::window_icon())
        // A dark title bar, to go with the game, where the system offers one.
        .with_theme(Some(winit::window::Theme::Dark))
        .with_maximized(maximized)
        .with_visible(false);
    if let Some(position) = position {
        attributes = attributes.with_position(position);
    }
    GlWindow::new(event_loop, attributes)
}

impl App {
    fn handle_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) -> Result<()> {
        if !matches!(event, WindowEvent::RedrawRequested) {
            self.dirty = true;
        }
        match event {
            WindowEvent::CloseRequested => {
                self.save();
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                self.width = size.width as i32;
                self.height = size.height as i32;
                self.gl.resize(size.width, size.height);
                self.track_placement();
            }
            WindowEvent::Moved(_) => self.track_placement(),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => self.scale = scale_factor as f32,
            WindowEvent::RedrawRequested => self.frame()?,
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer.x = position.x as f32;
                self.pointer.y = position.y as f32;
            }
            WindowEvent::CursorLeft { .. } => {
                // Nothing stays highlighted under a pointer that has left the window.
                if !self.pointer.left_down && !self.pointer.right_down {
                    self.pointer.x = -1.0;
                    self.pointer.y = -1.0;
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;
                match button {
                    MouseButton::Left => {
                        self.pointer.left_down = pressed;
                        self.pointer.left_pressed |= pressed;
                        self.pointer.left_released |= !pressed;
                    }
                    MouseButton::Right => {
                        self.pointer.right_down = pressed;
                        self.pointer.right_pressed |= pressed;
                        self.pointer.right_released |= !pressed;
                    }
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.pointer.wheel += match delta {
                    MouseScrollDelta::LineDelta(_, lines) => lines,
                    MouseScrollDelta::PixelDelta(pixels) => pixels.y as f32 / (50.0 * self.scale),
                };
            }
            WindowEvent::Focused(false) => {
                // A button held when the window loses the mouse is let go.
                self.pointer.left_released |= self.pointer.left_down;
                self.pointer.right_released |= self.pointer.right_down;
                self.pointer.left_down = false;
                self.pointer.right_down = false;
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                // With Alt held the keys are the system's, but for the one that fills the screen.
                match Key::of(&event) {
                    Some(Key::Enter) if self.modifiers.alt_key() => self.toggle_full_screen(),
                    Some(key) if !self.modifiers.alt_key() => self.handle_key(key),
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Asks for the next frame, or lets the loop wait: nothing moving and nothing new means no drawing of the
    /// same picture again.
    fn schedule(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);
        let visible = self.width > 0 && self.height > 0 && self.gl.window.is_minimized() != Some(true);
        if visible && (self.dirty || self.needs_animation()) {
            self.dirty = false;
            self.gl.window.request_redraw();
        } else {
            self.idle = true;
        }
    }

    /// Starts a frame: the seconds since the last one, or nothing while the window has no area to draw in.
    pub(super) fn begin_frame(&mut self) -> Result<Option<f32>> {
        let now = Instant::now();
        if std::mem::take(&mut self.idle) {
            self.last_frame = now; // the time spent waiting is not animation time
        }
        let seconds = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        self.time += seconds;
        if self.width <= 0 || self.height <= 0 {
            return Ok(None);
        }
        if self.scale != self.font_scale {
            self.painter.set_fonts(widgets::build_fonts(&mut self.fonts, self.scale)?);
            self.font_scale = self.scale;
        }
        Ok(Some(seconds))
    }

    /// Ends a frame: what the mouse did has been dealt with, and the picture goes to the screen.
    pub(super) fn end_frame(&mut self) {
        // The interface answers the mouse's buttons and wheel while it is drawn, after the scene: what they did
        // shows in a frame more.
        let pointer = &self.pointer;
        if pointer.left_pressed || pointer.left_released || pointer.right_pressed || pointer.right_released || pointer.wheel != 0.0 {
            self.dirty = true;
        }
        self.pointer.left_pressed = false;
        self.pointer.left_released = false;
        self.pointer.right_pressed = false;
        self.pointer.right_released = false;
        self.pointer.wheel = 0.0;
        self.pointer.consumed = false;
        if self.cursor_hand != self.shown_cursor_hand {
            self.shown_cursor_hand = self.cursor_hand;
            self.gl.window.set_cursor(if self.cursor_hand { CursorIcon::Pointer } else { CursorIcon::Default });
        }
        self.gl.swap_buffers();
        // Swapping the buffers waits for the screen; where it does not, the loop sleeps a little.
        if self.last_frame.elapsed() < Duration::from_millis(2) {
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// Notes where the window is while it is neither maximized nor full screen, to open there next time.
    pub(super) fn track_placement(&mut self) {
        if self.full_screen {
            return;
        }
        let window = &self.gl.window;
        self.maximized = window.is_maximized();
        let size = window.inner_size();
        if self.maximized || window.is_minimized() == Some(true) || size.width == 0 || size.height == 0 {
            return;
        }
        let corner = window.outer_position().unwrap_or_default();
        self.placement = Some(WindowPlacement { left: corner.x, top: corner.y, width: size.width as i32, height: size.height as i32 });
    }

    pub(super) fn toggle_full_screen(&mut self) {
        self.full_screen = !self.full_screen;
        self.gl.window.set_fullscreen(self.full_screen.then_some(Fullscreen::Borderless(None)));
    }

    fn clipboard(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        self.clipboard.as_mut()
    }

    /// Puts a text on the clipboard; false when the system does not take it.
    pub(super) fn copy_text(&mut self, text: String) -> bool {
        self.clipboard().is_some_and(|clipboard| clipboard.set_text(text).is_ok())
    }

    /// The text on the clipboard; empty when it holds none.
    pub(super) fn pasted_text(&mut self) -> String {
        self.clipboard().and_then(|clipboard| clipboard.get_text().ok()).unwrap_or_default()
    }
}

/// Starts the game when the system is ready for its window, and passes on what happens to it.
#[derive(Default)]
struct Shell {
    app: Option<App>,
    failure: Option<Box<dyn std::error::Error>>,
}

impl Shell {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: Box<dyn std::error::Error>) {
        self.app = None;
        self.failure = Some(error);
        event_loop.exit();
    }
}

impl ApplicationHandler for Shell {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.app.is_none() && self.failure.is_none() {
            match App::new(event_loop) {
                Ok(app) => self.app = Some(app),
                Err(error) => self.fail(event_loop, error),
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if let Some(app) = &mut self.app {
            if let Err(error) = app.handle_event(event_loop, event) {
                self.fail(event_loop, error);
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(app) = &mut self.app {
            app.schedule(event_loop);
        }
    }
}

pub fn run() -> Result<()> {
    let event_loop = EventLoop::new()?;
    let mut shell = Shell::default();
    event_loop.run_app(&mut shell)?;
    shell.failure.map_or(Ok(()), Err)
}
