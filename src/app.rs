//! The game's window: it runs the game, the computer players and the interface, and draws them.

mod widgets;
mod window;

use std::hash::{BuildHasher, Hasher};
use std::time::Instant;

use winit::event_loop::ActiveEventLoop;
use winit::keyboard::ModifiersState;

use crate::Result;
use crate::board_geometry::{self as board, board_bit};
use crate::board_view::{BoardView, Event};
use crate::camera::OrbitCamera;
use crate::draughts::{self, Color, EndReason, Game, GameResult, GameStatus, Move, Square, Variant, squares, to_notation};
use crate::engine::{AiPlayer, SearchLimits, SearchResult};
use crate::gfx;
use crate::math::{Vec3, vec3};
use crate::piece_models as pieces;
use crate::platform::GlWindow;
use crate::renderer::{Arrow, BoardMarks, FrostedPanel, Renderer, Scene};
use crate::settings::{Settings, WindowPlacement};
use crate::sound::{Effect, SoundBank};
use crate::strings::{Language, Str, tr};
use crate::text::FontLibrary;
use crate::themes::{THEME_COUNT, themes};
use crate::ui::{self, FontId, Painter, Rect, rect, rgb, rgba};

pub use window::run;

use widgets::{MUTED_COLOR, TEXT_COLOR, Toast};
use window::{Key, Pointer};

const TITLE: &str = "3D Checkers";
// The window's size where none is remembered, and its smallest, on a screen of the usual scale.
const DEFAULT_SIZE: (f64, f64) = (1500.0, 920.0);
const MIN_SIZE: (f64, f64) = (900.0, 600.0);

// The widgets; a selector takes two numbers, one for each of its arrows.
const HELP_BUTTON: i32 = 1;
const UNDO_BUTTON: i32 = 2;
const REDO_BUTTON: i32 = 3;
const HINT_BUTTON: i32 = 4;
const NEW_GAME_BUTTON: i32 = 5;
const FLIP_BUTTON: i32 = 6;
const MODE_SELECTOR: i32 = 7;
const LEVEL_SELECTOR: i32 = 9;
const RULES_SELECTOR: i32 = 11;
const THEME_SELECTOR: i32 = 13;
const SOUND_BUTTON: i32 = 15;
const LANGUAGE_BUTTON: i32 = 16;
const HELP_CLOSE: i32 = 101;

const WHITE_DISC: ui::Color = rgb(238, 232, 220);
const BLACK_DISC: ui::Color = rgb(22, 22, 26);
const DISC_RIM: ui::Color = rgb(120, 125, 140);

const HELP_ROW: f32 = 27.0;
const HELP_PAD: f32 = 28.0;

/// The keys of the help, each with what it does.
const HELP_LINES: [(Option<Str>, &str, Str); 21] = [
    (Some(Str::KeyClick), "", Str::HelpClick),
    (Some(Str::KeyDrag), "", Str::HelpDrag),
    (Some(Str::KeyRightDrag), "", Str::HelpRotate),
    (Some(Str::KeyWheel), "", Str::HelpZoom),
    (None, "Ctrl+Z  ←  Backspace", Str::HelpUndo),
    (None, "Ctrl+Y  →", Str::HelpRedo),
    (None, "H", Str::HelpHint),
    (None, "N", Str::HelpNew),
    (None, "F", Str::HelpFlip),
    (None, "V", Str::HelpTopView),
    (None, "1 – 5", Str::HelpLevel),
    (None, "M", Str::HelpMode),
    (None, "R", Str::HelpRules),
    (None, "T", Str::HelpTheme),
    (None, "S", Str::HelpSound),
    (None, "L", Str::HelpLanguage),
    (None, "Tab", Str::HelpPanel),
    (None, "F11  Alt+Enter", Str::HelpFullScreen),
    (None, "Ctrl+C", Str::HelpCopy),
    (None, "Ctrl+V", Str::HelpPaste),
    (None, "Esc  F1", Str::HelpClose),
];

/// Who plays whom.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    PlayWhite,
    PlayBlack,
    TwoPlayers,
    Demo,
}

impl Mode {
    const ALL: [Mode; 4] = [Mode::PlayWhite, Mode::PlayBlack, Mode::TwoPlayers, Mode::Demo];

    /// The mode a number of steps on, round and round.
    fn stepped(self, steps: i32) -> Mode {
        Mode::ALL[(self as i32 + steps).rem_euclid(4) as usize]
    }

    fn against_computer(self) -> bool {
        matches!(self, Mode::PlayWhite | Mode::PlayBlack)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Gesture {
    None,
    PressPiece,
    DragPiece,
    TurnCamera,
}

/// Depth, time and the randomness of each level.
fn level_limits(level: i32, seed: u64) -> SearchLimits {
    let (max_depth, milliseconds, random_margin) = match level {
        1 => (2, 3000, 150),
        2 => (4, 3000, 60),
        3 => (8, 2500, 20),
        4 => (64, 1200, 0),
        _ => (64, 3500, 0),
    };
    SearchLimits { max_depth, milliseconds, random_margin, seed }
}

/// A number that differs from run to run and from call to call.
fn random_seed() -> u64 {
    std::collections::hash_map::RandomState::new().build_hasher().finish()
}

/// The marks of the board for a set of dark squares.
fn board_bits(dark_squares: u32) -> u64 {
    squares(dark_squares).fold(0, |bits, square| bits | board_bit(square))
}

/// The middles of the squares a move passes through, from where it starts.
fn route_of(played: &Move) -> Vec<Vec3> {
    std::iter::once(played.from()).chain(played.landings()).map(board::square_center).collect()
}

fn result_title(result: GameResult) -> Str {
    match result {
        GameResult::WhiteWins => Str::WhiteWins,
        GameResult::BlackWins => Str::BlackWins,
        _ => Str::Draw,
    }
}

fn reason_text(status: GameStatus, variant: Variant) -> Str {
    let giveaway = variant == Variant::Giveaway;
    match status.reason {
        EndReason::NoPieces if !giveaway => Str::NoPieces,
        EndReason::NoMoves if !giveaway => Str::NoMoves,
        EndReason::NoPieces | EndReason::NoMoves => Str::NoMovesGiveaway,
        EndReason::Repetition => Str::ByRepetition,
        EndReason::QuietMoves => Str::ByQuietMoves,
        _ => Str::ByLoneKings,
    }
}

fn variant_name(variant: Variant) -> Str {
    match variant {
        Variant::Russian => Str::RulesRussian,
        Variant::English => Str::RulesEnglish,
        Variant::Giveaway => Str::RulesGiveaway,
    }
}

fn disc_color(side: Color) -> ui::Color {
    match side {
        Color::White => WHITE_DISC,
        Color::Black => BLACK_DISC,
    }
}

pub struct App {
    renderer: Renderer,
    painter: Painter,
    fonts: FontLibrary,
    view: BoardView,
    sound: SoundBank,
    clipboard: Option<arboard::Clipboard>,
    settings: Settings,
    language: Language,
    width: i32,
    height: i32,
    scale: f32,      // of the interface: 1 at 96 dots per inch
    font_scale: f32, // the scale the fonts were made for
    dirty: bool,     // something to draw since the last frame
    idle: bool,      // waiting for input: the time waited is not animation time
    full_screen: bool,
    maximized: bool,
    placement: Option<WindowPlacement>,
    last_frame: Instant,
    time: f32,

    game: Game,
    camera: OrbitCamera,
    mode: Mode,
    level: i32,
    theme: usize,
    computer: AiPlayer,
    hinter: AiPlayer,
    computer_thinking: bool,
    computer_clock: f32,
    computer_result: Option<SearchResult>,
    hint_thinking: bool,
    hint: Option<Move>,
    hint_glow: f32,
    announced_end: bool,
    events: Vec<Event>,

    pointer: Pointer,
    modifiers: ModifiersState,
    gesture: Gesture,
    press_x: f32,
    press_y: f32,
    // The pointer when the camera last turned.
    drag_x: f32,
    drag_y: f32,
    press_square: Option<Square>,
    selected: Option<Square>,
    selected_moves: Vec<Move>,
    route: Vec<Square>, // the landings chosen so far, when several captures share an end
    hover_piece: Option<Square>,
    hover_square: Option<Square>,
    cursor_hand: bool,
    shown_cursor_hand: bool,

    active_widget: i32,
    panel_visible: bool,
    panel_slide: f32,
    help_visible: bool,
    move_scroll: f32,
    follow_moves: bool,
    banner_fade: f32,
    toasts: Vec<Toast>,

    // The window goes last: what was made in its OpenGL context is dropped before it.
    gl: GlWindow,
}

impl App {
    fn new(event_loop: &ActiveEventLoop) -> Result<App> {
        let settings = Settings::load();
        let gl = window::open(event_loop, settings.window, settings.maximized)?;
        gfx::set_glsl_version(gl.version());

        let mut painter = Painter::new()?;
        let mut fonts = FontLibrary::new();
        let scale = gl.window.scale_factor() as f32;
        painter.set_fonts(widgets::build_fonts(&mut fonts, scale)?);
        gl.window.set_visible(true);
        let size = gl.window.inner_size();
        let (width, height) = (size.width as i32, size.height as i32);
        let language = settings.language;
        widgets::show_loading(&gl, &mut painter, tr(Str::Title, language));

        let theme = settings.theme as usize;
        let renderer = Renderer::new(&pieces::build_disc(), &mut fonts, &themes()[theme])?;
        let mode = Mode::ALL[settings.mode as usize];
        let game = Game::deserialize(&settings.game).unwrap_or_else(|| Game::new(Variant::ALL[settings.variant as usize]));
        let mut view = BoardView::new();
        view.rebuild(&game, true);
        let mut sound = SoundBank::new();
        sound.enabled = settings.sound;

        let mut app = App {
            renderer,
            painter,
            fonts,
            view,
            sound,
            clipboard: None,
            language,
            width,
            height,
            scale,
            font_scale: scale,
            dirty: true,
            idle: false,
            full_screen: false,
            maximized: settings.maximized,
            placement: settings.window,
            last_frame: Instant::now(),
            time: 0.0,
            announced_end: game.status().over(),
            game,
            camera: OrbitCamera::new(),
            mode,
            level: settings.level,
            theme,
            computer: AiPlayer::new(32),
            hinter: AiPlayer::new(16),
            computer_thinking: false,
            computer_clock: 0.0,
            computer_result: None,
            hint_thinking: false,
            hint: None,
            hint_glow: 0.0,
            events: Vec::new(),
            pointer: Pointer::default(),
            modifiers: ModifiersState::empty(),
            gesture: Gesture::None,
            press_x: 0.0,
            press_y: 0.0,
            drag_x: 0.0,
            drag_y: 0.0,
            press_square: None,
            selected: None,
            selected_moves: Vec::new(),
            route: Vec::new(),
            hover_piece: None,
            hover_square: None,
            cursor_hand: false,
            shown_cursor_hand: false,
            active_widget: 0,
            panel_visible: settings.panel,
            panel_slide: if settings.panel { 1.0 } else { 0.0 },
            help_visible: false,
            move_scroll: 0.0,
            follow_moves: true,
            banner_fade: 0.0,
            toasts: Vec::new(),
            settings,
            gl,
        };
        app.camera.reset(app.view_side());
        app.track_placement();
        Ok(app)
    }

    fn frame(&mut self) -> Result<()> {
        let Some(seconds) = self.begin_frame()? else { return Ok(()) };
        self.renderer.resize(self.width, self.height)?;

        self.update(seconds);

        let theme = self.theme();
        let threatened = self.threatened_by(self.hover_square);

        // The marks: the selected piece and where it can go, the last move, and the pieces that must take.
        let mut marks = BoardMarks { selected: self.selected.map_or(0, board_bit), ..BoardMarks::default() };
        for candidate in self.candidates() {
            *(if candidate.is_capture() { &mut marks.capture } else { &mut marks.quiet }) |= board_bit(candidate.to());
            // The next landing on a longer route.
            if let Some(next) = candidate.landing(self.route.len()).filter(|_| usize::from(candidate.steps) > self.route.len() + 1) {
                marks.quiet |= board_bit(next);
            }
        }
        marks.route = self.route.iter().fold(0, |bits, square| bits | board_bit(*square));
        if let Some(last) = self.game.moves().last() {
            marks.last_move = route_bits(&last.played);
        }
        if self.human_to_move() && !self.view.animating() {
            marks.forced = board_bits(self.game.current().pieces_that_can_take(self.game.rules()));
        }
        marks.threatened = board_bits(threatened);
        marks.hover = self.hover_square.map_or(0, board_bit);

        let arrow = self.hint.as_ref().filter(|_| self.hint_glow > 0.01).map(|hint| Arrow {
            route: route_of(hint),
            color: theme.accent * 3.5,
            alpha: 0.9 * self.hint_glow,
        });
        let scene = Scene {
            view: self.camera.view(),
            projection: self.camera.projection(self.width as f32 / self.height as f32),
            eye: self.camera.eye(),
            time: self.time,
            pieces: self.view.draws(self.selected, self.hover_piece, threatened, theme.accent, self.time),
            contacts: self.view.contacts(),
            marks,
            arrow,
            panels: self.frosted_panels(),
        };
        self.renderer.render(&scene);

        self.cursor_hand = false;
        self.painter.begin(self.width, self.height);
        self.draw_interface();
        self.painter.flush();
        self.handle_board_pointer();

        self.end_frame();
        Ok(())
    }

    fn update(&mut self, seconds: f32) {
        let space = self.slide_panel(seconds);
        self.camera.set_space(space);
        self.camera.update(seconds);

        self.events.clear();
        self.view.update(seconds, &mut self.events);
        for event in &self.events {
            self.sound.play(match event {
                Event::Step => Effect::Step,
                Event::Jump => Effect::Jump,
                Event::Crown => Effect::Crown,
                Event::Pile => Effect::Pile,
            });
        }

        let over = self.banner_wanted();
        self.banner_fade = (self.banner_fade + if over { seconds } else { -seconds } * 3.0).clamp(0.0, 1.0);
        self.hint_glow = (self.hint_glow + if self.hint.is_some() { seconds } else { -seconds } * 4.0).clamp(0.0, 1.0);
        self.age_toasts(seconds);

        if self.hint_thinking {
            if let Some(result) = self.hinter.take_result() {
                self.hint_thinking = false;
                if !result.best.is_null() && self.human_to_move() {
                    self.hint = Some(result.best);
                    self.show_toast(format!("{}{}", self.t(Str::HintToast), to_notation(&result.best, self.game.variant())));
                }
            } else if !self.hinter.busy() {
                self.hint_thinking = false;
            }
        }

        self.update_computer(seconds);

        if over && !self.announced_end {
            self.announced_end = true;
            self.sound.play(Effect::GameOver);
        }
        if !self.game.status().over() {
            self.announced_end = false;
        }
    }

    /// Whether the banner of the game's end is to be seen: the game is over, and the last piece has come to rest.
    fn banner_wanted(&self) -> bool {
        self.game.status().over() && !self.view.animating()
    }

    fn needs_animation(&self) -> bool {
        // What slides and fades is held against where it is going: it may be at rest when its goal changes.
        let panel_target = if self.panel_visible { 1.0 } else { 0.0 };
        let banner_target = if self.banner_wanted() { 1.0 } else { 0.0 };
        self.camera.moving()
            || self.view.animating()
            || self.computer_thinking
            || self.computer_result.is_some()
            || self.hint_thinking
            || self.selected.is_some()
            || !self.toasts.is_empty()
            || self.panel_slide != panel_target
            || self.banner_fade != banner_target
            || self.hint.is_some()
            || self.hint_glow > 0.0
            || self.gesture != Gesture::None
            || self.pointer.left_down
            || self.pointer.right_down
            || self.hover_square.is_some()
            || (self.human_to_move() && self.game.current().has_capture(self.game.rules()))
            || (!self.game.status().over() && !self.is_human(self.game.current().side_to_move()))
    }

    // The game ---------------------------------------------------------------------------------------

    fn is_human(&self, side: Color) -> bool {
        match self.mode {
            Mode::PlayWhite => side == Color::White,
            Mode::PlayBlack => side == Color::Black,
            Mode::TwoPlayers => true,
            Mode::Demo => false,
        }
    }

    fn human_to_move(&self) -> bool {
        !self.game.status().over() && self.is_human(self.game.current().side_to_move())
    }

    /// The side the board is seen from.
    fn view_side(&self) -> Color {
        if self.mode == Mode::PlayBlack { Color::Black } else { Color::White }
    }

    fn stop_computer(&mut self) {
        self.computer.cancel();
        self.computer_thinking = false;
        self.computer_result = None;
        self.hinter.cancel();
        self.hint_thinking = false;
    }

    fn update_computer(&mut self, seconds: f32) {
        if self.game.status().over() || self.is_human(self.game.current().side_to_move()) {
            return;
        }
        self.computer_clock += seconds;
        if !self.computer_thinking && self.computer_result.is_none() {
            let limits = level_limits(self.level, random_seed());
            self.computer.start(*self.game.current(), self.game.variant(), self.game.repetition_history(), limits);
            self.computer_thinking = true;
            self.computer_clock = 0.0;
        }
        if self.computer_thinking {
            if let Some(result) = self.computer.take_result() {
                self.computer_result = Some(result);
                self.computer_thinking = false;
            } else if !self.computer.busy() {
                self.computer_thinking = false; // cancelled; a new search starts next frame
            }
        }
        // The computer waits for the pieces to settle, and a moment more, so that its move can be followed.
        let pause = if self.mode == Mode::Demo { 0.9 } else { 0.45 };
        if !self.view.animating() && self.computer_clock >= pause && self.gesture == Gesture::None {
            if let Some(result) = self.computer_result.take() {
                if !result.best.is_null() {
                    self.play_move(&result.best);
                }
            }
        }
    }

    /// Sets out a game, the one being played or another, from its first position.
    fn start_game(&mut self, variant: Variant) {
        self.stop_computer();
        self.computer.new_game();
        self.game = Game::new(variant);
        self.view.rebuild(&self.game, true);
        self.camera.reset(self.view_side());
        self.announced_end = false;
        self.after_position_change();
    }

    fn new_game(&mut self) {
        self.start_game(self.game.variant());
    }

    fn play_move(&mut self, played: &Move) {
        let before = *self.game.current();
        if !self.game.play(played) {
            return;
        }
        if let Some(record) = self.game.moves().last() {
            self.view.play_move(&before, record, true);
        }
        self.after_position_change();
    }

    fn undo_one(&mut self) {
        let Some(record) = self.game.moves().last().cloned() else { return };
        self.game.undo();
        self.view.play_move(self.game.current(), &record, false);
    }

    fn redo_one(&mut self) {
        let before = *self.game.current();
        self.game.redo();
        if let Some(record) = self.game.moves().last() {
            self.view.play_move(&before, record, true);
        }
    }

    fn undo(&mut self) {
        self.stop_computer();
        if !self.game.can_undo() {
            self.show_toast(self.t(Str::NothingToUndo));
            return;
        }
        self.undo_one();
        // Against the computer, back to the player's own turn.
        if self.mode.against_computer() && !self.is_human(self.game.current().side_to_move()) && self.game.can_undo() {
            self.undo_one();
        }
        self.after_position_change();
    }

    fn redo(&mut self) {
        self.stop_computer();
        if !self.game.can_redo() {
            return;
        }
        self.redo_one();
        if self.mode.against_computer() && !self.is_human(self.game.current().side_to_move()) && self.game.can_redo() {
            self.redo_one();
        }
        self.after_position_change();
    }

    fn set_mode(&mut self, mode: Mode) {
        self.stop_computer();
        self.mode = mode;
        if mode.against_computer() {
            self.camera.face_side(self.view_side());
        }
        self.clear_selection();
        self.save();
    }

    fn set_level(&mut self, level: i32) {
        self.level = level.clamp(1, 5);
        if self.computer_thinking {
            self.stop_computer(); // starts again at the new level
        }
        self.save();
    }

    /// The rules a number of steps on from the ones being played: other rules make another game.
    fn step_variant(&mut self, steps: i32) {
        let chosen = Variant::ALL[(self.game.variant() as i32 + steps).rem_euclid(Variant::ALL.len() as i32) as usize];
        if chosen == self.game.variant() {
            return;
        }
        self.start_game(chosen);
        self.show_toast(format!("{}{}", self.t(Str::NewRules), self.t(variant_name(chosen))));
    }

    fn set_theme(&mut self, theme: i32) {
        self.theme = theme.rem_euclid(THEME_COUNT as i32) as usize;
        self.renderer.set_theme(self.theme());
        self.save();
    }

    fn toggle_sound(&mut self) {
        self.sound.enabled = !self.sound.enabled;
        self.save();
    }

    fn toggle_language(&mut self) {
        self.language = self.language.other();
        self.save();
    }

    fn request_hint(&mut self) {
        if !self.human_to_move() || self.hint_thinking {
            return;
        }
        self.hint = None;
        let limits = SearchLimits { max_depth: 64, milliseconds: 1200, random_margin: 0, seed: 0 };
        self.hinter.start(*self.game.current(), self.game.variant(), self.game.repetition_history(), limits);
        self.hint_thinking = true;
    }

    fn after_position_change(&mut self) {
        self.clear_selection();
        self.hint = None;
        self.follow_moves = true;
        self.save();
    }

    fn clear_selection(&mut self) {
        self.selected = None;
        self.selected_moves.clear();
        self.route.clear();
        self.hover_square = None;
    }

    fn select(&mut self, square: Square) {
        self.selected = Some(square);
        self.route.clear();
        self.selected_moves = self.game.legal_moves().into_iter().filter(|candidate| candidate.from() == square).collect();
    }

    /// The selected piece's moves that follow the landings chosen so far.
    fn candidates(&self) -> Vec<Move> {
        let follows = |candidate: &&Move| {
            self.route.len() < usize::from(candidate.steps) && candidate.landings().zip(&self.route).all(|(landing, chosen)| landing == *chosen)
        };
        self.selected_moves.iter().filter(follows).copied().collect()
    }

    /// Whether a click on a square goes on with the move: it ends one of the candidates, or is their next landing.
    fn is_target(&self, square: Option<Square>) -> bool {
        let Some(square) = square else { return false };
        self.candidates().iter().any(|candidate| candidate.to() == square || candidate.landing(self.route.len()) == Some(square))
    }

    /// The pieces that a click on a square would take, for the highlight.
    fn threatened_by(&self, square: Option<Square>) -> u32 {
        let Some(square) = square else { return 0 };
        let mut taken = 0;
        for candidate in self.candidates() {
            if candidate.to() == square {
                taken |= candidate.captured;
            } else if candidate.landing(self.route.len()) == Some(square) && candidate.is_capture() {
                taken |= candidate.taken[..=self.route.len()].iter().fold(0, |bits, victim| bits | draughts::bit(Square::from(*victim)));
            }
        }
        taken
    }

    fn choose_square(&mut self, square: Square) {
        let candidates = self.candidates();
        let ending: Vec<&Move> = candidates.iter().filter(|candidate| candidate.to() == square).collect();
        if let [only] = ending[..] {
            self.play_move(only);
            return;
        }
        if ending.len() > 1 {
            // Different captures end here: the player shows the way, square by square.
            self.show_toast(self.t(Str::ChooseRoute));
        } else if candidates.iter().any(|candidate| candidate.landing(self.route.len()) == Some(square)) {
            self.route.push(square);
        }
        if let Some(selected) = self.selected {
            self.view.release(selected);
        }
    }

    fn copy_game(&mut self, position: bool) {
        let text = if position {
            self.game.current().fen(self.game.variant())
        } else {
            self.game.to_pdn(&self.player_name(Color::White), &self.player_name(Color::Black))
        };
        if self.copy_text(text) {
            self.show_toast(self.t(if position { Str::FenCopied } else { Str::PdnCopied }));
        }
    }

    fn paste_game(&mut self) {
        let Ok(game) = Game::parse(&self.pasted_text(), self.game.variant()) else {
            self.show_toast(self.t(Str::NothingToPaste));
            return;
        };
        self.stop_computer();
        self.game = game;
        self.view.rebuild(&self.game, true);
        self.announced_end = self.game.status().over();
        self.after_position_change();
        self.show_toast(self.t(Str::GameLoaded));
    }

    fn save(&mut self) {
        self.settings.theme = self.theme as i32;
        self.settings.level = self.level;
        self.settings.mode = self.mode as i32;
        self.settings.variant = self.game.variant() as i32;
        self.settings.sound = self.sound.enabled;
        self.settings.panel = self.panel_visible;
        self.settings.language = self.language;
        self.settings.game = self.game.serialize();
        self.settings.window = self.placement;
        self.settings.maximized = self.maximized;
        self.settings.save();
    }

    // Input ------------------------------------------------------------------------------------------

    /// Where the ray through the pointer meets the plane at a height.
    fn pointer_on_plane(&self, height: f32) -> Vec3 {
        let (origin, direction) = self.camera.ray(self.pointer.x, self.pointer.y, self.width as f32, self.height as f32);
        if direction.y.abs() < 1e-5 {
            return origin;
        }
        let t = (height - origin.y) / direction.y;
        origin + direction * t.max(0.0)
    }

    fn square_under_pointer(&self) -> Option<Square> {
        board::square_at(self.pointer_on_plane(0.0))
    }

    /// The square of the piece under the pointer, or else the square of the board there.
    fn pointed_square(&self) -> Option<Square> {
        let (origin, direction) = self.camera.ray(self.pointer.x, self.pointer.y, self.width as f32, self.height as f32);
        self.view.pick(origin, direction).or_else(|| self.square_under_pointer())
    }

    fn handle_board_pointer(&mut self) {
        let blocked = self.pointer.consumed || self.help_visible;
        let scale = self.scale;
        let human = self.human_to_move();
        let position = *self.game.current();
        let legal = if human { self.game.legal_moves() } else { Vec::new() };
        let movable = |square: Option<Square>| square.is_some_and(|square| legal.iter().any(|candidate| candidate.from() == square));
        let own_piece =
            |square: Option<Square>| human && square.and_then(|square| position.at(square)).is_some_and(|piece| piece.color == position.side_to_move());

        // What the pointer is over.
        let inside_window = self.pointer.x >= 0.0 && self.pointer.y >= 0.0 && self.pointer.x < self.width as f32 && self.pointer.y < self.height as f32;
        (self.hover_piece, self.hover_square) = match self.gesture {
            Gesture::None | Gesture::DragPiece => {
                let square = if inside_window && !blocked { self.pointed_square() } else { None };
                let dragging = self.gesture == Gesture::DragPiece;
                let target = if dragging { self.square_under_pointer() } else { square };
                (square.filter(|_| !dragging && movable(square)), target.filter(|_| self.is_target(target)))
            }
            _ => (None, None),
        };
        if self.hover_piece.is_some() || self.hover_square.is_some() || self.gesture == Gesture::DragPiece {
            self.cursor_hand = true;
        }

        if !blocked && self.pointer.left_pressed {
            let square = self.pointed_square();
            (self.press_x, self.press_y) = (self.pointer.x, self.pointer.y);
            (self.drag_x, self.drag_y) = (self.pointer.x, self.pointer.y);
            if let Some(target) = square.filter(|_| self.selected.is_some() && self.is_target(square)) {
                self.choose_square(target);
            } else if let Some(piece) = square.filter(|_| movable(square)) {
                if square != self.selected {
                    self.select(piece);
                }
                self.gesture = Gesture::PressPiece;
                self.press_square = square;
            } else {
                if own_piece(square) && position.has_capture(self.game.rules()) {
                    self.show_toast(self.t(Str::TakingIsCompulsory));
                }
                self.gesture = Gesture::TurnCamera;
                self.press_square = None;
            }
        }
        if !self.pointer.consumed && self.pointer.right_pressed && self.gesture == Gesture::None {
            self.gesture = Gesture::TurnCamera;
            self.press_square = None;
            (self.press_x, self.press_y) = (self.pointer.x, self.pointer.y);
            (self.drag_x, self.drag_y) = (self.pointer.x, self.pointer.y);
        }

        let travel = (self.pointer.x - self.press_x).abs() + (self.pointer.y - self.press_y).abs();
        if self.gesture == Gesture::PressPiece && travel > 6.0 * scale && self.pointer.left_down {
            self.gesture = Gesture::DragPiece;
        }
        if let (Gesture::DragPiece, Some(held)) = (self.gesture, self.press_square) {
            self.view.hold(held, self.pointer_on_plane(0.3) + vec3(0.0, 0.05, 0.0));
        }
        if self.gesture == Gesture::TurnCamera {
            self.camera.rotate(-(self.pointer.x - self.drag_x) * 0.0055 / scale, (self.pointer.y - self.drag_y) * 0.0045 / scale);
            (self.drag_x, self.drag_y) = (self.pointer.x, self.pointer.y);
        }

        if self.pointer.left_released || self.pointer.right_released {
            if self.gesture == Gesture::DragPiece && self.pointer.left_released {
                let square = self.square_under_pointer();
                if let Some(target) = square.filter(|_| self.is_target(square)) {
                    self.choose_square(target);
                } else if let Some(held) = self.press_square {
                    self.view.release(held);
                }
            } else if self.gesture == Gesture::TurnCamera && travel < 4.0 * scale && self.pointer.left_released {
                self.clear_selection(); // a click beside the pieces
            }
            let left_ends = self.pointer.left_released && self.gesture != Gesture::None && !(self.gesture == Gesture::TurnCamera && self.pointer.right_down);
            let right_ends = self.pointer.right_released && self.gesture == Gesture::TurnCamera && !self.pointer.left_down;
            if left_ends || right_ends {
                self.gesture = Gesture::None;
            }
        }

        if !self.pointer.consumed && self.pointer.wheel != 0.0 {
            self.camera.zoom(self.pointer.wheel);
        }
    }

    fn handle_key(&mut self, key: Key) {
        let control = self.modifiers.control_key();
        let shift = self.modifiers.shift_key();
        let step = if shift { -1 } else { 1 };

        if self.help_visible && matches!(key, Key::Escape | Key::F1) {
            self.help_visible = false;
            return;
        }

        if control {
            match key {
                Key::Character('Z') if shift => self.redo(),
                Key::Character('Z') => self.undo(),
                Key::Character('Y') => self.redo(),
                Key::Character('C') => self.copy_game(shift),
                Key::Character('V') => self.paste_game(),
                Key::Character('N') => self.new_game(),
                _ => {}
            }
            return;
        }

        match key {
            Key::Backspace | Key::Left => self.undo(),
            Key::Right => self.redo(),
            Key::Character('N') => self.new_game(),
            Key::Character('H') => self.request_hint(),
            Key::Character('F') => self.camera.flip(),
            Key::Character('V') => self.camera.toggle_top_view(),
            Key::Home => self.camera.reset(self.view_side()),
            Key::Character('M') => self.set_mode(self.mode.stepped(1)),
            Key::Character('R') => self.step_variant(step),
            Key::Character('T') => self.set_theme(self.theme as i32 + step),
            Key::Character('S') => self.toggle_sound(),
            Key::Character('L') => self.toggle_language(),
            Key::Tab => {
                self.panel_visible = !self.panel_visible;
                self.save();
            }
            Key::F1 => self.help_visible = !self.help_visible,
            Key::F11 => self.toggle_full_screen(),
            Key::Escape if self.full_screen => self.toggle_full_screen(),
            Key::Escape => {
                // A step back along the route being chosen, or the piece put down.
                if self.route.pop().is_none() {
                    self.clear_selection();
                }
            }
            Key::Character(digit @ '1'..='5') => self.set_level(digit as i32 - '0' as i32),
            _ => {}
        }
    }

    // The interface ----------------------------------------------------------------------------------

    /// Whether a dialog is open over everything else.
    fn modal(&self) -> bool {
        self.help_visible
    }

    fn help_card(&self) -> Rect {
        let scale = self.scale;
        let (width, height) = (680.0 * scale, (24.0 * HELP_ROW + 2.0 * HELP_PAD + 52.0 + 12.0) * scale);
        let free = self.free_area();
        rect(free.x + (free.width - width) / 2.0, (10.0 * scale).max((self.height as f32 - height) / 2.0), width, height)
    }

    fn banner_card(&self) -> Rect {
        let scale = self.scale;
        let title = self.t(result_title(self.game.status().result));
        let width = (self.painter.measure(FontId::Display, title) + 80.0 * scale).max(320.0 * scale);
        let free = self.free_area();
        rect(free.x + (free.width - width) / 2.0, 26.0 * scale - (1.0 - self.banner_fade) * 20.0 * scale, width, 92.0 * scale)
    }

    fn frosted_panels(&self) -> Vec<FrostedPanel> {
        let mut panels = Vec::new();
        if self.panel_slide > 0.001 {
            panels.push(self.frosted(self.panel_rect(), 18.0, 1.0));
        }
        if self.banner_fade > 0.001 {
            panels.push(self.frosted(self.banner_card(), 18.0, self.banner_fade));
        }
        if self.help_visible {
            panels.push(self.frosted(self.help_card(), 20.0, 1.0));
        }
        panels
    }

    fn player_name(&self, side: Color) -> String {
        if self.mode == Mode::TwoPlayers {
            self.t(Str::Player).to_owned()
        } else if self.is_human(side) {
            self.t(Str::You).to_owned()
        } else {
            format!("{} · {}", self.t(Str::Computer), self.t(Str::level(self.level)))
        }
    }

    fn draw_interface(&mut self) {
        self.draw_panel();
        self.draw_banner();
        self.draw_toasts();
        if self.help_visible {
            self.draw_help();
        }
    }

    fn draw_status(&mut self, area: Rect) {
        let scale = self.scale;
        let status = self.game.status();
        let side = self.game.current().side_to_move();
        let accent = self.accent();
        let thinking = self.computer_thinking || self.computer_result.is_some();
        self.painter.fill(area, 12.0 * scale, rgba(255, 255, 255, 14));

        // A disc of the side to move.
        let (disc_x, disc_y) = (area.x + 22.0 * scale, area.y + area.height / 2.0);
        self.painter.disc(disc_x, disc_y, 10.0 * scale, DISC_RIM);
        self.painter.disc(disc_x, disc_y, 9.0 * scale, disc_color(side));

        let (title, detail, title_color, detail_color) = if status.over() {
            (self.t(result_title(status.result)), self.t(reason_text(status, self.game.variant())).to_owned(), accent, MUTED_COLOR)
        } else {
            let title = self.t(if side == Color::White { Str::WhiteToMove } else { Str::BlackToMove });
            if thinking {
                (title, self.t(Str::Thinking).to_owned(), TEXT_COLOR, MUTED_COLOR)
            } else if self.human_to_move() && self.game.current().has_capture(self.game.rules()) {
                (title, self.t(Str::MustTake).to_owned(), TEXT_COLOR, ui::Color::from_linear(self.theme().accent_warm, 1.0))
            } else {
                (title, self.player_name(side), TEXT_COLOR, MUTED_COLOR)
            }
        };
        let text_x = area.x + 44.0 * scale;
        let line_height = self.painter.line_height(FontId::Heading);
        self.painter.text(FontId::Heading, text_x, area.y + area.height / 2.0 - line_height + 3.0 * scale, title, title_color);
        self.painter.text(FontId::Body, text_x, area.y + area.height / 2.0 + 2.0 * scale, &detail, detail_color);
        if thinking || self.hint_thinking {
            self.painter.spinner(area.right() - 22.0 * scale, area.y + area.height / 2.0, 8.0 * scale, 2.2 * scale, self.time * 5.0, accent);
        }
    }

    fn draw_move_list(&mut self, area: Rect) {
        let scale = self.scale;
        self.painter.fill(area, 12.0 * scale, rgba(0, 0, 0, 40));
        let moves: Vec<String> = self.game.moves().iter().map(|record| record.notation.clone()).collect();
        if moves.is_empty() {
            self.painter.text_centered(FontId::Body, area, self.t(Str::NoMovesYet), MUTED_COLOR);
            return;
        }

        let row_height = (25.0 * scale).round();
        let inner = area.inset(8.0 * scale);
        let visible = ((inner.height / row_height) as usize).max(1);
        // Each row starts with the side that moves first in these rules.
        let offset = usize::from(self.game.start().side_to_move() != self.game.rules().first);
        let rows = (moves.len() + offset).div_ceil(2);
        let max_scroll = rows.saturating_sub(visible) as f32;

        if area.contains(self.pointer.x, self.pointer.y) && self.pointer.wheel != 0.0 && !self.help_visible {
            self.move_scroll = (self.move_scroll - self.pointer.wheel * 3.0).clamp(0.0, max_scroll);
            self.follow_moves = self.move_scroll >= max_scroll;
            self.pointer.wheel = 0.0;
        }
        if self.follow_moves {
            self.move_scroll = max_scroll;
        }
        self.move_scroll = self.move_scroll.clamp(0.0, max_scroll);

        let first = self.move_scroll as usize;
        let number_width = 40.0 * scale;
        let column_width = (inner.width - number_width) / 2.0;
        let accent = self.accent();
        for row in first..rows.min(first + visible) {
            let y = inner.y + (row - first) as f32 * row_height;
            let number = format!("{}.", row + 1);
            let number_x = inner.x + number_width - 10.0 * scale - self.painter.measure(FontId::Body, &number);
            self.painter.text(FontId::Body, number_x, y + 3.0 * scale, &number, MUTED_COLOR);
            for column in 0..2 {
                let Some(ply) = (row * 2 + column).checked_sub(offset).filter(|ply| *ply < moves.len()) else { continue };
                let x = inner.x + number_width + column as f32 * column_width;
                if ply == moves.len() - 1 {
                    self.painter.fill(rect(x - 6.0 * scale, y + scale, column_width - 6.0 * scale, row_height - 2.0 * scale), 7.0 * scale, accent.fade(0.28));
                }
                // A long capture that does not fit shows where it began and ended.
                let mut text = moves[ply].clone();
                if self.painter.measure(FontId::Body, &text) > column_width - 12.0 * scale {
                    if let (Some(start), Some(end)) = (text.find(['-', ':', 'x']), text.rfind(['-', ':', 'x'])) {
                        text = format!("{}…{}", &text[..=start], &text[end..]);
                    }
                }
                self.painter.text(FontId::Body, x, y + 3.0 * scale, &text, TEXT_COLOR);
            }
        }
        if max_scroll > 0.0 {
            // A thin scroll bar.
            let bar_height = inner.height * visible as f32 / rows as f32;
            let bar_y = inner.y + (inner.height - bar_height) * (self.move_scroll / max_scroll);
            self.painter.fill(rect(area.right() - 5.0 * scale, bar_y, 3.0 * scale, bar_height), 1.5 * scale, rgba(255, 255, 255, 50));
        }
    }

    /// A player's row: the color and who plays it, and the men and kings left.
    fn draw_player(&mut self, side: Color, x: f32, y: f32, width: f32, row_height: f32) {
        let scale = self.scale;
        let position = *self.game.current();
        let text_y = y + (row_height - self.painter.line_height(FontId::Body)) / 2.0;
        let fill = disc_color(side);
        self.painter.disc(x + 9.0 * scale, y + row_height / 2.0, 7.0 * scale, DISC_RIM);
        self.painter.disc(x + 9.0 * scale, y + row_height / 2.0, 6.0 * scale, fill);

        // The men and the kings left, each count after a little picture of the piece: a king is two discs.
        let (men, kings) = (position.men(side).count_ones(), position.kings_of(side).count_ones());
        let mut right = x + width;
        for (number, discs) in [(kings, 2), (men, 1)] {
            if discs == 2 && number == 0 {
                continue;
            }
            let text = number.to_string();
            let text_width = self.painter.measure(FontId::Body, &text);
            self.painter.text(FontId::Body, right - text_width, text_y, &text, MUTED_COLOR);
            let (icon_x, icon_y) = (right - text_width - 11.0 * scale, y + row_height / 2.0);
            for disc in 0..discs {
                let offset = if discs == 2 { (2.5 - 5.0 * disc as f32) * scale } else { 0.0 };
                self.painter.disc(icon_x, icon_y + offset, 6.0 * scale, DISC_RIM);
                self.painter.disc(icon_x, icon_y + offset, 5.0 * scale, fill);
            }
            right = icon_x - 6.0 * scale - 12.0 * scale;
        }

        // The name takes what is left of the row.
        let name = format!("{}  ·  {}", self.t(if side == Color::White { Str::White } else { Str::Black }), self.player_name(side));
        let name = self.fitted(FontId::Body, &name, right - (x + 26.0 * scale));
        self.painter.text(FontId::Body, x + 26.0 * scale, text_y, &name, TEXT_COLOR);
    }

    /// A text cut to a width, with an ellipsis in place of what was cut.
    fn fitted(&self, font: FontId, text: &str, width: f32) -> String {
        if self.painter.measure(font, text) <= width {
            return text.to_owned();
        }
        let mut kept = text.to_owned();
        while kept.pop().is_some() {
            let shortened = format!("{}…", kept.trim_end());
            if self.painter.measure(font, &shortened) <= width {
                return shortened;
            }
        }
        String::new()
    }

    fn draw_panel(&mut self) {
        if self.panel_slide <= 0.001 {
            return;
        }
        let scale = self.scale;
        let panel = self.panel_rect();
        if panel.contains(self.pointer.x, self.pointer.y) && (self.pointer.left_pressed || self.pointer.right_pressed) {
            self.pointer.consumed = true;
        }
        self.painter.shadow(panel, 22.0 * scale, rgba(0, 0, 0, 70));
        self.painter.fill(panel, 18.0 * scale, rgba(16, 18, 26, 120));
        self.painter.outline(panel, 18.0 * scale, rgba(255, 255, 255, 24));

        let pad = 20.0 * scale;
        let (x, width) = (panel.x + pad, panel.width - 2.0 * pad);
        let mut y = panel.y + pad;

        self.painter.text(FontId::Heading, x, y, self.t(Str::Title), TEXT_COLOR);
        if self.button(HELP_BUTTON, rect(panel.right() - pad - 30.0 * scale, y - 2.0 * scale, 30.0 * scale, 30.0 * scale), "?", true, false) {
            self.help_visible = !self.help_visible;
        }
        y += 44.0 * scale;

        self.draw_status(rect(x, y, width, 62.0 * scale));
        y += 62.0 * scale + 14.0 * scale;

        // The players, with the men and kings each has left.
        for side in [Color::White, Color::Black] {
            self.draw_player(side, x, y, width, 28.0 * scale);
            y += 28.0 * scale;
        }
        y += 12.0 * scale;

        // From the bottom up: the footer, the settings and the buttons; the moves take the space between.
        let mut bottom = panel.bottom() - pad;
        let footer_height = self.painter.line_height(FontId::Body);
        self.painter.text_centered(FontId::Body, rect(x, bottom - footer_height, width, footer_height), self.t(Str::Footer), MUTED_COLOR.fade(0.8));
        bottom -= footer_height + 14.0 * scale;

        let (row_height, gap) = (34.0 * scale, 8.0 * scale);
        let half = (width - gap) / 2.0;
        bottom -= row_height;
        let sound = format!("{}: {}", self.t(Str::Sound), self.t(if self.sound.enabled { Str::On } else { Str::Off }));
        if self.button(SOUND_BUTTON, rect(x, bottom, half, row_height), &sound, true, false) {
            self.toggle_sound();
        }
        if self.button(LANGUAGE_BUTTON, rect(x + half + gap, bottom, half, row_height), self.t(Str::LanguageName), true, false) {
            self.toggle_language();
        }
        bottom -= gap;

        let theme = self.theme();
        let mode_name = [Str::ModeWhite, Str::ModeBlack, Str::ModeTwoPlayers, Str::ModeDemo][self.mode as usize];
        let choices = [
            (THEME_SELECTOR, Str::Theme, (if self.language == Language::Russian { theme.name_russian } else { theme.name_english }).to_owned()),
            (RULES_SELECTOR, Str::Rules, self.t(variant_name(self.game.variant())).to_owned()),
            (LEVEL_SELECTOR, Str::Level, format!("{} · {}", self.level, self.t(Str::level(self.level)))),
            (MODE_SELECTOR, Str::Mode, self.t(mode_name).to_owned()),
        ];
        for (id, label, value) in choices {
            bottom -= row_height;
            let change = self.selector(id, rect(x, bottom, width, row_height), self.t(label), &value);
            if change != 0 {
                match id {
                    THEME_SELECTOR => self.set_theme(self.theme as i32 + change),
                    RULES_SELECTOR => self.step_variant(change),
                    LEVEL_SELECTOR => self.set_level(self.level + change),
                    _ => self.set_mode(self.mode.stepped(change)),
                }
            }
            bottom -= 6.0 * scale;
        }
        bottom -= 8.0 * scale;

        let button_height = 38.0 * scale;
        bottom -= button_height;
        if self.button(NEW_GAME_BUTTON, rect(x, bottom, half, button_height), self.t(Str::NewGame), true, false) {
            self.new_game();
        }
        if self.button(FLIP_BUTTON, rect(x + half + gap, bottom, half, button_height), self.t(Str::Flip), true, false) {
            self.camera.flip();
        }
        bottom -= gap + button_height;
        let third = (width - 2.0 * gap) / 3.0;
        if self.button(UNDO_BUTTON, rect(x, bottom, third, button_height), self.t(Str::Undo), self.game.can_undo(), false) {
            self.undo();
        }
        if self.button(REDO_BUTTON, rect(x + third + gap, bottom, third, button_height), self.t(Str::Redo), self.game.can_redo(), false) {
            self.redo();
        }
        let can_hint = self.human_to_move() && !self.hint_thinking;
        if self.button(HINT_BUTTON, rect(x + 2.0 * (third + gap), bottom, third, button_height), self.t(Str::Hint), can_hint, true) {
            self.request_hint();
        }
        bottom -= 16.0 * scale;

        self.painter.text(FontId::Body, x, y, self.t(Str::Moves), MUTED_COLOR);
        y += self.painter.line_height(FontId::Body) + 6.0 * scale;
        if bottom - y > 40.0 * scale {
            self.draw_move_list(rect(x, y, width, bottom - y));
        }
    }

    fn draw_banner(&mut self) {
        if self.banner_fade <= 0.001 {
            return;
        }
        let scale = self.scale;
        let fade = self.banner_fade;
        let status = self.game.status();
        let card = self.banner_card();
        self.painter.shadow(card, 24.0 * scale, rgba(0, 0, 0, 120).fade(fade));
        self.painter.fill(card, 18.0 * scale, rgba(14, 16, 24, 150).fade(fade));
        self.painter.outline(card, 18.0 * scale, self.accent().fade(0.6 * fade));
        self.painter.text_centered(
            FontId::Display,
            rect(card.x, card.y + 8.0 * scale, card.width, 48.0 * scale),
            self.t(result_title(status.result)),
            TEXT_COLOR.fade(fade),
        );
        let reason = self.t(reason_text(status, self.game.variant()));
        self.painter.text_centered(FontId::Body, rect(card.x, card.y + 54.0 * scale, card.width, 26.0 * scale), reason, MUTED_COLOR.fade(fade));
    }

    fn draw_help(&mut self) {
        let scale = self.scale;
        self.painter.fill(rect(0.0, 0.0, self.width as f32, self.height as f32), 0.0, rgba(0, 0, 0, 110));

        let (row_height, keys_width, pad) = (HELP_ROW * scale, 200.0 * scale, HELP_PAD * scale);
        let card = self.help_card();
        let accent = self.accent();
        self.painter.shadow(card, 26.0 * scale, rgba(0, 0, 0, 140));
        self.painter.fill(card, 20.0 * scale, rgba(18, 20, 28, 175));
        self.painter.outline(card, 20.0 * scale, rgba(255, 255, 255, 30));
        self.painter.text(FontId::Heading, card.x + pad, card.y + pad - 4.0 * scale, self.t(Str::Controls), TEXT_COLOR);
        let mut y = card.y + pad + 44.0 * scale;
        for (named, keys, action) in HELP_LINES {
            self.painter.text(FontId::Body, card.x + pad, y, named.map_or(keys, |id| self.t(id)), accent);
            self.painter.text(FontId::Body, card.x + pad + keys_width, y, self.t(action), TEXT_COLOR);
            y += row_height;
        }

        // The rules being played, in a sentence wrapped to the card's width.
        let variant = self.game.variant();
        let note = match variant {
            Variant::Russian => Str::RulesNoteRussian,
            Variant::English => Str::RulesNoteEnglish,
            Variant::Giveaway => Str::RulesNoteGiveaway,
        };
        y += 12.0 * scale;
        let heading = format!("{}: {}", self.t(Str::Rules), self.t(variant_name(variant)));
        self.painter.text(FontId::Body, card.x + pad, y, &heading, accent);
        let mut line = String::new();
        for word in self.t(note).split(' ') {
            let longer = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if !line.is_empty() && self.painter.measure(FontId::Body, &longer) > card.width - 2.0 * pad {
                y += row_height;
                self.painter.text(FontId::Body, card.x + pad, y, &line, MUTED_COLOR);
                line = word.to_owned();
            } else {
                line = longer;
            }
        }
        self.painter.text(FontId::Body, card.x + pad, y + row_height, &line, MUTED_COLOR);

        let close = self.button(HELP_CLOSE, rect(card.right() - pad - 30.0 * scale, card.y + pad - 8.0 * scale, 30.0 * scale, 30.0 * scale), "×", true, false);
        if close || (self.pointer.left_pressed && !card.contains(self.pointer.x, self.pointer.y)) {
            self.help_visible = false;
        }
        if self.pointer.left_pressed || self.pointer.right_pressed {
            self.pointer.consumed = true;
        }
    }
}

/// The marks of the board for the squares of a move: where it starts and where it lands.
fn route_bits(played: &Move) -> u64 {
    std::iter::once(played.from()).chain(played.landings()).fold(0, |bits, square| bits | board_bit(square))
}
