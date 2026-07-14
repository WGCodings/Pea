use shakmaty::{Chess, Move, Position, Role};
use crate::engine::types::{MAX_HISTORY, MAX_PLY_CONTINUATION_HISTORY};

#[derive(Clone)]
pub struct HistoryTables {
    pub quiet: QuietHistoryTable,
    pub noisy: NoisyHistoryTable,
    pub continuation: ContinuationHistoryTable,
}

impl HistoryTables {

    pub fn new()->Self {
        Self {
            quiet: QuietHistoryTable::new(),
            noisy: NoisyHistoryTable::new(),
            continuation: ContinuationHistoryTable::new(),
        }
    }

    pub fn clear(&mut self) {
        self.quiet.clear();
        self.noisy.clear();
        self.continuation.clear();
    }

    pub fn age_entries(&mut self) {
        self.quiet.age_entries();
        self.noisy.age_entries();
        self.continuation.age_entries();
    }
}

/// Standard gravity histroy update function shared by history tables
#[inline(always)]
fn update_history_value(history_value: &mut i16, bonus: i32) {
    let clamped = bonus.clamp(-MAX_HISTORY, MAX_HISTORY);

    let new = *history_value as i32
        + clamped
        - (*history_value as i32 * clamped.abs() / MAX_HISTORY);

    *history_value = new.clamp(-MAX_HISTORY, MAX_HISTORY) as i16;
}

// =====================================================================================================================//
// QUIET HISTORY                                                                                                        //
// =====================================================================================================================//

#[derive(Clone)]
pub struct QuietHistoryTable {
    pub(crate) table: Box<[[[i16; 64]; 64]; 2]>,
}

impl QuietHistoryTable {
    pub fn new() -> Self {
        Self {
            table: Box::new([[[0; 64]; 64]; 2]),
        }
    }

    pub fn clear(&mut self) {
        self.table.iter_mut().for_each(|x| x.fill([0;64]));
    }

    pub fn age_entries(&mut self) {
        self.table
            .iter_mut()
            .flatten()
            .flatten()
            .for_each(|x| *x /= 2);
    }

    #[inline(always)]
    pub fn get(
        &self,
        pos: &Chess,
        mv: &Move
    ) -> i32 {
        let side = pos.turn() as usize;
        let from = mv.from().unwrap().to_usize();
        let to = mv.to().to_usize();

        self.table[side][from][to] as i32
    }

    #[inline(always)]
    pub fn update(
        &mut self,
        pos: &Chess,
        mv: &Move,
        bonus: i32,
        malus: i32,
        quiets_searched: &[Move]
    ) {
        let from = mv.from().unwrap().to_usize();
        let to   = mv.to().to_usize();
        let side = pos.turn() as usize;

        update_history_value(&mut self.table[side][from][to], bonus);

        // Malus for other quiets
        for &m in quiets_searched {
            if m != *mv {
                let f = m.from().unwrap().to_usize();
                let t = m.to().to_usize();

                update_history_value(&mut self.table[side][f][t], -malus);
            }
        }

    }
}

// =====================================================================================================================//
// NOISY HISTORY                                                                                                        //
// =====================================================================================================================//


#[derive(Clone)]
pub struct NoisyHistoryTable {
    table: Box<[[[i16; 6]; 64]; 6]>,
}

impl NoisyHistoryTable {

    pub fn new() -> Self {
        Self {
            table: Box::new([[[0;6];64];6]),
        }
    }

    pub fn clear(&mut self) {
        self.table.iter_mut().for_each(|x| x.fill([0;6]));
    }

    pub fn age_entries(&mut self) {
        self.table
            .iter_mut()
            .flatten()
            .flatten()
            .for_each(|x| *x /= 2);
    }

    #[inline(always)]
    pub fn get(
        &self,
        pos : &Chess,
        mv: &Move
    ) -> i32 {
        if !mv.is_capture() {
            return 0;
        }

        let board = pos.board();


        let to_sq   = mv.to();
        let from_sq = mv.from().unwrap();
        let captured_piece = board.role_at(to_sq).unwrap_or(Role::Pawn) as usize - 1; // If none it is en passant so captured piece is pawn
        let moved_piece = board.role_at(from_sq).unwrap() as usize - 1;

        self.table[captured_piece][to_sq.to_usize()][moved_piece] as i32
    }

    #[inline(always)]
    pub fn update(
        &mut self,
        pos: &Chess,
        mv: &Move,
        bonus: i32,
        malus : i32,
        noisies_searched: &[Move]
    ) {
        let board = pos.board();

        let to_sq   = mv.to();
        let from_sq = mv.from().unwrap();
        let captured_piece = board.role_at(to_sq).unwrap_or(Role::Pawn) as usize - 1; // If none it is en passant so captured piece is pawn
        let moved_piece = board.role_at(from_sq).unwrap() as usize - 1;

        update_history_value(&mut self.table[captured_piece][to_sq.to_usize()][moved_piece], bonus);

        // Malus for other quiets
        for &m in noisies_searched {
            if m != *mv {

                let to_sq   = m.to();
                let from_sq = m.from().unwrap();
                let captured_piece = board.role_at(to_sq).unwrap_or(Role::Pawn) as usize - 1;
                let moved_piece = board.role_at(from_sq).unwrap() as usize - 1;

                update_history_value(&mut self.table[captured_piece][to_sq.to_usize()][moved_piece], -malus);
            }
        }
    }
}

// =====================================================================================================================//
// CONTINUATION HISTORY                                                                                                 //
// =====================================================================================================================//

#[derive(Clone)]
pub struct ContinuationHistoryTable {
    pub(crate) table: Box<[[[[[i16;64];6];64];6];MAX_PLY_CONTINUATION_HISTORY]>,
}

impl ContinuationHistoryTable {

    pub fn new() -> Self {
        Self {
            table: Box::new(
                [[[[[0;64];6];64];6];MAX_PLY_CONTINUATION_HISTORY]
            ),
        }
    }

    pub fn clear(&mut self) {
        self.table
            .iter_mut()
            .flatten()
            .flatten()
            .flatten()
            .for_each(|x| x.fill(0));
    }

    pub fn age_entries(&mut self) {
        self.table
            .iter_mut()
            .flatten()
            .flatten()
            .flatten()
            .flatten()
            .for_each(|x| *x /= 2);
    }

    #[inline(always)]
    pub fn get(
        &self,
        mv: &Move,
        ply: usize,
        moves: &[Option<Move>],
    ) -> i32 {

        let mut score = 0;


        let to = mv.to().to_usize();
        let piece = mv.role() as usize - 1;

        for i in 0..MAX_PLY_CONTINUATION_HISTORY {
            if ply > i && ((i+1)%2 == 0 || i == 0) {
                if let Some(prev) = moves[ply - 1 - i] {
                    let prev_piece = prev.role() as usize - 1;
                    let prev_to = prev.to() as usize;
                    score += self.table[i][prev_piece][prev_to][piece][to];
                }
            }
        }
        score as i32
    }

    #[inline(always)]
    pub fn update(
        &mut self,
        ply: usize,
        mv: &Move,
        bonus : i32,
        malus : i32,
        quiets_searched: &[Move],
        moves: &[Option<Move>],
    ) {

        self.update_continuation_value(ply, *mv, bonus,moves);

        // malus for continuation history
        for &m in quiets_searched {
            if m != *mv {
                self.update_continuation_value(ply,m,-malus/2,moves);
            }
        }

    }

    // helper for continuation to avoid double code
    #[inline(always)]
    fn update_continuation_value(&mut self, ply : usize, m : Move, bonus : i32, moves: &[Option<Move>]){
        let piece = m.role() as usize - 1;
        let to= m.to() as usize;

        for i in 0..MAX_PLY_CONTINUATION_HISTORY {
            if ply > i && ((1+i)%2 == 0 || i==0){
                if let Some(prev) = moves[ply - 1 - i] {
                    let prev_piece = prev.role() as usize - 1;
                    let prev_to    = prev.to() as usize;

                    update_history_value(&mut self.table[i][prev_piece][prev_to][piece][to], bonus);
                }
            }
        }
    }
}


