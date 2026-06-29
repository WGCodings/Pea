use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use shakmaty::{Chess, fen::Fen, CastlingMode, Move, Position, EnPassantMode, Role};
use shakmaty::zobrist::Zobrist64;
use crate::engine::corrhist::{CorrectionHistoryTable, MaterialKey, MinorsAndKingsKey, PawnKey};
use crate::engine::params::Params;
use crate::engine::search::context::{NNUEState, SearchContext, Stack};
use crate::engine::search::ordering::MoveOrdering;
use crate::engine::search::pv::PvTable;
use crate::engine::search::search::SearchStats;
use crate::engine::tt::TranspositionTable;
use crate::engine::types::{MATE_SCORE, MAX_PLY_CONTINUATION_HISTORY};
use crate::nnue::network::Network;
use crate::uci::parser::move_to_uci;
use crate::uci::state::UciState;
use crate::engine::state::Engine;

// ---------------------------------------------------------------------------
// Position helpers
// ---------------------------------------------------------------------------

pub fn read_position_from_fen(fen_str: &str) -> Option<Chess> {
    let fen: Fen = fen_str.parse().ok()?;
    fen.into_position(CastlingMode::Standard).ok()
}

pub fn role_from_index(idx: usize) -> Option<Role> {
    match idx {
        0 => Some(Role::Pawn),
        1 => Some(Role::Knight),
        2 => Some(Role::Bishop),
        3 => Some(Role::Rook),
        4 => Some(Role::Queen),
        5 => Some(Role::King),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Search output helpers
// ---------------------------------------------------------------------------

pub fn print_search_info(
    ctx:       &SearchContext,
    pos:       &Chess,
    depth:     usize,
    score:     i32,
    elapsed:   Duration,
    pv_table:  PvTable,
) -> Vec<Option<Move>> {
    let elapsed_millis = elapsed.as_millis();
    let elapsed_secs   = elapsed.as_secs_f64();
    let nodes          = (*ctx.node_count).load(Ordering::Relaxed);
    let nps            = if elapsed_secs > 0.0 { (nodes as f64 / elapsed_secs) as u64 } else { 0 };
    let tt_occupancy   = ctx.tt.tt_occupancy();

    let tt_line  = extract_pv_from_tt(pos, ctx.tt, ctx.stats.completed_depth + 20, &ctx.repetition_stack);
    let pv_line  = pv_table.line().to_vec();

    let pv_string = if pv_line.len() >= tt_line.len() {
        pv_to_string(&pv_line)
    } else {
        pv_to_string(&tt_line)
    };

    let score_str = if score.abs() > MATE_SCORE - 200 {
        let moves_to_mate = (MATE_SCORE - score.abs() + 1) / 2;
        let sign = if score > 0 { 1 } else { -1 };
        format!("mate {}", sign * moves_to_mate)
    } else {
        format!("cp {}", score)
    };

    println!(
        "info depth {} seldepth {} score {} nodes {} nps {} hashfull {} time {} pv {}",
        depth, ctx.stats.seldepth, score_str, nodes, nps, tt_occupancy, elapsed_millis, pv_string,
    );
    pv_line
}

pub fn pv_to_string(line: &[Option<Move>]) -> String {
    line.iter()
        .filter_map(|mv| *mv)
        .map(|mv| move_to_uci(&mv))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn extract_pv_from_tt(
    pos:              &Chess,
    tt:               &TranspositionTable,
    max_depth:        usize,
    repetition_stack: &[u64],
) -> Vec<Option<Move>> {
    let mut pv          = Vec::new();
    let mut current_pos = pos.clone();
    let mut visited: Vec<u64> = repetition_stack.to_vec();

    for _ in 0..max_depth {
        if current_pos.is_stalemate()
            || current_pos.is_insufficient_material()
            || current_pos.halfmoves() >= 100
        { break; }

        let hash        = current_pos.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;
        let repetitions = visited.iter().filter(|&&h| h == hash).count();
        if repetitions >= 2 { break; }

        let legal = current_pos.legal_moves();
        if let Some(mv) = tt.probe_move(hash, &legal) {
            pv.push(Some(mv));
            visited.push(hash);
            current_pos.play_unchecked(mv);
        } else {
            break;
        }
    }
    pv
}

pub fn print_bestmove(
    best_move:    Move,
    pv:           &[Option<Move>],
    engine_state: &mut Engine,
    uci_state:    &UciState,
) {
    engine_state.ponder_move = pv.get(1).and_then(|m| *m);

    match (uci_state.ponder_enabled, engine_state.ponder_move) {
        (true, Some(pm)) =>
            println!("bestmove {} ponder {}", move_to_uci(&best_move), move_to_uci(&pm)),
        _ =>
            println!("bestmove {}", move_to_uci(&best_move)),
    }
}

// ---------------------------------------------------------------------------
// SearchContext constructor
// ---------------------------------------------------------------------------

pub fn build_search_context<'a>(
    tt:           &'a TranspositionTable,
    corrhist_pawn: CorrectionHistoryTable<PawnKey>,
    corrhist_material: CorrectionHistoryTable<MaterialKey>,
    corrhist_minor: CorrectionHistoryTable<MinorsAndKingsKey>,
    params:       &'a Params,
    ordering:     &'a MoveOrdering,
    network:      &'a Network,
    rep_stack:    Vec<u64>,
    nnue_state:   NNUEState,
    stop:         Arc<AtomicBool>,
    node_count:   Arc<AtomicU64>,
    is_main:      bool,
    time_limit:   Option<Duration>,
) -> SearchContext<'a> {
    SearchContext {
        start_time:           Instant::now(),
        time_limit:           time_limit.unwrap_or(Duration::from_millis(100)),
        node_limit:           u64::MAX,
        stop,
        node_count,
        is_main,
        params,
        ordering,
        stats:                SearchStats::default(),
        repetition_stack:     rep_stack,
        tt,
        nnue:                 nnue_state,
        network,
        killers:              [[None; 3]; 128],
        history:              [[[0i16; 64]; 64]; 2],
        capture_history:      [[[0i16; 6]; 64]; 6],
        continuation_history: Box::new([[[[[0i16; 64]; 6]; 64]; 6]; MAX_PLY_CONTINUATION_HISTORY]),
        corrhist_pawn,
        corrhist_material,
        corrhist_minor,
        stack:                Stack {
            moves:       [None; 128],
            evals:       [0; 128],
            double_exts: [0; 128],
        },
        excluded_move:        [None; 128],
    }
}