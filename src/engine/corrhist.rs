use std::marker::PhantomData;
use shakmaty::{Bitboard, Chess, Position, Square};

// Special thanks to Jamie Whiting, author of Akimbo for inspiration and code examples. Only slight modifications were made.

const SIZE: usize = 16384;
const GRAIN: i32 = 256;
const SCALE: i32 = 256;
const MAX: i32 = 8192;

// Key can be different for other kinds of corrhist tables
pub trait CorrHistKey {
    fn key(pos: &Chess) -> usize;
}

// Shared struct - generic over the key strategy
#[derive(Clone)]
pub struct CorrectionHistoryTable<K: CorrHistKey> {
    pub(crate) table: Box<[[i32; SIZE]; 2]>,
    _marker: PhantomData<K>,
}

impl<K: CorrHistKey> Default for CorrectionHistoryTable<K> {
    fn default() -> Self {
        Self {
            table: vec![[0i32; SIZE]; 2]
                .into_boxed_slice()
                .try_into()
                .unwrap(),
            _marker: PhantomData
        }
    }
}

// Make Corrhisttable for different keys - different  correction histories
impl<K: CorrHistKey> CorrectionHistoryTable<K> {

    // Age entries, reduce importance
    pub fn age_entries(&mut self) {
        self.table.iter_mut().flatten().for_each(|x| *x /= 2);
    }
    // Clear table
    pub fn clear(&mut self) {
        self.table.iter_mut().for_each(|t| t.fill(0));
    }

    // Update the correction history based on the key and difference between static eval and score
    pub fn update_correction_history(&mut self, pos: &Chess, depth: i32, eval_diff: i32) {
        let entry = &mut self.table[usize::from(pos.turn())][K::key(pos)];
        let scaled_diff = eval_diff * GRAIN;
        let new_weight = 16.min(depth + 1);
        let update = *entry * (SCALE - new_weight) + scaled_diff * new_weight;
        *entry = i32::clamp(update / SCALE, -MAX, MAX);
    }

    // If key matches, correct raw eval with correction
    pub fn correct_evaluation(&self, pos: &Chess, raw_eval: i32) -> i32 {
        let entry = self.table[usize::from(pos.turn())][K::key(pos)];
        raw_eval + entry / GRAIN
    }
}

// Pawn correction history
#[derive(Clone)]
pub struct PawnKey;
impl CorrHistKey for PawnKey {
    fn key(pos: &Chess) -> usize {
        (pawnhash(pos) % SIZE as u64) as usize
    }
}


static PAWN_ZOBRIST: [u64; 64] = generate_pawn_zobrist();

const fn generate_pawn_zobrist() -> [u64; 64] {
    let mut table = [0u64; 64];
    let mut seed: u64 = 0x9E3779B97F4A7C15;

    let mut sq = 0;
    while sq < 64 {
        seed = splitmix64(seed);
        table[sq] = seed;
        sq += 1;
    }
    table
}

const fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

// Helper function to calculate hashes (should be done later incrementally)
fn pawnhash(pos: &Chess) -> u64 {
    let mut pawns = pos.board().pawns();
    let mut hash = 0u64;

    while !pawns.is_empty(){
        let lsb = Bitboard(pawns.0 & pawns.0.wrapping_neg());
        let square = Square::new(lsb.0.trailing_zeros());
        hash ^= PAWN_ZOBRIST[square as usize];
        pawns.discard(square);
    }

    hash
}
