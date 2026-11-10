//! The rules of draughts on the 8×8 board in three variants: Russian draughts, English draughts (checkers)
//! and giveaway. Positions, moves, notation, and a game with its history.

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub fn opposite(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Russian,
    English,
    Giveaway,
}

impl Variant {
    pub const ALL: [Variant; 3] = [Variant::Russian, Variant::English, Variant::Giveaway];

    /// The name the saved game goes by.
    fn name(self) -> &'static str {
        ["russian", "english", "giveaway"][self as usize]
    }
}

pub struct Rules {
    pub flying_kings: bool,     // kings move and take at any distance along a diagonal
    pub men_take_back: bool,    // men take backwards as well as forwards
    pub crowned_takes_on: bool, // a man crowned in the middle of a capture goes on taking, as a king
    pub first: Color,           // who moves first
    pub giveaway: bool,         // the side left without a move wins
    pub lone_kings_draw: bool,  // a king against a king is a draw
    pub quiet_limit: u16,       // plies of king moves with nothing taken that draw the game
}

const ALL_RULES: [Rules; 3] = [
    // Russian draughts: a draw after 15 moves each made with kings only, taking nothing.
    Rules { flying_kings: true, men_take_back: true, crowned_takes_on: true, first: Color::White, giveaway: false, lone_kings_draw: true, quiet_limit: 30 },
    // English draughts: Black moves first; a draw after 40 moves each without a capture or a man's move.
    Rules { flying_kings: false, men_take_back: false, crowned_takes_on: false, first: Color::Black, giveaway: false, lone_kings_draw: true, quiet_limit: 80 },
    // Giveaway, on the Russian rules: whoever runs out of moves first wins.
    Rules { flying_kings: true, men_take_back: true, crowned_takes_on: true, first: Color::White, giveaway: true, lone_kings_draw: false, quiet_limit: 30 },
];

pub fn rules_of(variant: Variant) -> &'static Rules {
    &ALL_RULES[variant as usize]
}

/// The 32 dark squares: 0 is a1, 1 c1, 2 e1, 3 g1, 4 b2 ... 31 h8.
pub type Square = i32;
pub const SQUARE_COUNT: i32 = 32;

pub const fn rank_of(square: Square) -> i32 {
    square >> 2
}

pub const fn file_of(square: Square) -> i32 {
    ((square & 3) << 1) | (rank_of(square) & 1)
}

pub const fn is_dark(file: i32, rank: i32) -> bool {
    file >= 0 && file < 8 && rank >= 0 && rank < 8 && (file + rank) & 1 == 0
}

/// The dark square at a file and rank, or -1 for a light square or one off the board.
pub const fn square_at(file: i32, rank: i32) -> Square {
    if is_dark(file, rank) { rank * 4 + file / 2 } else { -1 }
}

pub const fn bit(square: Square) -> u32 {
    1 << square
}

/// The squares of a mask, from a1 on.
pub fn squares(mut bits: u32) -> impl Iterator<Item = Square> {
    std::iter::from_fn(move || {
        (bits != 0).then(|| {
            let square = bits.trailing_zeros() as Square;
            bits &= bits - 1;
            square
        })
    })
}

/// The number a square goes by in English draughts: 1 to 32 from Black's side.
fn number_of(square: Square) -> i32 {
    (7 - rank_of(square)) * 4 + (square & 3) + 1
}

/// "c3" in Russian draughts and giveaway; in English draughts the squares are numbered, so c3 is "22".
pub fn square_name(square: Square, variant: Variant) -> String {
    if variant == Variant::English {
        return number_of(square).to_string();
    }
    format!("{}{}", char::from(b'a' + file_of(square) as u8), char::from(b'1' + rank_of(square) as u8))
}

fn parse_number(text: &str) -> Option<i32> {
    if text.is_empty() || text.len() > 3 || !text.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// A square in either form.
pub fn parse_square(text: &str) -> Option<Square> {
    if let Some(number) = parse_number(text) {
        return (1..=32).contains(&number).then(|| (7 - (number - 1) / 4) * 4 + (number - 1) % 4);
    }
    let &[file, rank] = text.as_bytes() else { return None };
    let square = square_at(i32::from(file.to_ascii_lowercase()) - i32::from(b'a'), i32::from(rank) - i32::from(b'1'));
    (square >= 0).then_some(square)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Move {
    pub from: u8,
    pub steps: u8,       // landings: 1 for a simple move or a single capture
    pub crowns: bool,    // a man that becomes a king
    pub captured: u32,   // the squares of the pieces taken
    pub path: [u8; 12],  // the landing squares in order; the last is where the piece stops
    pub taken: [u8; 12], // the pieces taken in order: taken[i] is jumped on the way to path[i]
}

impl Move {
    pub fn from(&self) -> Square {
        Square::from(self.from)
    }

    pub fn to(&self) -> Square {
        Square::from(self.path[usize::from(self.steps) - 1])
    }

    /// The landing squares in order.
    pub fn landings(&self) -> impl Iterator<Item = Square> + '_ {
        self.path[..usize::from(self.steps)].iter().map(|square| Square::from(*square))
    }

    /// The landing after as many as have been made, if the move goes that far.
    pub fn landing(&self, index: usize) -> Option<Square> {
        (index < usize::from(self.steps)).then(|| Square::from(self.path[index]))
    }

    pub fn is_null(&self) -> bool {
        self.steps == 0
    }

    pub fn is_capture(&self) -> bool {
        self.captured != 0
    }

    pub fn capture_count(&self) -> i32 {
        self.captured.count_ones() as i32
    }
}

/// Moves from the same square to the same square, taking the same pieces, come to the same thing.
impl PartialEq for Move {
    fn eq(&self, other: &Move) -> bool {
        if self.is_null() || other.is_null() {
            return self.is_null() && other.is_null();
        }
        self.from == other.from && self.to() == other.to() && self.captured == other.captured
    }
}

#[derive(Clone, Copy)]
pub struct Piece {
    pub color: Color,
    pub king: bool,
}

// The four diagonals: 0 and 1 lead towards Black (White's forward), 2 and 3 towards White.
const FILE_STEP: [i32; 4] = [1, -1, 1, -1];
const RANK_STEP: [i32; 4] = [1, 1, -1, -1];

fn is_forward(side: Color, direction: usize) -> bool {
    (side == Color::White) == (direction < 2)
}

fn crown_row(side: Color) -> u32 {
    match side {
        Color::White => 0xF000_0000,
        Color::Black => 0x0000_000F,
    }
}

/// The squares along each diagonal from each square, nearest first.
struct RayTable {
    squares: [[[u8; 7]; 4]; 32],
    length: [[u8; 4]; 32],
}

impl RayTable {
    fn ray(&self, square: Square, direction: usize) -> &[u8] {
        &self.squares[square as usize][direction][..usize::from(self.length[square as usize][direction])]
    }
}

static RAYS: RayTable = {
    let mut table = RayTable { squares: [[[0; 7]; 4]; 32], length: [[0; 4]; 32] };
    let mut square = 0;
    while square < 32 {
        let mut d = 0;
        while d < 4 {
            let (mut file, mut rank) = (file_of(square) + FILE_STEP[d], rank_of(square) + RANK_STEP[d]);
            while is_dark(file, rank) {
                let length = table.length[square as usize][d] as usize;
                table.squares[square as usize][d][length] = square_at(file, rank) as u8;
                table.length[square as usize][d] += 1;
                file += FILE_STEP[d];
                rank += RANK_STEP[d];
            }
            d += 1;
        }
        square += 1;
    }
    table
};

const fn split_mix_64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The keys that make up a position's key: for a white man, a white king, a black man and a black king on
/// each square, and for Black to move.
struct ZobristKeys {
    piece: [[u64; 32]; 4],
    side: u64,
}

static ZOBRIST: ZobristKeys = {
    let mut keys = ZobristKeys { piece: [[0; 32]; 4], side: 0 };
    let mut state = 0x5D2C_7A11_E9F3_B046;
    let mut kind = 0;
    while kind < 4 {
        let mut square = 0;
        while square < 32 {
            keys.piece[kind][square] = split_mix_64(&mut state);
            square += 1;
        }
        kind += 1;
    }
    keys.side = split_mix_64(&mut state);
    keys
};

fn piece_key(color: Color, king: bool, square: Square) -> u64 {
    ZOBRIST.piece[if color == Color::White { 0 } else { 2 } + usize::from(king)][square as usize]
}

/// Whether a king at a square could take something, in the middle of a capture that has taken `taken`.
fn flying_king_can_take(at: Square, enemy: u32, occupied: u32, taken: u32) -> bool {
    (0..4).any(|d| {
        let ray = RAYS.ray(at, d);
        let Some(k) = ray.iter().position(|square| occupied & bit(Square::from(*square)) != 0) else { return false };
        let victim = bit(Square::from(ray[k]));
        k + 1 < ray.len() && enemy & victim != 0 && taken & victim == 0 && occupied & bit(Square::from(ray[k + 1])) == 0
    })
}

/// Builds every capture of one piece, jump by jump, by the rules: a piece cannot be jumped twice and stays on
/// the board until the capture is over (the "Turkish strike"), a capture goes on while anything can be
/// taken, and a flying king that can go on taking must land where it can.
struct CaptureBuilder<'a> {
    rules: &'a Rules,
    side: Color,
    enemy: u32,
    occupied: u32, // without the piece that moves
    moves: &'a mut Vec<Move>,
    current: Move,
    crowned: bool,
    first: usize,
}

impl CaptureBuilder<'_> {
    fn from(&mut self, square: Square, king: bool) {
        self.current = Move { from: square as u8, ..Move::default() };
        self.crowned = false;
        self.first = self.moves.len();
        if king && self.rules.flying_kings {
            self.flying_king(square, 0);
        } else {
            self.short_jumps(square, 0, king);
        }
    }

    fn push(&mut self, landing: Square, victim: Square) {
        let step = usize::from(self.current.steps);
        self.current.path[step] = landing as u8;
        self.current.taken[step] = victim as u8;
        self.current.steps += 1;
    }

    fn pop(&mut self) {
        self.current.steps -= 1;
    }

    /// A man, or a king that is not a flying one, jumping over a neighbour.
    fn short_jumps(&mut self, at: Square, taken: u32, king: bool) {
        let mut found = false;
        for d in 0..4 {
            let ray = RAYS.ray(at, d);
            if (!king && !self.rules.men_take_back && !is_forward(self.side, d)) || ray.len() < 2 {
                continue;
            }
            let (victim, landing) = (Square::from(ray[0]), Square::from(ray[1]));
            if self.enemy & bit(victim) == 0 || taken & bit(victim) != 0 || self.occupied & bit(landing) != 0 {
                continue;
            }
            found = true;
            let now = taken | bit(victim);
            self.push(landing, victim);
            if !king && bit(landing) & crown_row(self.side) != 0 {
                self.crowned = true;
                if !self.rules.crowned_takes_on {
                    self.record(now); // the crowning ends the move
                } else if self.rules.flying_kings {
                    self.flying_king(landing, now);
                } else {
                    self.short_jumps(landing, now, true);
                }
                self.crowned = false;
            } else {
                self.short_jumps(landing, now, king);
            }
            self.pop();
        }
        if !found && self.current.steps > 0 {
            self.record(taken);
        }
    }

    fn flying_king(&mut self, at: Square, taken: u32) {
        let (enemy, occupied) = (self.enemy, self.occupied);
        let mut found = false;
        for d in 0..4 {
            let ray = RAYS.ray(at, d);
            let free = |square: &u8| occupied & bit(Square::from(*square)) == 0;
            let k = ray.iter().take_while(|square| free(square)).count();
            if k + 1 >= ray.len() || enemy & bit(Square::from(ray[k])) == 0 || taken & bit(Square::from(ray[k])) != 0 {
                continue;
            }
            let end = k + 1 + ray[k + 1..].iter().take_while(|square| free(square)).count();
            if end == k + 1 {
                continue;
            }
            found = true;
            let victim = Square::from(ray[k]);
            let now = taken | bit(victim);
            // Where the king could go on taking, it must land on one of those squares.
            let onward = |square: u8| flying_king_can_take(Square::from(square), enemy, occupied, now);
            let any_onward = ray[k + 1..end].iter().any(|square| onward(*square));
            for &landing in &ray[k + 1..end] {
                if any_onward && !onward(landing) {
                    continue;
                }
                self.push(Square::from(landing), victim);
                self.flying_king(Square::from(landing), now);
                self.pop();
            }
        }
        if !found && self.current.steps > 0 {
            self.record(taken);
        }
    }

    fn record(&mut self, taken: u32) {
        let found = Move { captured: taken, crowns: self.crowned, ..self.current };
        // Another route to the same result is the same move.
        if !self.moves[self.first..].contains(&found) {
            self.moves.push(found);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Position {
    white: u32,
    black: u32,
    kings: u32,
    side: Color,
    quiet_plies: u16,
    key: u64,
}

impl Position {
    pub fn start(variant: Variant) -> Position {
        // The first three ranks, and the last three.
        let mut position = Position { white: 0x0000_0FFF, black: 0xFFF0_0000, kings: 0, side: rules_of(variant).first, quiet_plies: 0, key: 0 };
        position.compute_key();
        position
    }

    /// A PDN position: "W:W21,22,K30:B1-12", or with the squares named, "W:Wc3,Ka1:Bb8".
    pub fn from_fen(fen: &str) -> Result<Position, String> {
        let text: String = fen.chars().filter(|c| !c.is_whitespace() && *c != '"').collect();
        let mut parts = text.trim_end_matches('.').split(':');

        let side = match parts.next().filter(|part| part.len() == 1).map(str::to_ascii_uppercase).as_deref() {
            Some("W") => Color::White,
            Some("B") => Color::Black,
            Some(_) => return Err("The side to move should be W or B.".to_owned()),
            None => return Err("The position should start with the side to move, W or B.".to_owned()),
        };
        let mut position = Position { white: 0, black: 0, kings: 0, side, quiet_plies: 0, key: 0 };
        let mut sides = 0;
        for part in parts {
            sides += 1;
            if part.is_empty() {
                continue;
            }
            let white = match part.as_bytes()[0].to_ascii_uppercase() {
                b'W' => true,
                b'B' => false,
                _ => return Err(format!("Expected W or B before the squares, found \"{part}\".")),
            };
            for item in part[1..].split(',').filter(|item| !item.is_empty()) {
                let (king, item) = match item.strip_prefix(['K', 'k']) {
                    Some(rest) => (true, rest),
                    None => (false, item),
                };
                // A square, or a range of numbered squares such as "1-12".
                let squares: Vec<Square> = if let Some((first, last)) = item.split_once('-') {
                    match (parse_number(first), parse_number(last)) {
                        (Some(first), Some(last)) if first >= 1 && last <= 32 && first <= last => {
                            (first..=last).filter_map(|number| parse_square(&number.to_string())).collect()
                        }
                        _ => return Err(format!("\"{item}\" is not a range of squares.")),
                    }
                } else {
                    vec![parse_square(item).ok_or_else(|| format!("\"{item}\" is not a dark square."))?]
                };
                for square in squares {
                    if position.occupied() & bit(square) != 0 {
                        return Err("A square is taken twice.".to_owned());
                    }
                    *(if white { &mut position.white } else { &mut position.black }) |= bit(square);
                    if king {
                        position.kings |= bit(square);
                    }
                }
            }
        }
        if sides == 0 {
            return Err("The position should start with the side to move, W or B.".to_owned());
        }
        if position.men(Color::White) & crown_row(Color::White) != 0 || position.men(Color::Black) & crown_row(Color::Black) != 0 {
            return Err("A man cannot stand on the far rank; it would be a king.".to_owned());
        }
        if position.occupied() == 0 {
            return Err("The position has no pieces.".to_owned());
        }
        position.compute_key();
        Ok(position)
    }

    pub fn fen(&self, variant: Variant) -> String {
        let mut fen = String::from(if self.side == Color::White { "W" } else { "B" });
        for color in [Color::White, Color::Black] {
            fen += if color == Color::White { ":W" } else { ":B" };
            let mut list: Vec<Square> = squares(self.pieces(color)).collect();
            if variant == Variant::English {
                list.sort_by_key(|square| number_of(*square));
            }
            let names: Vec<String> =
                list.iter().map(|square| format!("{}{}", if self.kings & bit(*square) != 0 { "K" } else { "" }, square_name(*square, variant))).collect();
            fen += &names.join(",");
        }
        fen
    }

    pub fn side_to_move(&self) -> Color {
        self.side
    }

    pub fn pieces(&self, color: Color) -> u32 {
        match color {
            Color::White => self.white,
            Color::Black => self.black,
        }
    }

    pub fn men(&self, color: Color) -> u32 {
        self.pieces(color) & !self.kings
    }

    pub fn kings(&self) -> u32 {
        self.kings
    }

    pub fn kings_of(&self, color: Color) -> u32 {
        self.pieces(color) & self.kings
    }

    pub fn occupied(&self) -> u32 {
        self.white | self.black
    }

    pub fn at(&self, square: Square) -> Option<Piece> {
        if self.occupied() & bit(square) == 0 {
            return None;
        }
        Some(Piece { color: if self.white & bit(square) != 0 { Color::White } else { Color::Black }, king: self.kings & bit(square) != 0 })
    }

    /// King moves with nothing taken, in a row.
    pub fn quiet_plies(&self) -> u16 {
        self.quiet_plies
    }

    pub fn key(&self) -> u64 {
        self.key
    }

    /// A king against a king, with nothing else on the board.
    pub fn lone_kings(&self) -> bool {
        self.white.count_ones() == 1 && self.black.count_ones() == 1 && self.kings == self.occupied()
    }

    fn can_take(&self, rules: &Rules, square: Square) -> bool {
        let side = if self.white & bit(square) != 0 { Color::White } else { Color::Black };
        let (enemy, occupied) = (self.pieces(side.opposite()), self.occupied());
        let king = self.kings & bit(square) != 0;
        if king && rules.flying_kings {
            return flying_king_can_take(square, enemy, occupied & !bit(square), 0);
        }
        (0..4).any(|d| {
            let ray = RAYS.ray(square, d);
            (king || rules.men_take_back || is_forward(side, d))
                && ray.len() >= 2
                && enemy & bit(Square::from(ray[0])) != 0
                && occupied & bit(Square::from(ray[1])) == 0
        })
    }

    pub fn has_capture(&self, rules: &Rules) -> bool {
        self.pieces_that_can_take(rules) != 0
    }

    /// The pieces of the side to move that can take something.
    pub fn pieces_that_can_take(&self, rules: &Rules) -> u32 {
        squares(self.pieces(self.side)).filter(|square| self.can_take(rules, *square)).fold(0, |all, square| all | bit(square))
    }

    /// The legal moves: when something can be taken, only the captures.
    pub fn generate_moves(&self, rules: &Rules, moves: &mut Vec<Move>) {
        moves.clear();
        let (own, enemy, occupied) = (self.pieces(self.side), self.pieces(self.side.opposite()), self.occupied());

        // Taking is compulsory: when anything can be taken, the captures are the only moves.
        for square in squares(own) {
            if self.can_take(rules, square) {
                let mut builder = CaptureBuilder {
                    rules,
                    side: self.side,
                    enemy,
                    occupied: occupied & !bit(square),
                    moves,
                    current: Move::default(),
                    crowned: false,
                    first: 0,
                };
                builder.from(square, self.kings & bit(square) != 0);
            }
        }
        if !moves.is_empty() {
            return;
        }

        for square in squares(own) {
            let king = self.kings & bit(square) != 0;
            for d in 0..4 {
                if !king && !is_forward(self.side, d) {
                    continue;
                }
                let ray = RAYS.ray(square, d);
                let reach = if king && rules.flying_kings { ray.len() } else { ray.len().min(1) };
                for &landing in ray[..reach].iter().take_while(|landing| occupied & bit(Square::from(**landing)) == 0) {
                    let mut found =
                        Move { from: square as u8, steps: 1, crowns: !king && bit(Square::from(landing)) & crown_row(self.side) != 0, ..Move::default() };
                    found.path[0] = landing;
                    moves.push(found);
                }
            }
        }
    }

    pub fn legal_moves(&self, rules: &Rules) -> Vec<Move> {
        let mut moves = Vec::new();
        self.generate_moves(rules, &mut moves);
        moves
    }

    pub fn after(&self, played: &Move) -> Position {
        let mut next = *self;
        let side = self.side;
        let (from, to) = (played.from(), played.to());
        let king = self.kings & bit(from) != 0;
        let crowned = king || played.crowns;

        next.key ^= piece_key(side, king, from) ^ piece_key(side, crowned, to);
        for square in squares(played.captured) {
            next.key ^= piece_key(side.opposite(), self.kings & bit(square) != 0, square);
        }
        let (own, enemy) = match side {
            Color::White => (&mut next.white, &mut next.black),
            Color::Black => (&mut next.black, &mut next.white),
        };
        *own = (*own & !bit(from)) | bit(to);
        *enemy &= !played.captured;
        next.kings &= !bit(from) & !played.captured;
        if crowned {
            next.kings |= bit(to);
        }
        next.quiet_plies = if king && !played.is_capture() { self.quiet_plies + 1 } else { 0 };
        next.side = side.opposite();
        next.key ^= ZOBRIST.side;
        next
    }

    fn compute_key(&mut self) {
        self.key = if self.side == Color::Black { ZOBRIST.side } else { 0 };
        for square in squares(self.occupied()) {
            let color = if self.white & bit(square) != 0 { Color::White } else { Color::Black };
            self.key ^= piece_key(color, self.kings & bit(square) != 0, square);
        }
    }
}

/// "c3-d4", "c3:e5:g3" in Russian notation; "22-18", "18x11x2" in English.
pub fn to_notation(played: &Move, variant: Variant) -> String {
    let separator = if !played.is_capture() {
        '-'
    } else if variant == Variant::English {
        'x'
    } else {
        ':'
    };
    let mut text = square_name(played.from(), variant);
    for landing in played.landings() {
        text.push(separator);
        text += &square_name(landing, variant);
    }
    text
}

/// The legal move that the squares name; the squares on the way may be left out where the move is clear without them.
pub fn parse_move(text: &str, position: &Position, rules: &Rules) -> Option<Move> {
    // The squares, between the separators and marks such as "!" or "?".
    let named: Vec<Square> =
        text.split(|c: char| !c.is_ascii_alphanumeric() || c == 'x' || c == 'X').filter(|token| !token.is_empty()).map(parse_square).collect::<Option<_>>()?;
    if named.len() < 2 {
        return None;
    }

    let mut found = None;
    for candidate in position.legal_moves(rules) {
        if candidate.from() != named[0] || candidate.to() != named[named.len() - 1] {
            continue;
        }
        // The squares named on the way must be landings of the move, in order.
        let mut next = 1;
        for landing in candidate.landings().take(usize::from(candidate.steps) - 1) {
            if next + 1 < named.len() && landing == named[next] {
                next += 1;
            }
        }
        if next + 1 != named.len() {
            continue;
        }
        if usize::from(candidate.steps) + 1 == named.len() {
            return Some(candidate); // the whole route was given
        }
        found = found.or(Some(candidate));
    }
    found
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GameResult {
    #[default]
    Ongoing,
    WhiteWins,
    BlackWins,
    Draw,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EndReason {
    #[default]
    None,
    NoPieces,
    NoMoves,
    Repetition,
    QuietMoves,
    LoneKings,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GameStatus {
    pub result: GameResult,
    pub reason: EndReason,
}

impl GameStatus {
    pub fn over(&self) -> bool {
        self.result != GameResult::Ongoing
    }
}

#[derive(Clone)]
pub struct MoveRecord {
    pub played: Move,
    pub king: bool,       // the piece moved as a king
    pub taken_kings: u32, // the pieces taken that were kings
    pub notation: String,
}

/// A game: the variant, the starting position, the moves played, and the moves taken back that can be replayed.
#[derive(Clone)]
pub struct Game {
    variant: Variant,
    positions: Vec<Position>,
    moves: Vec<MoveRecord>,
    redo: Vec<MoveRecord>,
    status: GameStatus,
}

impl Game {
    pub fn new(variant: Variant) -> Game {
        Game::from_position(variant, Position::start(variant))
    }

    pub fn from_position(variant: Variant, start: Position) -> Game {
        let mut game = Game { variant, positions: vec![start], moves: Vec::new(), redo: Vec::new(), status: GameStatus::default() };
        game.update_status();
        game
    }

    /// A PDN game or position. The GameType tag chooses the variant; without one, the fallback is used.
    pub fn parse(text: &str, fallback: Variant) -> Result<Game, String> {
        // The tags, and the moves with comments and variations left out.
        let mut variant = fallback;
        let mut start = None;
        let mut moves = String::new();
        let mut depth = 0;
        let mut rest = text;
        while let Some(c) = rest.chars().next() {
            if depth == 0 && c == '[' {
                let Some(end) = rest.find(']') else { break };
                let tag = &rest[1..end];
                let (name, value) = match tag.split_once('"') {
                    Some((name, value)) => (name, value.split('"').next().unwrap_or_default()),
                    None => (tag, ""),
                };
                match name.trim().to_ascii_lowercase().as_str() {
                    "gametype" if value.starts_with("21") => variant = Variant::English,
                    "gametype" if value.starts_with("25") && variant != Variant::Giveaway => variant = Variant::Russian,
                    "variant" if value.to_ascii_lowercase().contains("giveaway") || value.contains("оддавк") => variant = Variant::Giveaway,
                    "fen" => start = Some(Position::from_fen(value)?),
                    _ => {}
                }
                rest = &rest[end + 1..];
                continue;
            }
            match c {
                '{' | '(' => depth += 1,
                '}' | ')' if depth > 0 => depth -= 1,
                _ if depth == 0 => moves.push(c),
                _ => {}
            }
            rest = &rest[c.len_utf8()..];
        }

        // A position on its own.
        let moves = moves.trim();
        if start.is_none() && moves.len() > 2 && matches!(moves.as_bytes(), [b'W' | b'B' | b'w' | b'b', b':', ..]) {
            return Ok(Game::from_position(variant, Position::from_fen(moves)?));
        }

        let has_start = start.is_some();
        let mut game = Game::from_position(variant, start.unwrap_or_else(|| Position::start(variant)));
        let mut played = 0;
        for token in moves.split_whitespace() {
            // After a move number, if there is one.
            let token = token.rsplit('.').next().unwrap_or_default();
            if token.is_empty() {
                continue;
            }
            if matches!(token, "*" | "1-0" | "0-1" | "1/2-1/2" | "2-0" | "0-2" | "1-1" | "0-0") {
                break;
            }
            let found = parse_move(token, game.current(), game.rules());
            if !found.is_some_and(|found| game.play(&found)) {
                return Err(format!("The move \"{token}\" cannot be played."));
            }
            played += 1;
        }
        if played == 0 && !has_start {
            return Err("There is no game or position here.".to_owned());
        }
        Ok(game)
    }

    /// The variant, the starting position and the moves, for saving.
    pub fn serialize(&self) -> String {
        let moves: Vec<String> = self.moves.iter().map(|record| to_notation(&record.played, Variant::Russian)).collect();
        format!("{}|{}|{}", self.variant.name(), self.start().fen(Variant::Russian), moves.join(" "))
    }

    pub fn deserialize(text: &str) -> Option<Game> {
        let mut parts = text.splitn(3, '|');
        let (name, start, moves) = (parts.next()?, parts.next()?, parts.next()?);
        let variant = Variant::ALL.into_iter().find(|variant| variant.name() == name)?;
        let mut game = Game::from_position(variant, Position::from_fen(start).ok()?);
        for token in moves.split(' ').filter(|token| !token.is_empty()) {
            let found = parse_move(token, game.current(), game.rules());
            if !found.is_some_and(|found| game.play(&found)) {
                break; // keep what could be played
            }
        }
        Some(game)
    }

    pub fn variant(&self) -> Variant {
        self.variant
    }

    pub fn rules(&self) -> &'static Rules {
        rules_of(self.variant)
    }

    pub fn current(&self) -> &Position {
        self.positions.last().expect("a game has its starting position")
    }

    pub fn start(&self) -> &Position {
        &self.positions[0]
    }

    /// The position before a move of the game.
    pub fn position_at(&self, ply: usize) -> &Position {
        &self.positions[ply]
    }

    pub fn moves(&self) -> &[MoveRecord] {
        &self.moves
    }

    pub fn status(&self) -> GameStatus {
        self.status
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        if self.status.over() { Vec::new() } else { self.current().legal_moves(self.rules()) }
    }

    /// Plays a move; only legal moves are played.
    pub fn play(&mut self, played: &Move) -> bool {
        let Some(legal) = self.legal_moves().into_iter().find(|legal| legal == played) else { return false };
        self.push(legal);
        self.redo.clear();
        self.update_status();
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.moves.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(record) = self.moves.pop() else { return false };
        self.redo.push(record);
        self.positions.pop();
        self.update_status();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(record) = self.redo.pop() else { return false };
        self.push(record.played);
        self.update_status();
        true
    }

    /// The keys of the earlier positions that a repetition could still return to, for the engine.
    pub fn repetition_history(&self) -> Vec<u64> {
        // Moving a man or taking a piece cannot be undone, so no position before it can come back.
        let reversible = self.moves.iter().rev().take_while(|record| record.king && !record.played.is_capture()).count();
        let first = self.moves.len() - reversible;
        self.positions[first..self.moves.len()].iter().map(Position::key).collect()
    }

    pub fn to_pdn(&self, white_name: &str, black_name: &str) -> String {
        let (year, month, day) = today();
        let result = match self.status.result {
            GameResult::WhiteWins => "1-0",
            GameResult::BlackWins => "0-1",
            GameResult::Draw => "1/2-1/2",
            GameResult::Ongoing => "*",
        };
        let mut pdn = format!(
            "[Event \"3D Checkers\"]\n[Date \"{year:04}.{month:02}.{day:02}\"]\n[White \"{white_name}\"]\n[Black \"{black_name}\"]\n[Result \"{result}\"]\n"
        );
        pdn += &format!("[GameType \"{}\"]\n", if self.variant == Variant::English { 21 } else { 25 });
        if self.variant == Variant::Giveaway {
            pdn += "[Variant \"Giveaway\"]\n";
        }
        if self.start().key() != Position::start(self.variant).key() {
            pdn += &format!("[FEN \"{}\"]\n", self.start().fen(self.variant));
        }
        pdn.push('\n');

        // The moves in pairs, the side that moves first in the variant first in each pair.
        let first = self.rules().first;
        let mut words = Vec::new();
        let mut number = 1;
        for (ply, record) in self.moves.iter().enumerate() {
            let mover = self.positions[ply].side_to_move();
            words.push(if mover == first {
                format!("{number}. {}", record.notation)
            } else if ply == 0 {
                format!("{number}... {}", record.notation)
            } else {
                record.notation.clone()
            });
            if mover != first {
                number += 1;
            }
        }
        words.push(result.to_owned());

        // In lines of 80 characters at most.
        let mut line = String::new();
        for word in words {
            if !line.is_empty() && line.len() + word.len() + 1 > 80 {
                pdn += &line;
                pdn.push('\n');
                line.clear();
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line += &word;
        }
        pdn + &line + "\n"
    }

    fn push(&mut self, played: Move) {
        let position = *self.current();
        self.moves.push(MoveRecord {
            played,
            king: position.kings() & bit(played.from()) != 0,
            taken_kings: played.captured & position.kings(),
            notation: to_notation(&played, self.variant),
        });
        self.positions.push(position.after(&played));
    }

    fn update_status(&mut self) {
        let position = *self.current();
        let rules = self.rules();
        let moves = position.legal_moves(rules);
        let side = position.side_to_move();
        self.status = if moves.is_empty() {
            let winner = if rules.giveaway { side } else { side.opposite() };
            GameStatus {
                result: if winner == Color::White { GameResult::WhiteWins } else { GameResult::BlackWins },
                reason: if position.pieces(side) == 0 { EndReason::NoPieces } else { EndReason::NoMoves },
            }
        } else if position.quiet_plies() >= rules.quiet_limit {
            GameStatus { result: GameResult::Draw, reason: EndReason::QuietMoves }
        } else if self.repetition_history().iter().filter(|key| **key == position.key()).count() >= 2 {
            GameStatus { result: GameResult::Draw, reason: EndReason::Repetition }
        } else if rules.lone_kings_draw && position.lone_kings() && !moves[0].is_capture() {
            GameStatus { result: GameResult::Draw, reason: EndReason::LoneKings }
        } else {
            GameStatus::default()
        };
    }
}

/// Today's date by universal time, as a year, a month and a day.
fn today() -> (i64, i64, i64) {
    // After Howard Hinnant's "civil from days".
    let days = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs() / 86_400) as i64;
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 { shifted_month + 3 } else { shifted_month - 9 };
    (year_of_era + era * 400 + i64::from(month <= 2), month, day)
}
