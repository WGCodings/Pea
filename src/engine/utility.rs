use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use shakmaty::{Chess, fen::Fen, CastlingMode, Move, Color, Position, EnPassantMode};
use shakmaty::zobrist::Zobrist64;
use crate::engine::search::context::{NNUEState, SearchContext, Stack};
use crate::engine::search::ordering::MoveOrdering;
use crate::engine::search::pv::PvTable;
use crate::engine::search::search::SearchStats;
use crate::engine::state::EngineState;
use crate::engine::tt::TranspositionTable;
use crate::engine::types::{MAX_PLY_CONTINUATION_HISTORY, PIECE_VALUES};
use crate::nnue::network::Network;
use crate::uci::parser::move_to_uci;

/// Reads a FEN string and converts it to a `Chess` position.
pub fn read_position_from_fen(fen_str: &str) -> Option<Chess> {
    let fen: Fen = fen_str.parse().ok()?; // Parse the FEN string
    fen.into_position(CastlingMode::Standard).ok() // Convert to `Chess` position
}

pub fn print_search_info(ctx: &SearchContext, pos : &Chess, depth: usize, score: i32, elapsed: Duration) {
    let elapsed_millis = elapsed.as_millis();
    let elapsed_secs = elapsed.as_secs_f64();
    let nps = if elapsed_secs > 0.0 { (ctx.stats.nodes as f64 / elapsed_secs) as u64 } else { 0 };

    let tt_occupancy = ctx.tt.tt_occupancy();

    // After depth loop, extract full PV from TT
    let tt_pv = extract_pv_from_tt(pos, ctx.tt, ctx.stats.completed_depth+20,&ctx.repetition_stack.as_slice());

    let pv_string = pv_to_string(&tt_pv.as_slice());



    println!(
        "info depth {} seldepth {} score cp {} nodes {} nps {} hashfull {} time {} pv {}",
        depth,
        ctx.stats.seldepth,
        score,
        ctx.stats.nodes,
        nps,
        tt_occupancy,
        elapsed_millis,
        pv_string,
    );
}

pub fn pv_to_string(line: &[Option<Move>]) -> String {
    line.iter()
        .filter_map(|mv| *mv)
        .map(|mv| move_to_uci(&mv))
        .collect::<Vec<_>>()
        .join(" ")
}


pub fn build_search_context<'a>(
    engine_state: &'a mut EngineState,
    ordering: &'a MoveOrdering,
    network: &'a Network,
    time_limit: Option<Duration>,
) -> SearchContext<'a> {
    SearchContext {
        start_time: Instant::now(),
        time_limit: time_limit.unwrap_or(Duration::from_millis(100)),
        stop: AtomicBool::new(false),
        params: &engine_state.params,
        ordering,
        stats: SearchStats::default(),
        repetition_stack: engine_state.repetition_stack.to_vec(),
        tt: &mut engine_state.tt,
        nnue: NNUEState::new(&engine_state.position, network),
        network,
        killers: [[None; 3]; 128],
        history: [[[0; 64]; 64]; 2],
        capture_history: [[[0; 6]; 64]; 2],
        continuation_history: Box::new([[[[[0; 64]; 6]; 64]; 6]; MAX_PLY_CONTINUATION_HISTORY]),
        stack: Stack { moves: [None; 128], evals: [0; 128], double_exts: [0; 128] },
        excluded_move: [None; 128],
    }
}

pub fn extract_pv_from_tt(
    pos: &Chess,
    tt: &TranspositionTable,
    max_depth: usize,
    repetition_stack: &[u64],  // ← pass game history
) -> Vec<Option<Move>> {
    let mut pv = Vec::new();
    let mut current_pos = pos.clone();
    let mut visited: Vec<u64> = repetition_stack.to_vec();  // start with game history

    for _ in 0..max_depth {
        if current_pos.is_stalemate()
            || current_pos.is_insufficient_material()
            || current_pos.halfmoves() >= 100
        {
            break;
        }

        let hash = current_pos.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;

        // Check for repetition against full game history + pv so far
        let repetitions = visited.iter().filter(|&&h| h == hash).count();
        if repetitions >= 2 {
            break;
        }

        if let Some(entry) = tt.probe(hash) {
            if let Some(mv) = entry.best_move {
                let legal = current_pos.legal_moves();
                if legal.contains(&mv) {
                    pv.push(Some(mv));
                    visited.push(hash);
                    current_pos.play_unchecked(mv);
                } else {
                    break;
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }
    pv
}