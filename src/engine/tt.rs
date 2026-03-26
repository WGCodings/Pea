use std::mem::transmute;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use shakmaty::{Move, Role, Square};
use crate::engine::search::context::SearchContext;
use crate::engine::types::MATE_SCORE;
use crate::engine::utility::role_from_index;

const MATE_THRESHOLD: i32 = MATE_SCORE - 1000;

// =====================================================================================================================//
// BOUND ENUM                                                                                                           //
// =====================================================================================================================//
#[derive(Clone, Copy, PartialEq)]
pub enum Bound {
    Exact = 0,
    Lower = 1,
    Upper = 2,
}

impl Bound {
    fn from_u8(v: u8) -> Self {
        match v & 0b11 {
            0 => Bound::Exact,
            1 => Bound::Lower,
            _ => Bound::Upper,
        }
    }
}

// =====================================================================================================================//
// TTEntry                                                                                                              //
// =====================================================================================================================//
#[derive(Clone)]
pub struct TTEntry {
    pub key:       u64,
    pub depth:     u8,
    pub age:       u8,
    pub score:     i32,
    pub eval:      i32,
    pub bound:     Bound,
    pub best_move: Option<Move>,
}

impl TTEntry {
    #[inline(always)]
    pub fn try_score(&self, depth: usize, alpha: i32, beta: i32, ply: usize) -> Option<i32> {
        if self.depth as usize >= depth {
            let score = score_from_tt(self.score, ply);
            match self.bound {
                Bound::Exact => return Some(score),
                Bound::Lower if score >= beta  => return Some(beta),
                Bound::Upper if score <= alpha => return Some(alpha),
                _ => {}
            }
        }
        None
    }
}

// =====================================================================================================================//
// LAYOUT: score(2) + eval(2) + mv(2) + depth(1) + info(1) = 8 bytes = 64 bits                                        //
// info byte: age(5) + bound(2) + spare(1)                                                                             //
// =====================================================================================================================//
#[repr(C)]
#[derive(Clone, Copy)]
struct Layout {
    score: i16,
    eval:  i16,
    mv:    u16,
    depth: u8,
    info:  u8,  // age(5 bits) | bound(2 bits) | spare(1 bit)
}


// =====================================================================================================================//
// PACKED ENTRY                                                                                                         //
// =====================================================================================================================//

pub struct PackedEntry {
    key:  AtomicU64,  // key XOR data
    data: AtomicU64,  // Layout packed as u64
}
impl PackedEntry {
    fn default() -> Self {
        Self {
            key:  AtomicU64::new(0),
            data: AtomicU64::new(0),
        }
    }
}

impl PackedEntry {
    fn store(&self, entry: &TTEntry) {
        let info = (entry.age << 3) | (entry.bound as u8);
        let layout = Layout {
            score: entry.score as i16,
            eval:  entry.eval  as i16,
            mv:    encode_move(entry.best_move),
            depth: entry.depth,
            info,
        };
        let data = unsafe { transmute::<Layout, u64>(layout) };

        self.data.store(data, Ordering::Relaxed);
        self.key.store(entry.key ^ data, Ordering::Relaxed);
    }

    fn load(&self, key: u64) -> Option<TTEntry> {
        let data       = self.data.load(Ordering::Relaxed);
        let stored_key = self.key.load(Ordering::Relaxed) ^ data;

        if stored_key != key { return None; }

        let layout: Layout = unsafe { transmute::<u64, Layout>(data) };
        let age   = layout.info >> 3;
        let bound = Bound::from_u8(layout.info & 0b11);

        // best_move decoded later with legal move validation
        Some(TTEntry {
            key:       stored_key,
            depth:     layout.depth,
            age,
            score:     layout.score as i32,
            eval:      layout.eval  as i32,
            bound,
            best_move: decode_move_partial(layout.mv),
        })
    }
}

// =====================================================================================================================//
// TRANSPOSITION TABLE                                                                                                  //
// =====================================================================================================================//
pub struct TranspositionTable {
    table: Vec<PackedEntry>,
    mask:  usize,
    age:   AtomicU8,
}

impl TranspositionTable {
    pub fn new(size_mb: usize) -> Self {
        let bytes    = size_mb * 1024 * 1024;
        let capacity = (bytes / size_of::<PackedEntry>()).next_power_of_two();
        Self {
            table: (0..capacity).map(|_| PackedEntry::default()).collect(),
            mask:  capacity - 1,
            age:   AtomicU8::new(0),
        }
    }

    pub fn init_tt(&mut self, size_mb: usize) {
        *self = Self::new(size_mb);
    }

    #[inline(always)]
    fn index(&self, key: u64) -> usize {
        key as usize & self.mask
    }

    // Probe without move validation — use when legal moves not yet generated
    #[inline(always)]
    pub fn probe(&self, key: u64) -> Option<TTEntry> {
        self.table[self.index(key)].load(key)
    }

    // Probe with move validation — returns fully reconstructed Move
    #[inline(always)]
    pub fn probe_move(&self, key: u64, legal_moves: &[Move]) -> Option<Move> {
        let entry = self.table[self.index(key)].load(key)?;
        let encoded = encode_move(entry.best_move);
        if encoded == 0 { return None; }
        validate_move(encoded, legal_moves)
    }

    #[inline(always)]
    pub fn store(&self, key: u64, depth: usize, score: i32, eval: i32,
                 bound: Bound, best_move: Option<Move>) {
        let idx         = self.index(key);
        let current_age = self.age.load(Ordering::Relaxed);

        if let Some(existing) = self.table[idx].load(key) {
            if existing.key == key
                && existing.depth > depth as u8 + 2
                && existing.age == current_age {
                return;
            }
        }

        self.table[idx].store(&TTEntry {
            key,
            depth: depth as u8,
            age:   current_age,
            score,
            eval,
            bound,
            best_move,
        });
    }

    pub fn clear(&mut self) {
        for entry in &self.table {
            entry.key.store(0,  Ordering::Relaxed);
            entry.data.store(0, Ordering::Relaxed);
        }
        self.age.store(0, Ordering::Relaxed);
    }

    pub fn tt_occupancy(&self) -> u32 {
        self.table[..1000]
            .iter()
            .filter(|e| e.key.load(Ordering::Relaxed) != 0)
            .count() as u32
    }

    pub fn increment_age(&self) {
        let _ = self.age.fetch_update(
            Ordering::Relaxed,
            Ordering::Relaxed,
            |age| Some((age + 1) % 32),
        );
    }
}

// =====================================================================================================================//
// MOVE ENCODING / DECODING                                                                                             //
// =====================================================================================================================//

// Encode a Move into 16 bits. 0 = None.
// Layout: from(6) | to(6) | extra(2) | type(2)
// type: 0=Normal, 1=EnPassant, 2=Castle, 3=Put
// extra for Normal: 0=no promo, 1=Q, 2=R, 3=B (knight uses type bits differently)
#[inline(always)]
pub fn encode_move(mv: Option<Move>) -> u16 {
    match mv {
        None => 0,
        Some(Move::Normal { from, to, promotion, .. }) => {
            let promo = match promotion {
                None               => 0u16,
                Some(Role::Queen)  => 1u16,
                Some(Role::Rook)   => 2u16,
                Some(Role::Bishop) => 3u16,
                Some(Role::Knight) => 4u16,
                _                  => 0u16,
            };
            // bits 0-5: from, bits 6-11: to, bits 12-14: promo(3 bits), bit 15: 0
            (from as u16) | ((to as u16) << 6) | (promo << 12)
        }
        Some(Move::EnPassant { from, to }) => {
            // bit 15: 1, bit 14: 0 → type=2 (10xxxxxxxxxxxxxx)
            (from as u16) | ((to as u16) << 6) | (1u16 << 15)
        }
        Some(Move::Castle { king, rook }) => {
            // bit 15: 1, bit 14: 1 → type=3 (11xxxxxxxxxxxxxx)
            (king as u16) | ((rook as u16) << 6) | (3u16 << 14)
        }
        Some(Move::Put { role, to }) => {
            // bit 15: 0, bits 12-14: role+5 to avoid collision with promo
            (to as u16) | ((role as u16 + 5) << 12)
        }
    }
}

// Decode a partial move (role and capture unknown for Normal moves)
// Used only internally — call validate_move for full reconstruction
#[inline(always)]
pub fn decode_move_partial(encoded: u16) -> Option<Move> {
    if encoded == 0 { return None; }

    let from    = Square::new((encoded & 0x3F) as u32);
    let to      = Square::new(((encoded >> 6) & 0x3F) as u32);
    let extra   = (encoded >> 12) & 0x3;
    let mv_type = (encoded >> 14) & 0x3;

    match mv_type {
        0 => {
            let promotion = match extra {
                1 => Some(Role::Queen),
                2 => Some(Role::Rook),
                3 => Some(Role::Bishop),
                4 => Some(Role::Knight),
                _ => None,
            };
            Some(Move::Normal {
                role:      Role::Pawn, // placeholder
                from,
                capture:   None,       // placeholder
                to,
                promotion,
            })
        }
        1 => Some(Move::EnPassant { from, to }),
        2 => Some(Move::Castle { king: from, rook: to }),
        3 => {

            let role = role_from_index(extra as usize).unwrap_or(Role::Pawn);
            Some(Move::Put { role, to })
        }
        _ => None,
    }
}

// Validate encoded move against legal moves to get full Move
#[inline(always)]
pub fn validate_move(encoded: u16, legal_moves: &[Move]) -> Option<Move> {

    if encoded == 0 { return None; }

    let from = Square::new((encoded & 0x3F) as u32);
    let to   = Square::new(((encoded >> 6) & 0x3F) as u32);
    let high = encoded >> 12;  // top 4 bits

    // Determine move type from high bits
    if encoded & (1 << 15) != 0 {
        // EnPassant or Castle
        if encoded & (1 << 14) != 0 {
            // Castle: bits 14-15 = 11
            return legal_moves.iter().copied().find(|mv| matches!(mv,
                Move::Castle { king, rook } if *king == from && *rook == to
            ));
        } else {
            // EnPassant: bits 14-15 = 10
            return legal_moves.iter().copied().find(|mv| matches!(mv,
                Move::EnPassant { from: f, to: t } if *f == from && *t == to
            ));
        }
    }

    if high >= 5 {
        // Put move
        let role = role_from_index((high - 5) as usize).unwrap_or(Role::Pawn);
        return legal_moves.iter().copied().find(|mv| matches!(mv,
            Move::Put { role: r, to: t } if *r == role && *t == to
        ));
    }

    // Normal move: high bits 0-4 = promo
    let promotion = match high {
        0 => None,
        1 => Some(Role::Queen),
        2 => Some(Role::Rook),
        3 => Some(Role::Bishop),
        4 => Some(Role::Knight),
        _ => None,
    };

    legal_moves.iter().copied().find(|mv| matches!(mv,
        Move::Normal { from: f, to: t, promotion: p, .. }
            if *f == from && *t == to && *p == promotion
    ))
}

// =====================================================================================================================//
// HELPER FUNCTIONS                                                                                                     //
// =====================================================================================================================//
#[inline(always)]
fn score_to_tt(score: i32, ply: usize) -> i32 {
    if score > MATE_THRESHOLD { score + ply as i32 }
    else if score < -MATE_THRESHOLD { score - ply as i32 }
    else { score }
}

#[inline(always)]
pub fn score_from_tt(score: i32, ply: usize) -> i32 {
    if score > MATE_THRESHOLD { score - ply as i32 }
    else if score < -MATE_THRESHOLD { score + ply as i32 }
    else { score }
}

#[inline(always)]
pub(crate) fn tt_probe(
    key: u64,
    ctx: &SearchContext,
    depth: usize,
    alpha: i32,
    beta: i32,
    ply: usize,
) -> Option<i32> {
    ctx.tt.probe(key)?.try_score(depth, alpha, beta, ply)
}

#[inline(always)]
pub(crate) fn tt_store(
    key: u64,
    ctx: &SearchContext,
    depth: usize,
    best_score: i32,
    eval: i32,
    alpha: i32,
    beta: i32,
    best_move: Option<Move>,
    ply: usize,
) {
    let bound = if best_score <= alpha { Bound::Upper }
    else if best_score >= beta { Bound::Lower }
    else { Bound::Exact };
    ctx.tt.store(key, depth, score_to_tt(best_score, ply), eval, bound, best_move);
}