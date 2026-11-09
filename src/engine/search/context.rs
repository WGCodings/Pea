use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::{Duration, Instant};
use shakmaty::{ Board, Chess, Color, Move, Position, Role, Square};
use crate::engine::corrhist::{CorrectionHistoryTable, MajorsAndKingsKey, MaterialKey, MinorsAndKingsKey, PawnKey};
use crate::engine::finny::{FinnyTable};
use crate::engine::history::HistoryTables;
use crate::engine::params::Params;
use crate::engine::search::ordering::MoveOrdering;
use crate::engine::search::search::SearchStats;
use crate::engine::tt::TranspositionTable;
use crate::engine::types::MAX_PLY;
use crate::nnue::network::{calculate_index, get_bucket, role_index, update_into, Accumulator, Network};

// Keep track of move, eval and nr of double ext per ply.
pub struct Stack {
    pub moves: [Option<Move>; MAX_PLY],
    pub evals: [i32; MAX_PLY],
    pub double_exts: [i32; MAX_PLY],
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

    // TODO make fixed length array from it
    pub repetition_stack: Vec<u64>, // Stack of moves from previous moves played in the game, important for 3 fold repetition
    pub tt: &'a TranspositionTable, // TT

    pub nnue: NNUEState, // State of NNUE i e accumulators
    pub network: &'a Network, // NNUE network

    pub killers: [[Option<Move>; 3]; MAX_PLY],
    pub history: HistoryTables,


    // All corrhist tables
    pub corrhist_pawn : CorrectionHistoryTable<PawnKey>,
    pub corrhist_material: CorrectionHistoryTable<MaterialKey>,
    pub corrhist_minor: CorrectionHistoryTable<MinorsAndKingsKey>,
    pub corrhist_major: CorrectionHistoryTable<MajorsAndKingsKey>,
    
    pub stack : Stack,

    pub excluded_move: [Option<Move>; MAX_PLY], // excluded moves for Singular extensions

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

#[derive(Clone, Copy, Default)]
pub struct PieceSquare {
    pub side: u8,
    pub role: u8,
    pub sq: u8,
}

#[derive(Clone, Copy, Default)]
pub struct AccumulatorDelta {
    pub removed: [PieceSquare; 2],
    pub added: [PieceSquare; 2],
    pub n_removed: u8,
    pub n_added: u8,
}

impl AccumulatorDelta {
    fn default() -> Self {
        Self {
            removed: [PieceSquare::default(); 2],
            added: [PieceSquare::default(); 2],
            n_removed: 0, n_added: 0,
        }
    }
    #[inline(always)]
    fn add(&mut self, side: Color, role: Role, sq: Square) {
        self.added[self.n_added as usize] = PieceSquare { side: (side == Color::Black) as u8, role: role_index(role) as u8, sq: sq as u8 };
        self.n_added += 1;
    }
    #[inline(always)]
    fn remove(&mut self, side: Color, role: Role, sq: Square) {
        self.removed[self.n_removed as usize] = PieceSquare { side: (side == Color::Black) as u8, role: role_index(role) as u8, sq: sq as u8 };
        self.n_removed += 1;
    }
}

// =====================================================================================================================//
// STATE OF ACCUMULATOR                                                                                                 //
// =====================================================================================================================//

pub struct NNUEState {
    accs: Vec<[Accumulator; 2]>, // two accs per ply
    computed: Vec<[bool; 2]>, // flag to check if acc in ply ha s been computed or not
    buckets: Vec<[(usize, bool); 2]>, // per ply bucket/mirror info
    refresh: Vec<[bool; 2]>, // flag if needs refresh per ply
    deltas: Vec<AccumulatorDelta>, // adds or removed that need to be performed to accs per ply
    top: usize, // last ply we are currently in
    pub ft: Box<FinnyTable>, // finny table for lazy switching between king buckets accs
}
// Helper to get color based on index. 0 is white 1 is black
#[inline(always)]
fn persp(i: usize) -> Color {
    if i == 0 { Color::White } else { Color::Black }
}

impl NNUEState {
    pub fn new(pos: &Chess, net: &Network) -> Box<Self> {
        let mut state = Box::new(Self {
            accs: vec![[net.feature_bias; 2]; MAX_PLY],
            computed: vec![[false; 2]; MAX_PLY],
            buckets: vec![[(0, false); 2]; MAX_PLY],
            refresh: vec![[false; 2]; MAX_PLY],
            deltas: vec![AccumulatorDelta::default(); MAX_PLY],
            top: 0,
            ft: Box::from(FinnyTable::default(net)),
        });
        for perspective in 0..2 {
            state.buckets[0][perspective] = get_bucket(pos.board(), persp(perspective));
            finny_refresh(pos.board(), net, &mut state, perspective);
        }
        state.computed[0] = [true; 2];
        state
    }

    // Get latest acc on 'top', i e last ply
    #[inline(always)]
    pub fn current(&self) -> (&Accumulator, &Accumulator) {
        debug_assert!(self.computed[self.top] == [true; 2]);
        let a = &self.accs[self.top];
        (&a[0], &a[1])
    }
}

// =====================================================================================================================//
// MAKE AND UNMAKE NNUE ACCUMULATOR                                                                                     //
// =====================================================================================================================//

/// Loop over the accumulator delta stack until you find a clean one. Then do incremental updates from that point back to the current accumulator.
#[inline(always)]
pub fn clean_accumulator(pos: &Chess, net: &Network, state: &mut NNUEState) {
    let top = state.top;
    for perspective in 0..2 {
        if state.computed[top][perspective] {
            continue;
        }
        let mut i = top;
        let mut needs_refresh = false;
        // loop over plies to check if acc is computed already, else it needs refresh
        // if no refresh needed, apply deltas of all plies up to top
        while !state.computed[i][perspective] {
            if state.refresh[i][perspective] {
                needs_refresh = true;
                break;
            }
            i -= 1;
        }
        if needs_refresh {
            finny_refresh(pos.board(), net, state, perspective);
        } else {
            for j in i + 1..=top {
                apply_delta(net, state, j, perspective);
            }
        }
        state.computed[top][perspective] = true;
    }
}

pub fn finny_refresh(board: &Board, net: &Network, state: &mut NNUEState, perspective: usize) {
    let color = persp(perspective);
    let top = state.top;
    let (bucket, mirrored) = state.buckets[top][perspective];
    let entry = state.ft.get_entry(color, bucket, mirrored);

    let mut adds = [0usize; 32];
    let mut removes = [0usize; 32];
    let (mut nr_adds, mut nr_removes) = (0, 0);
    for side in 0..2 {
        let colour = persp(side);
        for (r, role) in Role::ALL.into_iter().enumerate() {
            // Compare all bitboards for changes, then add to adds and removes
            let now = board.by_color(colour) & board.by_role(role);
            let old = entry.piece_bb[side][r];
            for sq in now & !old {
                adds[nr_adds] = calculate_index(side, sq as usize, r, color, bucket, mirrored);
                nr_adds += 1;
            }
            for sq in old & !now {
                removes[nr_removes] = calculate_index(side, sq as usize, r, color, bucket, mirrored);
                nr_removes += 1;
            }
            entry.piece_bb[side][r] = now;
        }
    }
    // add changes to acc
    let entry: *mut Accumulator = &mut entry.acc;
    update_into(entry, entry, &adds[..nr_adds], &removes[..nr_removes], net);
    state.accs[top][perspective] = unsafe { *entry };
}
#[inline(always)]
pub fn make_move_nnue(pos: &Chess, child_pos: &Chess, mv: &Move, state: &mut NNUEState) {
    let us = pos.turn();
    let them = !us;
    let mut d = AccumulatorDelta::default();
    let mut king_moved = false;
    match *mv {
        Move::Normal { role, from, capture, to, promotion } => {
            d.remove(us, role, from);
            d.add(us, promotion.unwrap_or(role), to);
            if let Some(c) = capture {
                d.remove(them, c, to);
            }
            king_moved = role == Role::King;
        }
        Move::EnPassant { from, to } => {
            d.remove(us, Role::Pawn, from);
            d.add(us, Role::Pawn, to);
            d.remove(them, Role::Pawn, Square::from_coords(to.file(), from.rank()));
        }
        Move::Castle { king, rook } => {
            let side = mv.castling_side().unwrap();
            d.remove(us, Role::King, king);
            d.remove(us, Role::Rook, rook);
            d.add(us, Role::King, side.king_to(us));
            d.add(us, Role::Rook, side.rook_to(us));
            king_moved = true;
        }
        Move::Put { .. } => unreachable!(),
    }

    let parent = state.top;
    let ply = parent + 1;

    state.top = ply;
    state.deltas[ply] = d;
    state.computed[ply] = [false; 2];
    state.buckets[ply] = state.buckets[parent];
    state.refresh[ply] = [false; 2];
    if king_moved {
        let p = (us == Color::Black) as usize;
        let nb = get_bucket(child_pos.board(), us);
        if nb != state.buckets[parent][p] {
            state.buckets[ply][p] = nb;
            state.refresh[ply][p] = true;
        }
    }
}

#[inline(always)]
pub fn unmake_move_nnue(_pos: &Chess, _net: &Network, state: &mut NNUEState) {
    debug_assert!(state.top > 0);
    state.top -= 1;
}



// =====================================================================================================================//
// HELPER FUNCTION TO ACTIVATE AND DEACTIVATE FEATURES INTO FUSED UPDATES                                               //
// =====================================================================================================================//
// Update acc at ply x up to date for given perspective.
#[inline(always)]
fn apply_delta(net: &Network, state: &mut NNUEState, ply: usize, perspective: usize) {
    let delta = state.deltas[ply];
    let (bucket, mirrored) = state.buckets[ply][perspective];
    let color = persp(perspective);
    let idx = |f: &PieceSquare| calculate_index(f.side as usize, f.sq as usize, f.role as usize, color, bucket, mirrored);
    let mut adds = [0usize; 2];
    let mut rems = [0usize; 2];
    for k in 0..delta.n_added as usize {
        adds[k] = idx(&delta.added[k]);
    }
    for k in 0..delta.n_removed as usize {
        rems[k] = idx(&delta.removed[k]);
    }
    let (lo, hi) = state.accs.split_at_mut(ply); // get acc of ply-1 and ply
    // then copy ply-1 to ply.
    update_into(&mut hi[0][perspective], &lo[ply - 1][perspective], &adds[..delta.n_added as usize], &rems[..delta.n_removed as usize], net);
    state.computed[ply][perspective] = true;
}


