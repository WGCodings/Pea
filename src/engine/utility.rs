use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use shakmaty::{Chess, fen::Fen, CastlingMode, Move, Position, EnPassantMode, Role};
use shakmaty::zobrist::Zobrist64;
use crate::engine::corrhist::{CorrectionHistoryTable, MajorsAndKingsKey, MaterialKey, MinorsAndKingsKey, PawnKey};
use crate::engine::history::HistoryTables;
use crate::engine::params::Params;
use crate::engine::search::context::{NNUEState, SearchContext, Stack};
use crate::engine::search::ordering::MoveOrdering;
use crate::engine::search::pv::PvTable;
use crate::engine::search::search::SearchStats;
use crate::engine::tt::TranspositionTable;
use crate::engine::types::{MATE_SCORE, MOM, P_A, P_B};
use crate::nnue::network::Network;
use crate::uci::parser::move_to_uci;
use crate::uci::state::UciState;

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

fn material_count(chess: &Chess) -> i32 {
    let board = chess.board();

    (board.pawns()).count() as i32 * 1
        + (board.knights()).count() as i32 * 3
        + (board.bishops()).count() as i32 * 3
        + (board.rooks()).count() as i32 * 5
        + (board.queens()).count() as i32 * 9
}

// ---------------------------------------------------------------------------
// Search output helpers
// ---------------------------------------------------------------------------

pub fn print_search_info(
    ctx:       &SearchContext,
    uci:       &UciState,
    pos:       &Chess,
    depth:     usize,
    mut score:     i32,
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

    let mom = material_count(pos) as f32;

    let wdl_str = if uci.uci_show_wdl{
        format!("wdl {} {} {} ", win_rate(score,mom),draw_rate(score,mom),loss_rate(score,mom))
    } else {
        "".to_string()
    };

    let score_str = if score.abs() > MATE_SCORE - 200 {
        let moves_to_mate = (MATE_SCORE - score.abs() + 1) / 2;
        let sign = if score > 0 { 1 } else { -1 };
        format!("mate {}", sign * moves_to_mate)
    } else {
        if uci.normalize_score{
            score = normalize_score(score,mom);
        }
        format!("cp {}", score)
    };



    println!(
        "info depth {} seldepth {} score {} {}nodes {} nps {} hashfull {} time {} pv {}",
        depth, ctx.stats.seldepth, score_str,wdl_str, nodes, nps, tt_occupancy, elapsed_millis, pv_string,
    );
    pv_line
}

// ---------------------------------------------------------------------------
// Everything for the normalization of the eval and WDL scores
// ---------------------------------------------------------------------------

/// This function normalizes the raw eval based on parameters found from the Stockfish WDL tool
fn normalize_score(score : i32, mom : f32) -> i32{
    ((100.0*score as f32)/(((P_A[0]*mom/MOM + P_A[1])*mom/MOM + P_A[2])*mom/MOM + P_A[3])) as i32
}
/// Winrate in WDL model, pass as promille
pub fn win_rate(score: i32, mom: f32) -> i32 {
    let a = ((P_A[0]  * mom/MOM + P_A[1]) * mom/MOM + P_A[2]) * mom/MOM + P_A[3];
    let b = ((P_B[0]  * mom/MOM + P_B[1]) * mom/MOM + P_B[2]) * mom/MOM + P_B[3];

    (1.0 / (1.0 + ((-(score as f32 - a) / b)).exp())*1000.0).round() as i32
}

/// Loss rate in WDL model
pub fn loss_rate(score: i32, mom: f32) -> i32 {
    win_rate(-score, mom)
}

/// Draw rate in WDL model
pub fn draw_rate(score: i32, mom: f32) -> i32 {
    1000 - win_rate(score, mom) - loss_rate(score, mom)
}

// ---------------------------------------------------------------------------
// Functions to extract pv and stuff
// ---------------------------------------------------------------------------


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


// ---------------------------------------------------------------------------
// SearchContext constructor
// ---------------------------------------------------------------------------

pub fn build_search_context<'a>(
    tt:           &'a TranspositionTable,
    corrhist_pawn: CorrectionHistoryTable<PawnKey>,
    corrhist_material: CorrectionHistoryTable<MaterialKey>,
    corrhist_minor: CorrectionHistoryTable<MinorsAndKingsKey>,
    corrhist_major: CorrectionHistoryTable<MajorsAndKingsKey>,
    history_tables: HistoryTables,
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
        history:              history_tables,
        corrhist_pawn,
        corrhist_material,
        corrhist_minor,
        corrhist_major,
        stack:                Stack {
            moves:       [None; 128],
            evals:       [0; 128],
            double_exts: [0; 128],
        },
        excluded_move:        [None; 128],
    }
}