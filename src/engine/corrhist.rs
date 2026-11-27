use std::marker::PhantomData;
use shakmaty::{Chess, Move, Position};
use shakmaty::Color::{Black, White};
use crate::engine::hash::Hash;
use crate::engine::params::Params;
// Special thanks to Jamie Whiting, author of Akimbo for inspiration and code examples. Only slight modifications were made.

const SIZE: usize = 16384;
const GRAIN: i32 = 256;

#[derive(Clone)]
pub struct CorrHistTables {
    pub material: CorrectionHistoryTable<MaterialKey>,
    pub pawn: CorrectionHistoryTable<PawnKey>,
    pub minors_kings: CorrectionHistoryTable<MinorsAndKingsKey>,
    pub majors_kings : CorrectionHistoryTable<MajorsAndKingsKey>,
    pub nonpawn_black: CorrectionHistoryTable<NonPawnBlackKey>,
    pub nonpawn_white: CorrectionHistoryTable<NonPawnWhiteKey>,
    pub cont2: CorrectionHistoryTable<Cont2Key>


}

impl CorrHistTables {
    pub(crate) fn clear(&mut self) {
        self.material.clear();
        self.pawn.clear();
        self.minors_kings.clear();
        self.majors_kings.clear();
        self.nonpawn_black.clear();
        self.nonpawn_white.clear();
        self.cont2.clear();
    }

    pub(crate) fn update_correction_history(&mut self, pos: &Chess, depth: i32, diff: i32, move_stack :&[Option<Move>], ply : usize) {
        self.material.update_correction_history(pos, depth, diff,move_stack,ply);
        self.pawn.update_correction_history(pos, depth, diff,move_stack,ply);
        self.minors_kings.update_correction_history(pos, depth, diff,move_stack,ply);
        self.majors_kings.update_correction_history(pos, depth, diff,move_stack,ply);
        self.nonpawn_black.update_correction_history(pos, depth, diff,move_stack,ply);
        self.nonpawn_white.update_correction_history(pos, depth, diff,move_stack,ply);
        self.cont2.update_correction_history(pos, depth, diff, move_stack, ply);
    }
    pub(crate) fn age_entries(&mut self) {
        self.material.age_entries();
        self.pawn.age_entries();
        self.minors_kings.age_entries();
        self.majors_kings.age_entries();
        self.nonpawn_black.age_entries();
        self.nonpawn_white.age_entries();
        //self.cont2.age_entries();
    }

    pub(crate) fn correct_evaluation(&self, pos: &Chess, move_stack :&[Option<Move>], ply : usize) -> i32 {
        self.pawn.correct_evaluation(pos,move_stack,ply)
            + self.minors_kings.correct_evaluation(pos,move_stack,ply)
            + self.majors_kings.correct_evaluation(pos,move_stack,ply)
            + self.material.correct_evaluation(pos,move_stack,ply)
            + self.nonpawn_white.correct_evaluation(pos,move_stack,ply)
            + self.nonpawn_black.correct_evaluation(pos,move_stack,ply)
            + self.cont2.correct_evaluation(pos, move_stack, ply)
    }
    pub fn new(params : &Params)->Self {
        Self {
            material: CorrectionHistoryTable::new(256,params.corrhist_material),
            pawn: CorrectionHistoryTable::new(256,params.corrhist_pawn),
            minors_kings: CorrectionHistoryTable::new(256,params.corrhist_minors),
            majors_kings: CorrectionHistoryTable::new(256,params.corrhist_majors),
            nonpawn_white : CorrectionHistoryTable::new(256,params.corrhist_npw),
            nonpawn_black : CorrectionHistoryTable::new(256,params.corrhist_npb),
            cont2: CorrectionHistoryTable::new(128, params.corrhist_cont2),
        }
    }
}

// Key can be different for other kinds of corrhist tables
pub trait CorrHistKey {
    fn key(pos: &Chess, move_stack :&[Option<Move>], ply : usize) -> Option<usize>;
}

#[derive(Clone)]
pub struct CorrectionHistoryTable<K: CorrHistKey> {
    pub(crate) table: Box<[[i32; SIZE]; 2]>,
    pub scale: i32,
    pub max: i32,
    _marker: PhantomData<K>,
}

impl<K: CorrHistKey> CorrectionHistoryTable<K>{
    pub(crate) fn new(scale : i32, max_cp : i32) -> Self {
        Self {
            table: vec![[0i32; SIZE]; 2]
                .into_boxed_slice()
                .try_into()
                .unwrap(),
            scale,
            max: max_cp*scale,
            _marker: PhantomData
        }
    }
    // Age entries, reduce importance
    pub fn age_entries(&mut self) {
        self.table.iter_mut().flatten().for_each(|x| *x /= 2);
    }
    // Clear table
    pub fn clear(&mut self) {
        self.table.iter_mut().for_each(|t| t.fill(0));
    }

    // Update the correction history based on the key and difference between static eval and score
    pub fn update_correction_history(&mut self, pos: &Chess, depth: i32, eval_diff: i32, move_stack :&[Option<Move>], ply : usize) {
        let Some(key) = K::key(pos, move_stack, ply) else { return };
        let entry = &mut self.table[usize::from(pos.turn())][key];
        let scaled_diff = eval_diff * GRAIN;
        let new_weight = 16.min(depth + 1);
        let update = *entry * (self.scale - new_weight) + scaled_diff * new_weight;
        *entry = i32::clamp(update / self.scale, -self.max, self.max);
    }

    // If key matches, correct raw eval with correction
    pub fn correct_evaluation(&self, pos: &Chess, move_stack :&[Option<Move>], ply : usize) -> i32 {
        match K::key(pos, move_stack, ply) {
            Some(key) => self.table[usize::from(pos.turn())][key] / GRAIN,
            None      => 0,
        }
    }
}

// Pawn correction history
#[derive(Clone)]
pub struct PawnKey;
impl CorrHistKey for PawnKey {
    fn key(pos: &Chess, _: &[Option<Move>], _: usize) -> Option<usize> {
        let hash = Hash::pawnhash(pos);
        Some(hash.0 as usize % SIZE)
    }
}

// material corrhistkey
#[derive(Clone)]
pub struct MaterialKey;
impl CorrHistKey for MaterialKey {
    fn key(pos: &Chess, _: &[Option<Move>], _: usize) -> Option<usize> {
        let hash = Hash::materialhash(pos);
        Some(hash.0 as usize % SIZE)
    }
}

#[derive(Clone)]
pub struct MinorsAndKingsKey;
impl CorrHistKey for MinorsAndKingsKey {
    fn key(pos: &Chess, _: &[Option<Move>], _: usize) -> Option<usize> {
        let hash = Hash::minors_and_kings_hash(pos);
        Some(hash.0 as usize % SIZE)
    }
}

#[derive(Clone)]
pub struct MajorsAndKingsKey;
impl CorrHistKey for MajorsAndKingsKey {
    fn key(pos: &Chess, _: &[Option<Move>], _: usize) -> Option<usize> {
        let hash = Hash::majors_and_kings_hash(pos);
        Some(hash.0 as usize % SIZE)
    }
}

#[derive(Clone)]
pub struct NonPawnWhiteKey;
impl CorrHistKey for NonPawnWhiteKey {
    fn key(pos: &Chess, _: &[Option<Move>], _: usize) -> Option<usize> {
        let hash = Hash::nonpawn_hash(pos, White);
        Some(hash.0 as usize % SIZE)
    }
}
#[derive(Clone)]
pub struct NonPawnBlackKey;

impl CorrHistKey for NonPawnBlackKey {
    fn key(pos: &Chess, _: &[Option<Move>], _: usize) -> Option<usize>{
        let hash = Hash::nonpawn_hash(pos, Black);
        Some(hash.0 as usize % SIZE)
    }
}

#[derive(Clone)]
pub struct Cont2Key;
impl CorrHistKey for Cont2Key {
    fn key(_: &Chess, moves: &[Option<Move>], ply: usize) -> Option<usize> {
        if ply < 2 { return None; }
        let (prev, last) = (moves[ply - 2]?, moves[ply - 1]?);
        let a = (prev.role() as usize - 1) * 64 + prev.to() as usize;
        let b = (last.role() as usize - 1) * 64 + last.to() as usize;
        Some((a * 384 + b) % SIZE)
    }
}