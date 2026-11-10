//! The pieces as they stand on the board and as they move: gliding, hopping over the pieces they take,
//! stacking up when crowned, the pieces taken flying to piles beside the board, and everything dissolving
//! in when a game is set out.

use crate::board_geometry::{FRAME_OUTER, TABLE_HEIGHT, square_center};
use crate::draughts::{self, Color, Game, MoveRecord, Position, SQUARE_COUNT, Square};
use crate::math::{Mat4, PI, Vec3, smooth_step, vec3};
use crate::piece_models::{HEIGHT, RADIUS};
use crate::renderer::{ContactShadow, PieceDraw};

/// What happened in a frame, for the sounds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Step,
    Jump,
    Crown,
    Pile,
}

const CROWN_DROP: f32 = 0.9; // the height the second disc of a king falls from
const CROWN_TIME: f32 = 0.45;
const FADE_TIME: f32 = 0.45;
const PILE_HEIGHT: usize = 4; // discs
const DANGER: Vec3 = vec3(1.0, 0.28, 0.1); // the glow of a piece about to be taken

fn ease(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t)
}

fn approach(value: f32, target: f32, step: f32) -> f32 {
    if value < target { (value + step).min(target) } else { (value - step).max(target) }
}

fn ground_at(position: Vec3) -> f32 {
    if position.x.abs() < FRAME_OUTER && position.z.abs() < FRAME_OUTER { 0.0 } else { TABLE_HEIGHT }
}

fn hop_duration(distance: f32, jumping: bool) -> f32 {
    if jumping { 0.3 + 0.035 * distance } else { (0.2 + 0.06 * distance).min(0.6) }
}

fn route_duration(mut start: Vec3, points: &[Vec3], jumping: bool) -> f32 {
    let mut total = 0.0;
    for point in points {
        total += hop_duration((*point - start).length(), jumping);
        start = *point;
    }
    total
}

/// Where a piece is in the game.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Square(Square),
    Table(usize), // taken, with its place among the pieces of its color taken
    Leaving,      // dissolving, when a new game is set out
}

struct Piece {
    color: Color,
    king: bool,
    place: Place,
    yaw: f32, // each piece turned its own way, so that the grain differs
    seed: f32,
    position: Vec3,
    // Where it is going: hop after hop through the points of the route.
    route: Vec<Vec3>,
    hop: usize,
    hop_time: f32,
    hop_start: Vec3,
    jumping: bool, // over pieces, rather than gliding
    landing: Event,
    delay: f32,
    // The second disc of a king, settled on top (1) or gone (0).
    crown: f32,
    crown_target: f32,
    crown_delay: f32,
    dissolve: f32,
    dissolve_target: f32,
    dissolve_delay: f32,
    // A glow when it is jumped.
    flash: f32,
    flash_delay: Option<f32>,
    held: bool,
    remove_when_gone: bool,
}

impl Piece {
    fn send(&mut self, points: &[Vec3], jumping: bool, delay: f32, landing: Event) {
        self.route = points.to_vec();
        self.hop = 0;
        self.hop_time = 0.0;
        self.jumping = jumping;
        self.delay = delay;
        self.landing = landing;
        self.held = false;
    }

    fn square(&self) -> Option<Square> {
        match self.place {
            Place::Square(square) => Some(square),
            _ => None,
        }
    }
}

pub struct BoardView {
    pieces: Vec<Piece>,
    table_counts: [usize; 2],
    seed: u32,
}

impl BoardView {
    pub fn new() -> BoardView {
        BoardView { pieces: Vec::new(), table_counts: [0; 2], seed: 1 }
    }

    fn random(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    fn at(&mut self, square: Square) -> Option<&mut Piece> {
        self.pieces.iter_mut().find(|piece| piece.place == Place::Square(square))
    }

    /// The pieces taken stand in piles of four discs on the right of the player who took them; a king is two discs.
    fn table_position(&self, color: Color, order: usize, king: bool) -> Vec3 {
        let mut earlier: Vec<(usize, bool)> = self
            .pieces
            .iter()
            .filter(|piece| piece.color == color)
            .filter_map(|piece| match piece.place {
                Place::Table(place) if place < order => Some((place, piece.king)),
                _ => None,
            })
            .collect();
        earlier.sort_unstable();

        let (mut pile, mut height) = (0, 0);
        for is_king in earlier.into_iter().map(|(_, is_king)| is_king).chain([king]) {
            let size = if is_king { 2 } else { 1 };
            if height + size > PILE_HEIGHT {
                pile += 1;
                height = 0;
            }
            height += size;
        }
        let y = TABLE_HEIGHT + (height - if king { 2 } else { 1 }) as f32 * HEIGHT;
        let step = pile as f32;
        if color == Color::Black { vec3(5.3, y, 1.6 - step) } else { vec3(-5.3, y, -1.6 + step) }
    }

    /// A piece standing still at a place; one that appears dissolves in after the delay.
    fn add(&mut self, color: Color, king: bool, place: Place, position: Vec3, appear: Option<f32>) {
        let yaw = self.random() * 2.0 * PI;
        let seed = self.random();
        let crown = if king { 1.0 } else { 0.0 };
        self.pieces.push(Piece {
            color,
            king,
            place,
            yaw,
            seed,
            position,
            route: Vec::new(),
            hop: 0,
            hop_time: 0.0,
            hop_start: position,
            jumping: false,
            landing: Event::Step,
            delay: 0.0,
            crown,
            crown_target: crown,
            crown_delay: 0.0,
            dissolve: if appear.is_some() { 1.0 } else { 0.0 },
            dissolve_target: 0.0,
            dissolve_delay: appear.unwrap_or(0.0),
            flash: 0.0,
            flash_delay: None,
            held: false,
            remove_when_gone: false,
        });
    }

    fn put_on_table(&mut self, color: Color, king: bool, appear: Option<f32>) {
        let order = self.table_counts[color as usize];
        self.table_counts[color as usize] += 1;
        let position = self.table_position(color, order, king);
        self.add(color, king, Place::Table(order), position, appear);
    }

    /// Sets out the game's position and the pieces taken so far.
    pub fn rebuild(&mut self, game: &Game, animate: bool) {
        if animate {
            for piece in &mut self.pieces {
                piece.dissolve_target = 1.0;
                piece.dissolve_delay = 0.0;
                piece.remove_when_gone = true;
                piece.place = Place::Leaving;
            }
        } else {
            self.pieces.clear();
        }
        self.table_counts = [0; 2];

        let position = game.current();
        for square in 0..SQUARE_COUNT {
            if let Some(piece) = position.at(square) {
                self.add(piece.color, piece.king, Place::Square(square), square_center(square), animate.then_some(0.3 + 0.02 * square as f32));
            }
        }
        // The pieces taken so far, in the order they were taken.
        for (ply, record) in game.moves().iter().enumerate() {
            let taken = game.position_at(ply).side_to_move().opposite();
            for victim in &record.played.taken[..if record.played.is_capture() { usize::from(record.played.steps) } else { 0 }] {
                self.put_on_table(taken, record.taken_kings & draughts::bit(Square::from(*victim)) != 0, animate.then_some(0.5));
            }
        }
    }

    /// A move played (forward) or taken back; `before` is the position before the move either way.
    pub fn play_move(&mut self, before: &Position, record: &MoveRecord, forward: bool) {
        let played = &record.played;
        let capture = played.is_capture();
        let victims = before.side_to_move().opposite();
        let from = square_center(played.from());
        let points: Vec<Vec3> = played.landings().map(square_center).collect();
        let victim_squares: Vec<Square> = if capture { played.taken[..points.len()].iter().map(|square| Square::from(*square)).collect() } else { Vec::new() };
        let travel = route_duration(from, &points, capture);

        if forward {
            // Each piece taken glows as it is jumped, and leaves for the table when the move is over.
            let (mut time, mut at) = (0.0, from);
            let mut taken = 0;
            for (point, victim) in points.iter().zip(&victim_squares) {
                let hop = hop_duration((*point - at).length(), true);
                if let Some(index) = self.pieces.iter().position(|piece| piece.place == Place::Square(*victim)) {
                    let order = self.table_counts[victims as usize];
                    self.table_counts[victims as usize] += 1;
                    self.pieces[index].place = Place::Table(order);
                    let place = self.table_position(victims, order, self.pieces[index].king);
                    let piece = &mut self.pieces[index];
                    piece.flash_delay = Some(time + hop * 0.5);
                    piece.send(&[place], true, travel + 0.15 + 0.12 * taken as f32, Event::Pile);
                    taken += 1;
                }
                time += hop;
                at = *point;
            }
            if let Some(piece) = self.at(played.from()) {
                piece.place = Place::Square(played.to());
                piece.send(&points, capture, 0.0, if capture { Event::Jump } else { Event::Step });
                if played.crowns {
                    piece.king = true;
                    piece.crown_target = 1.0;
                    piece.crown_delay = travel + 0.05;
                }
            }
            return;
        }

        // Back: the crown comes off first, the piece retraces its route, and the pieces it took come back.
        let mut delay = 0.0;
        if let Some(piece) = self.at(played.to()) {
            if played.crowns {
                piece.king = false;
                piece.crown_target = 0.0;
                piece.crown_delay = 0.0;
                delay = CROWN_TIME * 0.8;
            }
            let back: Vec<Vec3> = points.iter().rev().skip(1).copied().chain([from]).collect();
            piece.place = Place::Square(played.from());
            piece.send(&back, capture, delay, if capture { Event::Jump } else { Event::Step });
        }
        if capture {
            let count = victim_squares.len();
            let base = self.table_counts[victims as usize].saturating_sub(count);
            for (k, victim) in victim_squares.iter().enumerate() {
                let returning = self.pieces.iter_mut().find(|piece| piece.color == victims && piece.place == Place::Table(base + k));
                if let Some(piece) = returning {
                    piece.place = Place::Square(*victim);
                    piece.send(&[square_center(*victim)], true, delay + 0.1 + 0.1 * (count - 1 - k) as f32, Event::Step);
                }
            }
            self.table_counts[victims as usize] = base;
        }
    }

    pub fn update(&mut self, seconds: f32, events: &mut Vec<Event>) {
        for piece in &mut self.pieces {
            if let Some(delay) = piece.flash_delay {
                piece.flash_delay = Some(delay - seconds).filter(|delay| *delay >= 0.0);
                if piece.flash_delay.is_none() {
                    piece.flash = 1.4;
                }
            }
            piece.flash = (piece.flash - seconds * 1.5).max(0.0);
            if piece.dissolve_delay > 0.0 {
                piece.dissolve_delay -= seconds;
            } else {
                piece.dissolve = approach(piece.dissolve, piece.dissolve_target, seconds / FADE_TIME);
            }
            if piece.crown_delay > 0.0 {
                piece.crown_delay -= seconds;
            } else if piece.crown != piece.crown_target {
                let before = piece.crown;
                piece.crown = approach(piece.crown, piece.crown_target, seconds / CROWN_TIME);
                if before < 1.0 && piece.crown >= 1.0 {
                    events.push(Event::Crown);
                }
            }

            if piece.held {
                continue;
            }
            if piece.delay > 0.0 {
                piece.delay -= seconds;
                continue;
            }
            if let Some(&target) = piece.route.get(piece.hop) {
                if piece.hop_time == 0.0 {
                    piece.hop_start = piece.position;
                }
                let distance = (target - piece.hop_start).length();
                let duration = hop_duration(distance, piece.jumping);
                piece.hop_time = (piece.hop_time + seconds).min(duration);
                let t = piece.hop_time / duration;
                let arc = if piece.jumping { 0.3 + 0.03 * distance } else { 0.025 + 0.012 * distance };
                piece.position = piece.hop_start.lerp(target, ease(t)) + vec3(0.0, arc * (PI * t).sin(), 0.0);
                if t >= 1.0 {
                    piece.position = target;
                    piece.hop += 1;
                    piece.hop_time = 0.0;
                    events.push(piece.landing);
                }
            }
        }
        self.pieces.retain(|piece| !(piece.remove_when_gone && piece.dissolve >= 1.0));
    }

    pub fn animating(&self) -> bool {
        self.pieces.iter().any(|piece| {
            piece.delay > 0.0
                || piece.hop < piece.route.len()
                || piece.crown != piece.crown_target
                || piece.dissolve != piece.dissolve_target
                || piece.flash_delay.is_some()
                || piece.flash > 0.0
        })
    }

    /// A piece held by the mouse.
    pub fn hold(&mut self, square: Square, position: Vec3) {
        if let Some(piece) = self.at(square) {
            piece.held = true;
            piece.route.clear();
            piece.hop = 0;
            piece.position = position;
        }
    }

    /// A piece let go again: it settles on its square.
    pub fn release(&mut self, square: Square) {
        if let Some(piece) = self.at(square) {
            piece.send(&[square_center(square)], false, 0.0, Event::Step);
        }
    }

    /// The square of the piece a ray hits first.
    pub fn pick(&self, origin: Vec3, direction: Vec3) -> Option<Square> {
        let mut best = None;
        let mut nearest = 1e9;
        for piece in &self.pieces {
            let Some(square) = piece.square().filter(|_| piece.dissolve <= 0.5) else { continue };
            let radius = RADIUS * 0.95;
            let height = HEIGHT * if piece.crown > 0.5 { 2.0 } else { 1.0 };
            let o = origin - piece.position;
            // The upright cylinder of the piece: its side, then its top.
            let a = direction.x * direction.x + direction.z * direction.z;
            let b = 2.0 * (o.x * direction.x + o.z * direction.z);
            let c = o.x * o.x + o.z * o.z - radius * radius;
            let mut hit = 1e9f32;
            let discriminant = b * b - 4.0 * a * c;
            if a > 1e-8 && discriminant >= 0.0 {
                let t = (-b - discriminant.sqrt()) / (2.0 * a);
                let y = o.y + direction.y * t;
                if t > 0.0 && (0.0..=height).contains(&y) {
                    hit = t;
                }
            }
            if direction.y.abs() > 1e-6 {
                let t = (height - o.y) / direction.y;
                let (x, z) = (o.x + direction.x * t, o.z + direction.z * t);
                if t > 0.0 && x * x + z * z <= radius * radius {
                    hit = hit.min(t);
                }
            }
            if hit < nearest {
                nearest = hit;
                best = Some(square);
            }
        }
        best
    }

    /// The discs to draw; `threatened` is the squares of pieces that a move under consideration would take.
    pub fn draws(&self, selected: Option<Square>, hovered: Option<Square>, threatened: u32, accent: Vec3, time: f32) -> Vec<PieceDraw> {
        let mut draws = Vec::with_capacity(self.pieces.len() * 2);
        for piece in &self.pieces {
            let square = piece.square();
            let mut draw = PieceDraw {
                color: piece.color,
                model: Mat4::translation(piece.position) * Mat4::rotation_y(piece.yaw),
                ground: ground_at(piece.position),
                glow_color: accent,
                glow: 0.0,
                dissolve: piece.dissolve,
                seed: piece.seed,
                crown: 0.0,
            };
            if square.is_some() && square == selected {
                draw.glow = 0.8 + 0.2 * (time * 4.0).sin();
            } else if square.is_some() && square == hovered {
                draw.glow = 0.35;
            }
            if square.is_some_and(|square| threatened & draughts::bit(square) != 0) {
                draw.glow_color = DANGER;
                draw.glow = 0.5 + 0.15 * (time * 5.0).sin();
            }
            if piece.flash > 0.0 {
                draw.glow_color = DANGER;
                draw.glow = draw.glow.max(piece.flash.min(1.0) * 1.3);
            }
            draws.push(draw);

            if piece.crown > 0.0 {
                // The second disc of a king, with the crown on top, turned to face its owner.
                let drop = (1.0 - ease_out(piece.crown)) * CROWN_DROP;
                let facing = if piece.color == Color::White { 0.0 } else { PI };
                draws.push(PieceDraw {
                    model: Mat4::translation(piece.position + vec3(0.0, HEIGHT + drop, 0.0)) * Mat4::rotation_y(facing),
                    dissolve: piece.dissolve.max(1.0 - piece.crown),
                    crown: 1.0,
                    seed: piece.seed + 0.37,
                    ..draw
                });
            }
        }
        draws
    }

    pub fn contacts(&self) -> Vec<ContactShadow> {
        self.pieces
            .iter()
            .filter_map(|piece| {
                let height = piece.position.y - ground_at(piece.position);
                let strength = 0.72 * (1.0 - smooth_step(0.0, 0.8, height)) * (1.0 - piece.dissolve);
                (strength > 0.01).then_some(ContactShadow { x: piece.position.x, z: piece.position.z, radius: RADIUS * (1.05 + height * 0.6), strength })
            })
            .collect()
    }
}
