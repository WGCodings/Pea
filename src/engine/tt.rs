use std::mem::transmute;
use std::sync::atomic::{AtomicU16, AtomicU64, AtomicU8, Ordering};
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
    None  = 3,
}

impl Bound {
    fn from_u8(v: u8) -> Self {
        match v & 0b11 {
            0 => Bound::Exact,
            1 => Bound::Lower,
            2 => Bound::Upper,
            _ => Bound::None,
        }
    }
}

// =====================================================================================================================//
// TTEntry                                                                                                              //
// =====================================================================================================================//
#[derive(Clone)]
pub struct TTEntry {
    pub depth:     u8,
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
// LAYOUT: key((2) + score(2) + mv(2) + depth(1) + info(1)                                                              //
// info byte: age(5) + occupied (1) + bound(2)                                                                          //
// =====================================================================================================================//
#[repr(C)]
#[derive(Clone, Copy)]
struct Layout {
    key:   u16, // hold lower 16 bits of zobrist
    score: i16,
    mv:    u16,
    depth: u8,
    info:  u8,  // age(5 bits) | occupied (1 bit) | bound(2 bits)
}
const OCCUPIED_MASK : u8 = 0b0000_0100; // Mask to get occupied bit from info

// Helpers to go from layout to u64 and back
#[inline(always)]
fn pack(layout: Layout) -> u64 {
    unsafe { transmute::<Layout, u64>(layout) }
}

#[inline(always)]
fn unpack(data: u64) -> Layout {
    unsafe { transmute::<u64, Layout>(data) }
}

#[repr(C, align(32))]
struct Bucket {
    entries: [AtomicU64; 3],
    evals:   [AtomicU16; 3],
    _padding : u16
}
const _: () = assert!(size_of::<Bucket>() == 32);
impl Bucket {
    fn default() -> Self {
        Self {
            entries:   std::array::from_fn(|_| AtomicU64::new(0)),
            evals:     std::array::from_fn(|_| AtomicU16::new(0)),
            _padding:  0,
        }
    }
}

// =====================================================================================================================//
// TRANSPOSITION TABLE                                                                                                  //
// =====================================================================================================================//
pub struct TranspositionTable {
    table: Vec<Bucket>,
    age:   AtomicU8,
}

impl TranspositionTable {
    pub fn new(size_mb: usize) -> Self {
        let bytes    = size_mb * 1024 * 1024;
        let buckets = (bytes / size_of::<Bucket>()).max(1);
        Self {
            table: (0..buckets).map(|_| Bucket::default()).collect(),
            age:   AtomicU8::new(0),
        }
    }

    #[inline(always)]
    fn index(&self, key: u64) -> usize {
        ((key as u128 * self.table.len() as u128) >> 64) as usize
    }


    // Probe without move validation
    #[inline(always)]
    pub fn probe(&self, key: u64) -> Option<TTEntry> {
        let bucket = &self.table[self.index(key)];
        let key_lower16 = key as u16;

        // find tt entry that corresponds to the u16 key and is occupied.
        for i in 0..3 {
            let layout = unpack(bucket.entries[i].load(Ordering::Relaxed));

            if layout.key != key_lower16 || (layout.info & OCCUPIED_MASK) == 0 { continue }

            let bound = Bound::from_u8(layout.info);

            return Some(TTEntry {
                depth: layout.depth,
                score: layout.score as i32,
                eval: bucket.evals[i].load(Ordering::Relaxed) as i16 as i32,
                bound,
                best_move: decode_move_partial(layout.mv)
            });
        }
        None
    }

    // Probe with move validation
    #[inline(always)]
    pub fn probe_move(&self, key: u64, legal_moves: &[Move]) -> Option<Move> {
        let entry = self.probe(key)?;
        let encoded = encode_move(entry.best_move);
        if encoded == 0 { return None; }
        validate_move(encoded, legal_moves)
    }

    #[inline(always)]
    pub fn store(&self, key: u64, depth: usize, score: i32, eval: i32,
                 bound: Bound, best_move: Option<Move>) {
        let bucket      = &self.table[self.index(key)];
        let current_age = self.age.load(Ordering::Relaxed);
        let key_lower16 = key as u16;

        let mut worst_slot = 0;
        let mut slot_fitness = i32::MAX;

        for (slot,entry) in bucket.entries.iter().enumerate() {
            let layout = unpack(entry.load(Ordering::Relaxed));
            // if same pos or empty, pick is as worst slot immediately
            if layout.info & OCCUPIED_MASK == 0 || layout.key == key_lower16 {
                worst_slot = slot;
                break;
            }
            // Pick the slot with the worst fitness as the one to overwrite
            let age_diff = (current_age.wrapping_sub(layout.info >> 3) & 31) as i32;
            let fitness = layout.depth as i32 - 8 * age_diff;
            if fitness < slot_fitness {
                worst_slot = slot;
                slot_fitness = fitness;
            }
        }

        let target_slot = unpack(bucket.entries[worst_slot].load(Ordering::Relaxed));
        let is_key_match = target_slot.info & OCCUPIED_MASK != 0 && target_slot.key == key_lower16;
        let same_search = target_slot.info >> 3 == current_age;

        if is_key_match && same_search && target_slot.depth > depth as u8 + 2 {
            return;
        }

        //if you are replacing an entry from another position, store a nullmove,
        //if you are replacing an entry from the same position (the stored key matches), preserve the existing move
        // when you fail low and don't have a best move
        let mut best_move_encoded = encode_move(best_move);
        if bound == Bound::Upper && is_key_match {
            best_move_encoded = target_slot.mv;
        }

        let new_entry = Layout {
            key: key_lower16,
            score: score as i16,
            mv: best_move_encoded,
            depth: depth as u8,
            info: (current_age << 3) | OCCUPIED_MASK | (bound as u8)
        };

        bucket.evals[worst_slot].store(eval as i16 as u16, Ordering::Relaxed);
        bucket.entries[worst_slot].store(pack(new_entry), Ordering::Relaxed);
    }

    pub fn clear(&mut self) {
        for bucket in &self.table {
            for i in 0..3 {
                bucket.entries[i].store(0, Ordering::Relaxed);
                bucket.evals[i].store(0, Ordering::Relaxed);
            }
        }
        self.age.store(0, Ordering::Relaxed);
    }

    pub fn tt_occupancy(&self) -> u32 {
        let current_age = self.age.load(Ordering::Relaxed);
        let n_buckets   = self.table.len().min(333);

        let used = self.table[..n_buckets]
            .iter()
            .flat_map(|b| b.entries.iter())
            .filter(|e| {
                let layout = unpack(e.load(Ordering::Relaxed));
                layout.info & OCCUPIED_MASK != 0 && (layout.info >> 3) == current_age
            })
            .count();

        (used * 1000 / (n_buckets * 3)) as u32
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
// extra for Normal: 0=no promo, 1=Q, 2=R, 3=B
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

// Decode a partial move
// Used only internally
#[inline(always)]
pub fn decode_move_partial(encoded: u16) -> Option<Move> {
    if encoded == 0 { return None; }

    let from = Square::new((encoded & 0x3F) as u32);
    let to   = Square::new(((encoded >> 6) & 0x3F) as u32);

    if encoded & (1 << 15) != 0 {
        return if encoded & (1 << 14) != 0 {
            Some(Move::Castle { king: from, rook: to })
        } else {
            Some(Move::EnPassant { from, to })
        };
    }

    let high = encoded >> 12;
    if high >= 5 {
        let role = role_from_index((high - 5) as usize).unwrap_or(Role::Pawn);
        return Some(Move::Put { role, to: from }); 
    }

    let promotion = match high {
        1 => Some(Role::Queen),
        2 => Some(Role::Rook),
        3 => Some(Role::Bishop),
        4 => Some(Role::Knight),
        _ => None,
    };
    Some(Move::Normal { role: Role::Pawn, from, capture: None, to, promotion })
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
pub(crate) fn tt_store(key: u64, ctx: &SearchContext, depth: usize, best_score: i32, eval: i32, bound: Bound, best_move: Option<Move>, ply: usize){
    ctx.tt.store(key, depth, score_to_tt(best_score, ply), eval, bound, best_move);
}
