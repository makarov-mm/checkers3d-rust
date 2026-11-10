//! The computer player: an alpha-beta searcher with iterative deepening, principal variation search and a
//! transposition table, and captures followed to the end, since a position with something to take is never quiet.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::draughts::{Color, Move, Position, Rules, Square, Variant, bit, file_of, is_dark, rank_of, rules_of, square_at, squares};

#[derive(Clone, Copy)]
pub struct SearchLimits {
    pub max_depth: i32,
    pub milliseconds: u64,  // iterative deepening stops when this runs out
    pub random_margin: i32, // a random move among those this close to the best, for the weaker levels
    pub seed: u64,
}

#[derive(Clone, Copy, Default)]
pub struct SearchResult {
    pub best: Move,
    pub score: i32, // for the side to move, a man being 100
    pub depth: i32,
    pub nodes: u64,
}

const INFINITY: i32 = 32000;
const WIN: i32 = 30000;
const MAX_PLY: usize = 128;

fn is_win_score(score: i32) -> bool {
    !(-WIN + 512 + 1..WIN - 512).contains(&score)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bound {
    Upper,
    Lower,
    Exact,
}

/// The value of a man by how far it has come (its rank counted from its own side).
const ADVANCE: [i32; 8] = [0, 2, 4, 7, 11, 16, 24, 0];

/// The value of a man's standing on a square, for White; Black's squares are mirrored. The middle of the board
/// is worth holding, the edges less.
#[rustfmt::skip]
const MAN_SQUARE: [i32; 32] = [
     0, 0, 0,  0, // a1 c1 e1 g1
     0, 2, 2, -2, // b2 d2 f2 h2
    -3, 4, 4,  1, // a3 c3 e3 g3
     1, 7, 6, -3, // b4 d4 f4 h4
    -3, 7, 7,  1, // a5 c5 e5 g5
     1, 5, 5, -2, // b6 d6 f6 h6
    -2, 3, 3,  1, // a7 c7 e7 g7
     0, 0, 0,  0, // b8 d8 f8 h8
];

fn back_rank(color: Color) -> u32 {
    match color {
        Color::White => 0x0000_000F,
        Color::Black => 0xF000_0000,
    }
}

/// The long diagonal a1-h8, where a flying king is strongest.
const MAIN_DIAGONAL: u32 = {
    let mut mask = 0;
    let mut i = 0;
    while i < 8 {
        mask |= bit(square_at(i, i));
        i += 1;
    }
    mask
};

/// The double diagonals beside it: g1-a7 and h2-b8.
const DOUBLE_DIAGONALS: u32 = {
    let mut mask = 0;
    let mut i = 0;
    while i < 7 {
        mask |= bit(square_at(6 - i, i)) | bit(square_at(7 - i, i + 1));
        i += 1;
    }
    mask
};

fn mirror(square: Square) -> Square {
    31 - square
}

fn count(bits: u32) -> i32 {
    bits.count_ones() as i32
}

/// The squares a king can move to, along the whole diagonals or one step along each.
fn king_mobility(square: Square, occupied: u32, flying: bool) -> i32 {
    let mut reach = 0;
    for (file_step, rank_step) in [(1, 1), (-1, 1), (1, -1), (-1, -1)] {
        let (mut file, mut rank) = (file_of(square) + file_step, rank_of(square) + rank_step);
        while is_dark(file, rank) && occupied & bit(square_at(file, rank)) == 0 {
            reach += 1;
            if !flying {
                break;
            }
            file += file_step;
            rank += rank_step;
        }
    }
    reach
}

/// The worth of a side's men and kings, and of where they stand.
fn side_score(position: &Position, color: Color, variant: Variant) -> i32 {
    let russian = variant != Variant::English;
    let (man_value, king_value) = (100, if russian { 280 } else { 145 });
    let (men, kings) = (position.men(color), position.kings_of(color));
    let occupied = position.occupied();
    let mut score = count(men) * man_value + count(kings) * king_value;

    for square in squares(men) {
        let own = if color == Color::White { square } else { mirror(square) };
        score += ADVANCE[rank_of(own) as usize] + MAN_SQUARE[own as usize];
    }
    // Men at home keep the other side from crowning, while it has no kings to come round with.
    if position.kings_of(color.opposite()) == 0 {
        score += count(men & back_rank(color)) * 6;
    }

    for square in squares(kings) {
        if russian {
            if MAIN_DIAGONAL & bit(square) != 0 {
                score += 20;
            } else if DOUBLE_DIAGONALS & bit(square) != 0 {
                score += 6;
            }
            score += 2 * king_mobility(square, occupied, true);
        } else {
            score += 4 * king_mobility(square, occupied, false) + (MAN_SQUARE[square as usize] + MAN_SQUARE[mirror(square) as usize]) / 2;
        }
    }
    score
}

/// Material and position for the side to move.
fn evaluate(position: &Position, variant: Variant) -> i32 {
    let side = position.side_to_move();
    if variant == Variant::Giveaway {
        // Fewer pieces is better; men pressing forward give the other side something to take.
        let pieces = |color: Color| {
            let advance: i32 = squares(position.men(color)).map(|square| rank_of(if color == Color::White { square } else { mirror(square) })).sum();
            count(position.pieces(color)) * 100 + count(position.kings_of(color)) * 40 - 3 * advance
        };
        return pieces(side.opposite()) - pieces(side);
    }

    let (own, other) = (side_score(position, side, variant), side_score(position, side.opposite(), variant));
    // When ahead, trading down brings the win closer.
    let pieces = count(position.occupied());
    (own - other) + (own - other) * (24 - pieces) / 40
}

/// A move in short, for the table and the killers: its squares and the number of pieces it takes.
fn code(played: &Move) -> u16 {
    (0x8000 | i32::from(played.from) | (played.to() << 5) | (played.capture_count().min(15) << 10)) as u16
}

#[derive(Clone, Copy)]
struct Entry {
    key: u64,
    score: i16,
    depth: i8,
    bound: Option<Bound>,
    best: u16, // the best move's code
}

const EMPTY_ENTRY: Entry = Entry { key: 0, score: 0, depth: 0, bound: None, best: 0 };

/// The moves of a ply and their order, kept between the nodes of the ply to spare the allocations.
#[derive(Default)]
struct Scratch {
    moves: Vec<Move>,
    scores: Vec<i32>,
}

#[derive(Clone, Copy)]
struct RootMove {
    played: Move,
    score: i32,
}

/// Seeds scrambled into a stream of random numbers (SplitMix64).
struct Random(u64);

impl Random {
    /// From 0 up to 1.
    fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
}

pub struct Engine {
    table: Vec<Entry>,
    scratch: Vec<Scratch>, // one for each ply
    keys: Vec<u64>,        // the game's and the search's, for repetitions
    killers: [[u16; 2]; MAX_PLY],
    history: [[i32; 32]; 32],
    variant: Variant,
    rules: &'static Rules,
    root_depth: i32,
    nodes: u64,
    deadline: Instant,
    stop: Arc<AtomicBool>,
    aborted: bool,
}

impl Engine {
    pub fn new(hash_megabytes: usize) -> Engine {
        let mut entries = 1;
        while entries * 2 * size_of::<Entry>() <= hash_megabytes * 1024 * 1024 {
            entries *= 2;
        }
        Engine {
            table: vec![EMPTY_ENTRY; entries],
            scratch: std::iter::repeat_with(Scratch::default).take(MAX_PLY + 1).collect(),
            keys: Vec::new(),
            killers: [[0; 2]; MAX_PLY],
            history: [[0; 32]; 32],
            variant: Variant::Russian,
            rules: rules_of(Variant::Russian),
            root_depth: 0,
            nodes: 0,
            deadline: Instant::now(),
            stop: Arc::default(),
            aborted: false,
        }
    }

    pub fn clear(&mut self) {
        self.table.fill(EMPTY_ENTRY);
        self.killers = [[0; 2]; MAX_PLY];
        self.history = [[0; 32]; 32];
    }

    fn should_stop(&self) -> bool {
        self.stop.load(Ordering::Relaxed) || Instant::now() >= self.deadline
    }

    fn is_repetition(&self, position: &Position) -> bool {
        // Only king moves without captures can come back to a position, and only as far back as they reach.
        let last = self.keys.len() - 1;
        let reach = usize::from(position.quiet_plies()).min(last);
        (2..=reach).step_by(2).any(|back| self.keys[last - back] == position.key())
    }

    fn score_move(&self, position: &Position, played: &Move, ply: usize) -> i32 {
        if played.is_capture() {
            return 100_000 + 1000 * played.capture_count() + 400 * count(played.captured & position.kings()) + if played.crowns { 300 } else { 0 };
        }
        if played.crowns {
            return 90_000;
        }
        let code = code(played);
        if code == self.killers[ply][0] {
            return 80_000;
        }
        if code == self.killers[ply][1] {
            return 79_000;
        }
        self.history[usize::from(played.from)][played.to() as usize].min(70_000)
    }

    fn slot(&self, key: u64) -> usize {
        key as usize & (self.table.len() - 1)
    }

    fn store(&mut self, key: u64, depth: i32, mut score: i32, bound: Bound, best: u16, ply: usize) {
        // A win is kept as its distance from this position, not from the root.
        if score >= WIN - MAX_PLY as i32 {
            score += ply as i32;
        } else if score <= -WIN + MAX_PLY as i32 {
            score -= ply as i32;
        }
        let slot = self.slot(key);
        let entry = &mut self.table[slot];
        if entry.key == key && i32::from(entry.depth) > depth && bound != Bound::Exact {
            return;
        }
        *entry = Entry { key, score: score as i16, depth: depth.clamp(-100, 100) as i8, bound: Some(bound), best };
    }

    fn negamax(&mut self, position: &Position, depth: i32, alpha: i32, beta: i32, ply: usize) -> i32 {
        self.nodes += 1;
        if self.nodes & 2047 == 0 && self.should_stop() {
            self.aborted = true;
        }
        if self.aborted {
            return 0;
        }
        if ply > 0 && (position.quiet_plies() >= self.rules.quiet_limit || self.is_repetition(position)) {
            return 0;
        }

        // The moves of this ply are worked on outside the engine, which the plies below borrow whole.
        let mut scratch = std::mem::take(&mut self.scratch[ply]);
        position.generate_moves(self.rules, &mut scratch.moves);
        let score = self.search_moves(position, depth, alpha, beta, ply, &mut scratch);
        self.scratch[ply] = scratch;
        score
    }

    fn search_moves(&mut self, position: &Position, depth: i32, mut alpha: i32, beta: i32, ply: usize, scratch: &mut Scratch) -> i32 {
        let rules = self.rules;
        let Scratch { moves, scores } = scratch;
        if moves.is_empty() {
            let lost = -(WIN - ply as i32);
            return if rules.giveaway { -lost } else { lost };
        }
        let capture = moves[0].is_capture();
        if rules.lone_kings_draw && !capture && position.lone_kings() {
            return 0;
        }
        // A position with something to take is never left to the evaluation: the captures are followed to the end.
        if (depth <= 0 && !capture) || ply >= MAX_PLY - 1 {
            return evaluate(position, self.variant);
        }

        let key = position.key();
        let entry = self.table[self.slot(key)];
        let mut table_move = 0;
        if entry.key == key {
            table_move = entry.best;
            if ply > 0 && i32::from(entry.depth) >= depth {
                let mut score = i32::from(entry.score);
                if score >= WIN - MAX_PLY as i32 {
                    score -= ply as i32;
                } else if score <= -WIN + MAX_PLY as i32 {
                    score += ply as i32;
                }
                match entry.bound {
                    Some(Bound::Exact) => return score,
                    Some(Bound::Lower) if score >= beta => return score,
                    Some(Bound::Upper) if score <= alpha => return score,
                    _ => {}
                }
            }
        }

        // Best first: the move that was best here before, then captures, crownings, killers and history.
        scores.clear();
        scores.extend(moves.iter().map(|candidate| if code(candidate) == table_move { 1_000_000 } else { self.score_move(position, candidate, ply) }));
        for i in 1..moves.len() {
            let mut j = i;
            while j > 0 && scores[j] > scores[j - 1] {
                scores.swap(j, j - 1);
                moves.swap(j, j - 1);
                j -= 1;
            }
        }

        // A forced move costs nothing: it is searched as deep as the position before it.
        let forced = moves.len() == 1 && (ply as i32) < 2 * self.root_depth + 8;
        let new_depth = depth - i32::from(!forced);
        let alpha_before = alpha;
        let (mut best, mut best_code) = (-INFINITY, 0);
        for (i, played) in moves.iter().enumerate() {
            let next = position.after(played);
            self.keys.push(next.key());
            let mut score;
            if i == 0 {
                score = -self.negamax(&next, new_depth, -beta, -alpha, ply + 1);
            } else {
                // Late quiet moves are first searched a little less deeply.
                let reduction = i32::from(!capture && !played.crowns && i >= 3 && depth >= 3);
                score = -self.negamax(&next, new_depth - reduction, -alpha - 1, -alpha, ply + 1);
                if score > alpha && (reduction > 0 || score < beta) {
                    score = -self.negamax(&next, new_depth, -beta, -alpha, ply + 1);
                }
            }
            self.keys.pop();
            if self.aborted {
                return 0;
            }

            if score > best {
                best = score;
                best_code = code(played);
            }
            if score > alpha {
                alpha = score;
                if alpha >= beta {
                    if !played.is_capture() {
                        let killers = &mut self.killers[ply];
                        if killers[0] != best_code {
                            *killers = [best_code, killers[0]];
                        }
                        self.history[usize::from(played.from)][played.to() as usize] += depth * depth;
                    }
                    break;
                }
            }
        }
        let bound = if best >= beta {
            Bound::Lower
        } else if best > alpha_before {
            Bound::Exact
        } else {
            Bound::Upper
        };
        self.store(key, depth, best, bound, best_code, ply);
        best
    }

    /// The best move of a position; `history` holds the keys of the earlier positions that could come back.
    pub fn search(&mut self, root: &Position, variant: Variant, history: &[u64], limits: &SearchLimits, stop: &Arc<AtomicBool>) -> SearchResult {
        let start = Instant::now();
        let time = Duration::from_millis(limits.milliseconds);
        self.deadline = start + time;
        self.stop = stop.clone();
        self.aborted = false;
        self.nodes = 0;
        if variant != self.variant {
            self.clear(); // what was found under other rules does not hold under these
        }
        self.variant = variant;
        self.rules = rules_of(variant);
        self.keys.clear();
        self.keys.extend_from_slice(history);
        self.keys.push(root.key());
        self.killers = [[0; 2]; MAX_PLY];
        self.history.as_flattened_mut().iter_mut().for_each(|value| *value /= 8);

        let mut root_moves: Vec<RootMove> = root.legal_moves(self.rules).into_iter().map(|played| RootMove { played, score: -INFINITY }).collect();
        if root_moves.is_empty() {
            return SearchResult::default();
        }
        root_moves.sort_by_cached_key(|candidate| -self.score_move(root, &candidate.played, 0));

        let mut result = SearchResult { best: root_moves[0].played, ..SearchResult::default() };
        if root_moves.len() == 1 && limits.random_margin == 0 {
            return result;
        }

        let exact_scores = limits.random_margin > 0;
        for depth in 1..=limits.max_depth {
            self.root_depth = depth;
            let mut alpha = -INFINITY;
            let beta = INFINITY;
            let (mut iteration_best, mut iteration_score) = (Move::default(), -INFINITY);
            let mut complete = true;

            for (i, candidate) in root_moves.iter_mut().enumerate() {
                let next = root.after(&candidate.played);
                self.keys.push(next.key());
                let mut score;
                if i == 0 || exact_scores {
                    score = -self.negamax(&next, depth - 1, -beta, if exact_scores { INFINITY } else { -alpha }, 1);
                } else {
                    score = -self.negamax(&next, depth - 1, -alpha - 1, -alpha, 1);
                    if !self.aborted && score > alpha {
                        score = -self.negamax(&next, depth - 1, -beta, -alpha, 1);
                    }
                }
                self.keys.pop();
                if self.aborted {
                    complete = false;
                    break;
                }
                candidate.score = score;
                if score > iteration_score {
                    iteration_score = score;
                    iteration_best = candidate.played;
                }
                alpha = alpha.max(score);
            }

            // An unfinished iteration still counts if a move beat the previous best, which it searched first.
            if !iteration_best.is_null() && (complete || iteration_score > result.score || result.depth == 0) {
                result.best = iteration_best;
                result.score = iteration_score;
            }
            if !complete {
                break;
            }
            result.depth = depth;

            root_moves.sort_by_key(|candidate| -candidate.score);
            if start.elapsed() > time / 2 || is_win_score(iteration_score) {
                break;
            }
        }

        if exact_scores && result.depth > 0 {
            // The weaker levels pick among the good moves, the better ones more often.
            let (top, margin) = (root_moves[0].score, limits.random_margin);
            let candidates: Vec<RootMove> = root_moves.iter().copied().filter(|candidate| candidate.score >= top - margin).collect();
            let weight = |candidate: &RootMove| 1.0 + f64::from(margin - (top - candidate.score)) / f64::from(margin.max(1)) * 3.0;
            let mut pick = Random(limits.seed ^ root.key()).unit() * candidates.iter().map(weight).sum::<f64>();
            let chosen = candidates
                .iter()
                .find(|candidate| {
                    pick -= weight(candidate);
                    pick < 0.0
                })
                .unwrap_or(&candidates[candidates.len() - 1]);
            result.best = chosen.played;
            result.score = chosen.score;
        }
        result.nodes = self.nodes;
        result
    }
}

/// Runs an engine on a thread of its own; the window polls for the result.
pub struct AiPlayer {
    engine: Arc<Mutex<Engine>>,
    worker: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    result: Arc<Mutex<Option<SearchResult>>>,
    busy: Arc<AtomicBool>,
}

impl AiPlayer {
    pub fn new(hash_megabytes: usize) -> AiPlayer {
        AiPlayer { engine: Arc::new(Mutex::new(Engine::new(hash_megabytes))), worker: None, stop: Arc::default(), result: Arc::default(), busy: Arc::default() }
    }

    pub fn start(&mut self, position: Position, variant: Variant, history: Vec<u64>, limits: SearchLimits) {
        self.cancel();
        self.busy.store(true, Ordering::Relaxed);
        self.stop = Arc::default();
        let (engine, stop, result, busy) = (self.engine.clone(), self.stop.clone(), self.result.clone(), self.busy.clone());
        self.worker = Some(std::thread::spawn(move || {
            let found = engine.lock().unwrap_or_else(PoisonError::into_inner).search(&position, variant, &history, &limits, &stop);
            if !stop.load(Ordering::Relaxed) {
                *result.lock().unwrap_or_else(PoisonError::into_inner) = Some(found);
            }
            busy.store(false, Ordering::Relaxed);
        }));
    }

    pub fn cancel(&mut self) {
        if let Some(worker) = self.worker.take() {
            self.stop.store(true, Ordering::Relaxed);
            let _ = worker.join();
        }
        self.busy.store(false, Ordering::Relaxed);
        self.take_result();
    }

    pub fn new_game(&mut self) {
        self.cancel();
        self.engine.lock().unwrap_or_else(PoisonError::into_inner).clear();
    }

    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::Relaxed)
    }

    pub fn take_result(&mut self) -> Option<SearchResult> {
        self.result.lock().unwrap_or_else(PoisonError::into_inner).take()
    }
}

impl Drop for AiPlayer {
    fn drop(&mut self) {
        self.cancel();
    }
}
