
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use shakmaty::{Chess, Color, EnPassantMode, Move, Piece, Position, Role, Square};
use shakmaty::zobrist::Zobrist64;
use crate::engine::params::Params;
use crate::engine::search::ordering::MoveOrdering;
use crate::engine::search::pv::{MultiPv, PvTable};
use crate::engine::search::search::SearchStats;
use crate::engine::tt::TranspositionTable;
use crate::nnue::network::{accumulators_from_position, calculate_index, role_index, Accumulator, Network};

pub struct SearchContext<'a> {
    pub start_time: Instant,
    pub time_limit: Duration,
    pub stop: AtomicBool,

    pub params: &'a Params,
    pub ordering: &'a MoveOrdering,

    pub pv: PvTable,
    pub stats: SearchStats,
    pub multipv: MultiPv,

    pub repetition_stack: Vec<u64>,
    pub tt: &'a mut TranspositionTable,

    pub nnue: NNUEState,
    pub network: &'a Network,

    pub killers: [[Option<Move>; 3]; 128],
    pub history: [[[i32; 64]; 64]; 2], // [side][from][to]
    pub counter_moves: [[[Option<Move>; 64]; 6]; 2],
}

impl<'a> SearchContext<'a> {

    #[inline(always)]
    pub fn is_threefold(&mut self, pos: &Chess) -> bool {

        let mut count = 0;

        let current = self.repetition_stack.last().unwrap_or(&0);
        let len = self.repetition_stack.len();

        if len == 0{
            return false;
        }

        // Avoid underflow
        let start = len.saturating_sub(pos.halfmoves() as usize + 1);

        // Scan backwards skipping last position
        for &hash in self.repetition_stack[start..len-1].iter().rev() {

            if hash == *current {
                count += 1;
                if count >= 1 {
                    return true; // 3-fold repetition
                }
            }
        }

        false
    }
    #[inline(always)]
    pub fn is_50_moves(&self,pos: &Chess) -> bool {
        pos.halfmoves()> 98
    }
    #[inline(always)]
    pub fn _init_history(&mut self, hash : u64) {
        self.repetition_stack.clear();
        self.repetition_stack.push(hash);
    }
    #[inline(always)]
    pub fn increase_history(&mut self, hash : u64) {
        self.repetition_stack.push(hash);
    }
    #[inline(always)]
    pub fn decrease_history(&mut self) {
        self.repetition_stack.pop();
    }
    #[inline(always)]
    pub fn store_killer(&mut self, ply: usize, mv: Move) {
        // Do not store duplicates
        if self.killers[ply][0] == Some(mv) {
            return;
        }

        // Shift old killer
        self.killers[ply][2] = self.killers[ply][1];
        self.killers[ply][1] = self.killers[ply][0];
        self.killers[ply][0] = Some(mv);
    }
    #[inline(always)]
    pub fn is_killer(&self, ply: usize, mv: &Move) -> bool {
        self.killers[ply][0].as_ref() == Some(mv)
            || self.killers[ply][1].as_ref() == Some(mv)
            || self.killers[ply][2].as_ref() == Some(mv)
    }
    #[inline(always)]
    pub fn get_history_score(&self, side: usize, mv : Move) -> i32 {
        let from = mv.from().unwrap().to_usize();
        let to = mv.to().to_usize();
        self.history[side][from][to]
    }
    #[inline(always)]
    pub fn increase_history_bonus(
        &mut self,
        side: usize,
        mv : Move,
        depth: usize,
    ) {
        let bonus = depth * depth;
        let from = mv.from().unwrap().to_usize();
        let to = mv.to().to_usize();

        let entry = &mut self.history[side][from][to];
        *entry += bonus as i32;

        // clamp to prevent overflow
        if *entry > 320000 {
            *entry = 320000;
        }
    }
    #[inline(always)]
    pub fn decrease_history_bonus(
        &mut self,
        side: usize,
        mv : Move,
        depth: usize,
    ) {
        let malus = depth as i32;
        let from = mv.from().unwrap().to_usize();
        let to = mv.to().to_usize();

        let entry = &mut self.history[side][from][to];
        *entry -= malus;

        if *entry < -320000 {
            *entry = -320000;
        }
    }
    #[inline(always)]
    pub fn clear_counter_moves(&mut self) {
        self.counter_moves = [[[None; 64]; 6]; 2];
    }
    #[inline(always)]
    pub fn store_counter_move(&mut self, prev_move: &Move, reply: Move, side: usize) {
        let role_idx = prev_move.role() as usize - 1;
        let to_sq = prev_move.to().to_usize();
        self.counter_moves[side][role_idx][to_sq] = Some(reply);
    }
    #[inline(always)]
    pub fn get_counter_move(&self, prev_move: &Move, side: usize) -> Option<Move> {
        let role_idx = prev_move.role() as usize - 1;
        let to_sq = prev_move.to().to_usize();
        self.counter_moves[side][role_idx][to_sq]
    }







}

pub struct AccumulatorDelta {
    removed: Vec<(usize, usize)>, // (us_idx, them_idx)
    added: Vec<(usize, usize)>,
}

pub struct NNUEState {
    pub us: Accumulator,
    pub them: Accumulator,
    pub stack: Vec<AccumulatorDelta>,
}

impl NNUEState {
    pub fn new<P: Position>(pos: &P, net: &Network) -> Self {
        let (us, them) = accumulators_from_position(pos, net);
        Self {
            us,
            them,
            stack: Vec::with_capacity(64), // To easily undo index enabling
        }
    }
}
#[inline(always)]
pub fn make_move_nnue<P: Position>(
    pos: &P,
    mv: &Move,
    net: &Network,
    state: &mut NNUEState,
) {
    let mut delta = AccumulatorDelta {
        removed: Vec::with_capacity(4),
        added: Vec::with_capacity(4),
    };

    let perspective = pos.turn();
    let board = pos.board();

    match *mv {

        // ============================================================
        // NORMAL MOVE (may include capture + promotion)
        // ============================================================

        Move::Normal { from, to, promotion,.. } => {

            let piece = board.piece_at(from).unwrap();
            let side = if piece.color == Color::White { 0 } else { 1 };
            let from_sq = from.to_usize();
            let to_sq = to.to_usize();

            let piece_type = role_index(piece.role);

            // --- remove moving piece from origin ---
            remove_feature_pair(
                state, net, &mut delta,
                side, from_sq, piece_type, perspective
            );

            // --- handle capture (if any) ---
            if let Some(captured) = board.piece_at(to) {
                let cap_side = if captured.color == Color::White { 0 } else { 1 };
                let cap_type = role_index(captured.role);

                remove_feature_pair(
                    state, net, &mut delta,
                    cap_side, to_sq, cap_type, perspective
                );
            }

            // --- add moved piece (promotion-aware) ---
            let final_role = promotion.unwrap_or(piece.role);
            let final_type = role_index(final_role);

            add_feature_pair(
                state, net, &mut delta,
                side, to_sq, final_type, perspective
            );
        }

        // ============================================================
        // EN PASSANT
        // ============================================================

        Move::EnPassant { from, to } => {

            let piece = board.piece_at(from).unwrap();
            let side = if piece.color == Color::White { 0 } else { 1 };

            let from_sq = from.to_usize();
            let to_sq = to.to_usize();
            let piece_type = role_index(Role::Pawn);

            // remove moving pawn
            remove_feature_pair(
                state, net, &mut delta,
                side, from_sq, piece_type, perspective
            );

            // remove captured pawn (behind target square)
            let captured_sq = Square::from_coords(to.file(), from.rank());
            let cap_sq = captured_sq.to_usize();

            let cap_side = 1 - side;

            remove_feature_pair(
                state, net, &mut delta,
                cap_side, cap_sq, piece_type, perspective
            );

            // add pawn to target square
            add_feature_pair(
                state, net, &mut delta,
                side, to_sq, piece_type, perspective
            );
        }

        // ============================================================
        // CASTLING
        // ============================================================

        Move::Castle { king, rook } => {

            let piece = board.piece_at(king).unwrap();
            let side = if piece.color == Color::White { 0 } else { 1 };

            let king_from = king.to_usize();
            let rook_from = rook.to_usize();

            // determine side of castle
            let kingside = rook.file() > king.file();

            let king_to = if kingside {
                king_from + 2
            } else {
                king_from - 2
            };

            let rook_to = if kingside {
                king_from + 1
            } else {
                king_from - 1
            };

            let king_type = role_index(Role::King);
            let rook_type = role_index(Role::Rook);

            // remove king
            remove_feature_pair(
                state, net, &mut delta,
                side, king_from, king_type, perspective
            );

            // remove rook
            remove_feature_pair(
                state, net, &mut delta,
                side, rook_from, rook_type, perspective
            );

            // add king
            add_feature_pair(
                state, net, &mut delta,
                side, king_to, king_type, perspective
            );

            // add rook
            add_feature_pair(
                state, net, &mut delta,
                side, rook_to, rook_type, perspective
            );
        }
        _ => {}
    }
    state.stack.push(delta);
    // swap accumulators (side to move changes)
    std::mem::swap(&mut state.us, &mut state.them);


}
fn remove_feature_pair(
    state: &mut NNUEState,
    net: &Network,
    delta: &mut AccumulatorDelta,
    side: usize,
    sq: usize,
    piece_type: usize,
    perspective: Color,
) {
    let us_idx = calculate_index(side, sq, piece_type, perspective);
    let them_idx = calculate_index(side, sq, piece_type, !perspective);

    state.us.remove_feature(us_idx, net);
    state.them.remove_feature(them_idx, net);

    delta.removed.push((us_idx, them_idx));
}

fn add_feature_pair(
    state: &mut NNUEState,
    net: &Network,
    delta: &mut AccumulatorDelta,
    side: usize,
    sq: usize,
    piece_type: usize,
    perspective: Color,
) {
    let us_idx = calculate_index(side, sq, piece_type, perspective);
    let them_idx = calculate_index(side, sq, piece_type, !perspective);

    state.us.add_feature(us_idx, net);
    state.them.add_feature(them_idx, net);

    delta.added.push((us_idx, them_idx));
}

#[inline(always)]
pub fn unmake_move_nnue(
    net: &Network,
    state: &mut NNUEState,
) {
    // swap back first
    std::mem::swap(&mut state.us, &mut state.them);

    let delta = state.stack.pop().unwrap();

    for (us_idx, them_idx) in delta.added {
        state.us.remove_feature(us_idx, net);
        state.them.remove_feature(them_idx, net);
    }

    for (us_idx, them_idx) in delta.removed {
        state.us.add_feature(us_idx, net);
        state.them.add_feature(them_idx, net);
    }

}
