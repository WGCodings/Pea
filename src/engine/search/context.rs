use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::{Duration, Instant};
use shakmaty::{Chess, Color, Move, Position, Role, Square};
use crate::engine::corrhist::{CorrectionHistoryTable, MajorsAndKingsKey, MaterialKey, MinorsAndKingsKey, PawnKey};
use crate::engine::history::HistoryTables;
use crate::engine::params::Params;
use crate::engine::search::ordering::MoveOrdering;
use crate::engine::search::search::SearchStats;
use crate::engine::tt::TranspositionTable;
use crate::nnue::network::{accumulators_from_position, calculate_index, role_index, Accumulator, Network};

// Keep track of move, eval and nr of double ext per ply.
pub struct Stack {
    pub moves: [Option<Move>; 128],
    pub evals: [i32; 128],
    pub double_exts: [i32; 128],
}
// The searchcontext is passed on during the search and contains parameters, time management, history, tt tables etc
pub struct SearchContext<'a> {
    pub start_time: Instant,
    pub time_limit: Duration,
    pub node_limit : u64,

    pub stop: Arc<AtomicBool>, // Arc to share across threads
    pub node_count: Arc<AtomicU64>,  // node counting over multiple threads
    pub is_main : bool, // Flag to check if this is a main or helper thread

    pub params: &'a Params, // Params struct loaded from yaml or default
    pub ordering: &'a MoveOrdering, // Used for ordering of moves

    pub stats: SearchStats, // Some search statistics

    pub repetition_stack: Vec<u64>, // Stack of moves from previous moves played in the game, important for 3 fold repetition
    pub tt: &'a TranspositionTable, // TT

    pub nnue: NNUEState, // State of NNUE i e accumulators
    pub network: &'a Network, // NNUE network

    pub killers: [[Option<Move>; 3]; 128],
    pub history: HistoryTables,


    // All corrhist tables
    pub corrhist_pawn : CorrectionHistoryTable<PawnKey>,
    pub corrhist_material: CorrectionHistoryTable<MaterialKey>,
    pub corrhist_minor: CorrectionHistoryTable<MinorsAndKingsKey>,
    pub corrhist_major: CorrectionHistoryTable<MajorsAndKingsKey>,
    
    pub stack : Stack,

    pub excluded_move: [Option<Move>; 128], // excluded moves for Singular extensions

}

impl<'a> SearchContext<'a> {

    // =====================================================================================================================//
    // THREEFOLD AND 50 MOVES                                                                                               //
    // =====================================================================================================================//
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
                    return true; // 1-fold repetition
                }
            }
        }

        false
    }
    #[inline(always)]
    pub fn is_50_moves(&self,pos: &Chess) -> bool {
        pos.halfmoves()>= 100
    }

    // =====================================================================================================================//
    // REPETITION MANAGEMENT                                                                                                //
    // =====================================================================================================================//
    #[inline(always)]
    pub fn increase_history(&mut self, hash : u64) {
        self.repetition_stack.push(hash);
    }
    #[inline(always)]
    pub fn decrease_history(&mut self) {
        self.repetition_stack.pop();
    }

    // =====================================================================================================================//
    // KILLER MOVES                                                                                                         //
    // =====================================================================================================================//
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
    pub fn clear_killers_at(&mut self,ply:usize) {
        self.killers[ply][0] = None;
        self.killers[ply][1] = None;
        self.killers[ply][2] = None;
    }



    // =====================================================================================================================//
    // CHECK IF IMPROVING                                                                                                   //
    // =====================================================================================================================//

    #[inline(always)]
    pub fn is_improving(&self, ply: usize) -> bool {
        if ply < 2 {
            return false;
        }

        self.stack.evals[ply] > self.stack.evals[ply - 2]
    }
}

// =====================================================================================================================//
// KEEP TRACK OF CHANGES TO ACCUMULATOR DURING MAKE MOVE                                                                //
// =====================================================================================================================//
pub struct AccumulatorDelta {
    removed: Vec<(usize, usize)>, // (us_idx, them_idx)
    added: Vec<(usize, usize)>,
}

// =====================================================================================================================//
// STATE OF ACCUMULATOR                                                                                                 //
// =====================================================================================================================//

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
            stack: Vec::with_capacity(64),
        }
    }
}

// =====================================================================================================================//
// MAKE AND UNMAKE NNUE ACCUMULATOR                                                                                     //
// =====================================================================================================================//

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

// =====================================================================================================================//
// HELPER FUNCTION TO ACTIVATE AND DEACTIVATE FEATURES                                                                  //
// =====================================================================================================================//

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


