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

    fn peek(&self) -> (u64, u8, u8) {
        let data = self.data.load(Ordering::Relaxed);
        let stored_key  = self.key.load(Ordering::Relaxed) ^ data;
        let layout: Layout = unsafe { transmute::<u64, Layout>(data) };

        (stored_key, layout.depth, layout.info >> 3)
    }
}

#[repr(C, align(64))]
struct Bucket {
    entries: [PackedEntry; 4],
}
impl Bucket {
    fn default() -> Self {
        Self { entries: std::array::from_fn(|_| PackedEntry::default()) }
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
        bucket.entries.iter().find_map(|e| e.load(key))
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

        let mut worst_slot = 0;
        let mut slot_fitness = i32::MAX;

        for (slot,entry) in bucket.entries.iter().enumerate() {
            let (slot_key, slot_depth, slot_age) = entry.peek();
            // if same pos or empty, pick is as worst slot immediately
            if slot_key == key || slot_key ==0 {
                worst_slot = slot;
                break;
            }
            // Pick the slot with the worst fitness as the one to overwrite
            let age_diff = (current_age.wrapping_sub(slot_age) & 31) as i32;
            let fitness = slot_depth as i32- 8 * age_diff;
            if fitness < slot_fitness {
                worst_slot = slot;
                slot_fitness = fitness;
            }
        }

        let target_slot = &bucket.entries[worst_slot];

        let (old_key, old_depth, old_age) = target_slot.peek();

        let same_search = old_age == current_age;

        if old_key == key && same_search && old_depth > depth as u8 + 2 {
            return;
        }

        //if you are replacing an entry from another position, store a nullmove,
        //if you are replacing an entry from the same position (the stored key matches), preserve the existing move
        // when you fail low and don't have a best move
        let mut best_move = best_move;
        if bound == Bound::Upper{
            if let Some(existing) = target_slot.load(key) {
                if existing.key == key
                {
                    best_move = existing.best_move;
                }
                else if existing.key != key{
                    best_move = None;
                }
            }
        }
        target_slot.store(&TTEntry {
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
        for bucket in &self.table {
            for (_,entry) in bucket.entries.iter().enumerate() {
                entry.key.store(0,  Ordering::Relaxed);
                entry.data.store(0, Ordering::Relaxed);
            }
        }
        self.age.store(0, Ordering::Relaxed);
    }

    pub fn tt_occupancy(&self) -> u32 {
        let current_age = self.age.load(Ordering::Relaxed);
        let n_buckets   = self.table.len().min(250);

        let used = self.table[..n_buckets]
            .iter()
            .flat_map(|b| b.entries.iter())
            .filter(|e| {
                let (key, _, age) = e.peek();
                key != 0 && age == current_age
            })
            .count();

        (used * 1000 / (n_buckets * 4)) as u32
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
