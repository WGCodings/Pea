use std::marker::PhantomData;
use shakmaty::{Chess, Position};
use crate::engine::hash::Hash;

// Special thanks to Jamie Whiting, author of Akimbo for inspiration and code examples. Only slight modifications were made.

const SIZE: usize = 16384;
const GRAIN: i32 = 256;
const SCALE: i32 = 256;
const MAX: i32 = 32*SCALE; // max correction is thus 32cp TODO experiment with larger MAX

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
    pub fn correct_evaluation(&self, pos: &Chess) -> i32 {
        let entry = self.table[usize::from(pos.turn())][K::key(pos)];
        entry / GRAIN
    }
}

// Pawn correction history
#[derive(Clone)]
pub struct PawnKey;
impl CorrHistKey for PawnKey {
    fn key(pos: &Chess) -> usize {
        let hash = Hash::pawnhash(pos);
        hash.0 as usize % SIZE
    }
}

// King bucket and non-pawn correction history
#[derive(Clone)]
pub struct MaterialKey;
impl CorrHistKey for MaterialKey {
    fn key(pos: &Chess) -> usize {
        let hash = Hash::materialhash(pos);
        hash.0 as usize % SIZE
    }
}