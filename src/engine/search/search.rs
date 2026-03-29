use std::cmp;
use std::sync::atomic::Ordering;

use std::time::{Duration, Instant};
use shakmaty::{Bitboard, Chess, EnPassantMode, Move, Position};
use shakmaty::zobrist::{Zobrist64};
use crate::engine::eval::evaluate;
use crate::engine::search::context::{make_move_nnue, unmake_move_nnue, SearchContext};
use crate::engine::search::pv::PvTable;
use crate::engine::search::see::see;
use crate::engine::time_manager::TimeManager;
use crate::engine::tt::{ encode_move, score_from_tt, tt_probe, tt_store, validate_move, Bound};
use crate::engine::types::{DRAW_SCORE, MATE_SCORE, MAX_INF, MIN_INF};
use crate::engine::utility::{print_search_info};

pub struct SearchStats {
    pub nodes: u64,
    pub depth_sum: u64,
    pub depth_samples: u64,
    pub seldepth: u32,
    pub completed_depth: usize,
    pub duration: Duration,
    pub singular_extensions : u32
}

impl SearchStats {
    pub(crate) fn default() -> SearchStats {
        Self {
            nodes: 0,
            depth_sum: 0,
            depth_samples: 0,
            seldepth: 0,
            completed_depth: 0,
            duration: Duration::ZERO,
            singular_extensions: 0,
        }
    }
}

pub fn search(pos: &Chess, ctx: &mut SearchContext, max_depth: usize, time_remaining: Option<Duration>) -> (i32, Move, Vec<Option<Move>>) {
    let start_time = Instant::now();
    let base_time = time_remaining.unwrap();

    ctx.time_limit = base_time;
    ctx.start_time = start_time;
    (*ctx.stop).store(false, Ordering::Relaxed);
    ctx.tt.increment_age();

    let mut tm = TimeManager::new(base_time, start_time);
    let mut best_score = MIN_INF;
    let mut best_move = None;
    let mut pv = PvTable::new();
    let mut latest_pv;
    let mut prev_score = 0;
    let mut tt_pv = vec![];


    for depth in 1..=max_depth {
        pv.clear();

        if tm.should_stop() { break; }

        let score = if depth >= ctx.params.aspw_min_depth as usize {
            aspiration_search(pos, ctx, depth, prev_score, &mut pv)
        } else {
            negamax(pos, ctx, depth, 0, MIN_INF, MAX_INF, true, &mut pv)
        };

        if (*ctx.stop).load(Ordering::Relaxed) { break; }

        tm.update(score, pv.best_move());


        ctx.time_limit = tm.current_limit;

        best_score = score;
        prev_score = score;
        best_move = pv.best_move();
        latest_pv = pv; // why use pv table if we get pv from tt? verification?
        ctx.stats.completed_depth = depth;

        if ctx.is_main{
            tt_pv = print_search_info(ctx, pos, depth, best_score, tm.elapsed(),latest_pv);
        }

    }

    ctx.stats.duration = tm.elapsed();
    (best_score, best_move.expect("No legal move found"), tt_pv)
}



#[inline(always)]
pub fn negamax(
    pos: &Chess,
    ctx: &mut SearchContext,
    mut depth: usize,
    ply: usize,
    mut alpha: i32,
    beta: i32,
    do_null : bool,
    pv: &mut PvTable,
) -> i32 {
    (*ctx.node_count).fetch_add(1, Ordering::Relaxed);
    ctx.stats.nodes += 1;
    ctx.stats.seldepth = cmp::max(ply as u32, ctx.stats.seldepth);
    ctx.stats.depth_sum += ply as u64;
    ctx.stats.depth_samples += 1;

    let is_root = ply == 0;
    let in_check = pos.is_check();
    let is_pv = beta-alpha >1;
    let is_excluded = ctx.excluded_move[ply].is_some();

    let original_alpha = alpha;
    let mut best_score = MIN_INF;
    let mut best_move = None;

    if ply > 0 {
        ctx.stack.double_exts[ply] = ctx.stack.double_exts[ply-1];
    }

    check_time(ctx);

    if pos.is_checkmate() {
        return -MATE_SCORE + ply as i32;
    }

    if (ctx.is_threefold(pos) || ctx.is_50_moves(pos) || pos.is_stalemate() || pos.is_insufficient_material()) && !is_root {
        return DRAW_SCORE;
    }

    if in_check && ply < 63 && depth <= 2 {
        depth += 1;
    }

    if (*ctx.stop).load(Ordering::Relaxed){ return DRAW_SCORE; }


    if depth <= 0 {
        (*ctx.node_count).fetch_sub(1, Ordering::Relaxed);
        ctx.stats.nodes -= 1;
        return quiescence(pos, ctx, alpha, beta,ply);
    }

    let hash = pos.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;
    let mut score;

    // =====================================================================================================================//
    // TT PROBE                                                                                                             //
    // =====================================================================================================================//

    let tt_entry = if !is_excluded {
        ctx.tt.probe(hash)
    } else {
        None
    };


    // TODO ADD CHECK IF PLY > MAX PLY MAYBE
    if !is_root && !is_excluded && tt_entry.is_some() {
        if let Some(score) = tt_entry.as_ref().and_then(|e| e.try_score(depth, alpha, beta, ply)) {
            return score;
        }
    }

    let static_eval = if is_excluded {
        ctx.stack.evals[ply]
    } else if let Some(e) = &tt_entry {
        e.eval
    } else {
        evaluate(pos, ctx.network, &ctx.nnue.us, &ctx.nnue.them)
    };

    ctx.stack.evals[ply] = static_eval;

    // Safety
    if ply > 63{
        return static_eval;
    }

    ctx.clear_killers_at(ply+1);


    let do_pruning = minors_or_majors(pos).count() >0 && !is_excluded;
    let improving = ctx.is_improving(ply);

    let mut can_futility_prune = false;


    // =====================================================================================================================//
    // REVERSE FUTILITY PRUNING                                                                                             //
    // =====================================================================================================================//
    let futility = (ctx.params.rfp_scaling as usize* depth) as i32 + ctx.params.rfp_improving_scaling as i32 * !improving as i32;
    if do_pruning && !is_pv && !in_check && depth <= ctx.params.rfp_max_depth as usize && !is_root && static_eval - futility   >=beta {
        return (static_eval + beta)/2;
    }

    // =====================================================================================================================//
    // STATIC NULL MOVE PRUNING                                                                                             //
    // =====================================================================================================================//
    if  do_pruning && !in_check && !is_pv && beta.abs() < MATE_SCORE {
        let score_margin = ctx.params.snmp_scaling as i32 * depth as i32;
        if static_eval-score_margin >= beta {
            return static_eval-score_margin
        }
    }

    // =====================================================================================================================//
    // NULL MOVE PRUNING                                                                                                    //
    // =====================================================================================================================//
    let nmp_margin : i32 = -ctx.params.nmp_margin as i32 + ctx.params.nmp_scaling as i32 * depth as i32 + ctx.params.nmp_improving_scaling as i32 * improving as i32 ;
    if  do_pruning && !in_check && !is_pv && !is_root &&
        static_eval + nmp_margin >= beta &&
        do_null && minors_or_majors(pos).count() >0 &&
        depth >=ctx.params.nmp_min_depth as usize {

        let mut reduction = (ctx.params.nmp_base_reduction as usize + depth/ctx.params.nmp_reduction_scaling as usize).min(depth);

        reduction += 2*improving as usize;

        reduction = reduction.clamp(1,depth);

        std::mem::swap(&mut ctx.nnue.us, &mut ctx.nnue.them);

        let child_pos = pos.clone().swap_turn().unwrap();

        let hash_child = child_pos.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;
        ctx.increase_history(hash_child);

        let score = -negamax(&child_pos, ctx, depth - reduction, ply + 1, -beta, -beta + 1, false, &mut PvTable::new());

        std::mem::swap(&mut ctx.nnue.us, &mut ctx.nnue.them);
        ctx.decrease_history();

        if (*ctx.stop).load(Ordering::Relaxed){ return DRAW_SCORE;}

        if score >= beta && score.abs() < MATE_SCORE {
            return beta;
        }
    }


    // =====================================================================================================================//
    // RAZORING                                                                                                             //
    // =====================================================================================================================//
    // TODO ADD IMRPOVING HEURISTIC TO MARGIN
    if  do_pruning && !in_check && !is_pv
        && depth <= ctx.params.raz_max_depth as usize
        && static_eval + ctx.params.raz_thr as i32 *(depth as i32)  + improving as i32 * 0 < alpha
    {
        let razor_score = quiescence(pos,ctx,alpha,beta,ply);
        if razor_score <= alpha{
            return razor_score;
        }
    }

    // =====================================================================================================================//
    // FUTILITY PRUNING PART 1                                                                                              //
    // =====================================================================================================================//
    if  depth <= ctx.params.fp_max_depth as usize && !is_pv && !in_check && alpha.abs() < MATE_SCORE && beta.abs() < MATE_SCORE && !is_excluded{
        let margin = ctx.params.fp_base as i32+ depth as i32 * ctx.params.fp_scaling as i32 + ctx.params.fp_improving_margin as i32 * improving as i32;
        can_futility_prune = static_eval+margin <= alpha;
    }

    let mut moves = pos.legal_moves();


    // best move is encoded in the tt as 16 bits but does not contain all info,
    // so when we need it we reconstruct the tt move based on all available legal moves
    let tt_move = tt_entry.as_ref().and_then(|e| {
        let encoded = encode_move(e.best_move);
        if encoded == 0 { return None; }
        validate_move(encoded,&moves)
    });


    // =====================================================================================================================//
    // INTERNAL ITERATIVE REDUCTION                                                                                         //
    // =====================================================================================================================//
    if tt_move.is_none() && depth >= ctx.params.iir_min_depth as usize {
        depth -= 1;
    }

    // =====================================================================================================================//
    // SINGULAR EXTENSION INFO                                                                                              //
    // =====================================================================================================================//

    let se_info: Option<(Move, i32)> = if !is_root && !is_excluded && depth >= ctx.params.se_min_depth as usize{
        tt_move.as_ref().and_then(|tt_mv| {
            tt_entry.as_ref().and_then(|entry| {
                let tt_depth_ok = entry.depth as usize + ctx.params.se_depth_ok as usize>= depth;
                let tt_score = score_from_tt(entry.score, ply);
                let not_mate = tt_score.abs() < MATE_SCORE - 100;
                let is_lower_bound = matches!(entry.bound, Bound::Lower | Bound::Exact);

                if tt_depth_ok && not_mate && is_lower_bound {
                    Some((tt_mv.clone(), tt_score))
                } else {
                    None
                }
            })
        })
    } else {
        None
    };


    let mut moves_searched : i32 = 0;
    let mut local_pv = PvTable::new();
    let mut quiets_searched: Vec<Move> = Vec::new();
    let mut tacticals_searched: Vec<Move> = Vec::new();

    ctx.ordering.order_moves(pos, ctx,  tt_move.as_ref(), &ctx.killers[ply], ply, &mut moves);


    for mv in moves {
        // Skip excluded move (used during singular extension search)
        if ctx.excluded_move[ply] == Some(mv) {
            continue;
        }

        let see= see(pos,mv);

        local_pv.clear();

        let is_capture = mv.is_capture();
        let is_quiet = !is_capture && !mv.is_promotion() ;


        moves_searched +=1;

        // TODO TEST IF PROMOTIONS ARE BEST ADDED TO QUIETS OR TACTICALS
        // Punish bad quiets and tacticals
        if is_quiet {
            // penalize quiets that fail low
            quiets_searched.push(mv);
        }
        // malus for captures that did not fail high
        if !is_quiet {
            tacticals_searched.push(mv);
        }

        // =====================================================================================================================//
        // LATE MOVE PRUNING                                                                                                    //
        // =====================================================================================================================//
        let lmp_moves= ctx.params.lmp_base as i32 +depth as i32 * ctx.params.lmp_lin_scaling as i32 + depth as i32 * depth as i32 * ctx.params.lmp_quad_scaling as i32;
        if depth <= ctx.params.lmp_max_depth as usize
            && !is_pv
            && !in_check
            && moves_searched > lmp_moves
            && is_quiet {
            if see <= 0 {
                continue;
            }
        }
        // =====================================================================================================================//
        // QUIET HISTORY PRUNING                                                                                                      //
        // =====================================================================================================================//
        // TODO FINETUNE PARAMETERS TO MAKE IT WORK ADD CAPTURE HISTORY PRUNING WITH LARGER MARGIN
        if !in_check
            && !is_pv
            && depth <= ctx.params.hist_prune_depth as usize
            && moves_searched > 1  // never prune first move?
        {
            if is_quiet && false{
                let hist = ctx.get_quiet_history_score(pos, mv, ply);
                if hist < -(ctx.params.hist_prune_margin as i32 * depth as i32) {
                    continue;
                }
            }
        }


        // =====================================================================================================================//
        // FUTILITY PRUNING PART 2                                                                                              //
        // =====================================================================================================================//
        if can_futility_prune && moves_searched >ctx.params.fp_min_moves_searched as i32 && is_quiet{
            continue;
        }


        // =====================================================================================================================//
        // HANGING PIECE PRUNING                                                                                                //
        // Prunes bad captures and quiet moves that hang pieces. It feels a little dangerous but it works so I leave it for now //
        // To be tuned later.                                                                                                   //
        // =====================================================================================================================//
        // From WIKI : This is usually done with a linear depth margin for captures, and a quadratic depth margin for quiets, though such details may vary.
        if depth <= ctx.params.hpp_max_depth as usize && !is_pv && !is_root && !in_check {
            if (see as i32) < !is_quiet as i32 * ctx.params.hpp_tactical_scaling as i32 * depth as i32  {
                continue;
            }
        }

        // =========================================================
        // SINGULAR EXTENSIONS (Thanks to Simbelmyne)
        // =========================================================
        let mut extension: i32 = 0;


        if let Some((ref se_mv, tt_score)) = se_info {
            if *se_mv == mv  {

                let mut se_pv = PvTable::new();

                let se_beta = (tt_score - ctx.params.se_scaling as i32 * depth as i32).max(-MATE_SCORE);
                let se_depth = (depth - 1) / 2;



                ctx.excluded_move[ply] = Some(mv);
                let se_score = negamax(pos, ctx, se_depth, ply, se_beta - 1, se_beta, false, &mut se_pv);
                ctx.excluded_move[ply] = None;


                if (*ctx.stop).load(Ordering::Relaxed) { return DRAW_SCORE; }

                if se_score < se_beta  {
                    ctx.stats.singular_extensions += 1;
                    extension += 1;

                    if !is_pv
                        && se_score + (ctx.params.se_dext_margin as i32) < se_beta
                        && ctx.stack.double_exts[ply] <= ctx.params.se_max_nr_dext as i32
                    {
                        // Double extensions
                        extension += 1;
                        ctx.stack.double_exts[ply] += 1;
                        // Triple extensions
                        if is_quiet && se_score + (ctx.params.se_text_margin as i32) < se_beta{
                            extension += 1;
                        }

                    }
                } else if se_beta >= beta {
                    return se_beta;
                }
                else if tt_score >= beta {
                    extension -= 1;
                }
            }
        }

        let mut child_pos = pos.clone();
        child_pos.play_unchecked(mv);



        make_move_nnue(pos, &mv, ctx.network, &mut ctx.nnue);

        let hash_child = child_pos.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;

        ctx.increase_history(hash_child);

        // Push move into stack for continuation history
        ctx.stack.moves[ply] = Some(mv);

        // =====================================================================================================================//
        // START PVS SEARCH WITH FULL WINDOW FOR THE FIRST MOVE AND LMR FOR LATE MOVES                                          //
        // =====================================================================================================================//
        if moves_searched == 1{
            score = -negamax(&child_pos, ctx, depth - 1 + extension as usize, ply + 1, -beta, -alpha, true, &mut local_pv);
        }
        else {
            let mut reduction = 0;
            if moves_searched >=ctx.params.lmr_min_searches as i32 &&  depth >= ctx.params.lmr_min_depth as usize && is_quiet  && !is_pv && !in_check{

                // Base reduction
                reduction = (ctx.params.lmr_red_constant+(depth as f32).ln() * (moves_searched as f32).ln()/ctx.params.lmr_red_scaling) as usize;

                if see <= 0 {
                    reduction += 1;
                }
                if let Some(ttm) = tt_move {
                    if ttm.is_capture() || ttm.is_promotion() {
                        reduction += 1;
                    }
                }
                if child_pos.is_check(){
                    reduction -=1;
                }
                if in_check{
                    reduction -=1;
                }
                if is_pv{
                    reduction -=1;
                }

                reduction -= (ctx.get_quiet_history_score(pos, mv, ply)/ ctx.params.lmr_history_divisor as i32) as usize;

                reduction = reduction.clamp(0,depth - 1);
            }

            score = -negamax(&child_pos, ctx, (depth - 1 - reduction + extension as usize).max(0) , ply + 1, -alpha-1, -alpha, true, &mut local_pv);

            if score > alpha && reduction >0 {
                score = -negamax(&child_pos, ctx, (depth - 1 + extension as usize).max(0), ply + 1, -alpha - 1, -alpha, true, &mut local_pv);
            }
            if score > alpha && score < beta {
                score = -negamax(&child_pos, ctx, (depth - 1 + extension as usize).max(0), ply + 1, -beta, -alpha, true, &mut local_pv);
            }
        }

        // Pop move from stack
        ctx.stack.moves[ply] = None;

        ctx.decrease_history();
        unmake_move_nnue(ctx.network, &mut ctx.nnue);



        if score > best_score {
            best_score = score;
            best_move = Some(mv);
        }

        if score >= beta {
            let bonus = ctx.params.cont_hist_scaling as i32 * depth as i32 - ctx.params.cont_hist_base as i32;

            if !is_capture{

                ctx.store_killer(ply, mv);

                ctx.update_quiet_history(pos.turn() as usize, mv, bonus, &quiets_searched); // Update quiet history, bonus for move, malus for quiets searched

                ctx.update_continuation_history(ply, mv, bonus, &quiets_searched); // Update continuation history, bonus for move, malus for quiets searched

            }else {
                ctx.update_capture_history(pos, mv, bonus, &tacticals_searched);
            }

            break;
        }
        if score > alpha {
            alpha = score;
            pv.add_child_to_parent(mv,&local_pv);

        }


    }

    if !(*ctx.stop).load(Ordering::Relaxed) && !is_excluded{
        tt_store(hash, ctx, depth, best_score, static_eval,original_alpha, beta, best_move,ply);
    }
    best_score
}

// =====================================================================================================================//
// Q SEARCH                                                                                                             //
// =====================================================================================================================//

#[inline(always)]
pub fn quiescence(
    pos: &Chess,
    ctx: &mut SearchContext,
    mut alpha: i32,
    beta: i32,
    ply : usize
) -> i32 {

    ctx.stats.nodes += 1;
    (*ctx.node_count).fetch_add(1, Ordering::Relaxed);
    check_time(ctx);
    if (*ctx.stop).load(Ordering::Relaxed){ return DRAW_SCORE; }

    if ctx.is_threefold(pos) || ctx.is_50_moves(pos) {
        return DRAW_SCORE;
    }

    let hash = pos.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;

    // TT probe for qsearch
    if let Some(score) = tt_probe(hash, ctx, 0, alpha, beta,ply) {
        return score;
    }

    let static_eval = if let Some(entry) = ctx.tt.probe(hash) {
        entry.eval
    } else {
        evaluate(pos, ctx.network, &ctx.nnue.us, &ctx.nnue.them)
    };


    if static_eval >= beta {
        return beta;
    }

    if static_eval > alpha {
        alpha = static_eval;
    }

    let original_alpha = alpha;

    let mut moves = pos.capture_moves();

    ctx.ordering.order_captures(pos, &mut moves);

    for mv in moves {

        let see = see(pos, mv) as i32;

        if see <0{
            continue;
        }

        make_move_nnue(pos, &mv, ctx.network, &mut ctx.nnue);

        let mut child = pos.clone();

        child.play_unchecked(mv);

        let child_hash = child.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;

        ctx.increase_history(child_hash);

        let score = -quiescence(&child, ctx, -beta, -alpha,ply+1);

        ctx.decrease_history();

        unmake_move_nnue(ctx.network, &mut ctx.nnue);

        if score >= beta {
            return beta;
        }

        if score > alpha {
            alpha = score;
        }
    }

    if !(*ctx.stop).load(Ordering::Relaxed) {
        tt_store(hash, ctx, 0, alpha, static_eval,original_alpha, beta, None,ply);
    }
    alpha
}

// =====================================================================================================================//
// ASPIRATION SEARCH                                                                                                    //
// =====================================================================================================================//

#[inline(always)]
fn aspiration_search(pos: &Chess, ctx: &mut SearchContext, depth: usize, prev_score: i32, pv: &mut PvTable) -> i32 {
    let mut window = ctx.params.aspw_window_size as i32;
    let mut alpha = prev_score - window;
    let mut beta = prev_score + window;
    // TODO WINDOW X+avg squared score/Y or prev_score - score scaling
    loop {
        let score = negamax(pos, ctx, depth, 0, alpha, beta, true, pv);

        if (*ctx.stop).load(Ordering::Relaxed) { return score; }

        if score <= alpha {
            alpha = cmp::max(alpha - window, MIN_INF);
        } else if score >= beta {
            beta = cmp::min(beta + window, MAX_INF);
        } else {
            return score;
        }

        window = (window as f32 * ctx.params.aspw_widening_factor) as i32;
    }
}
#[inline(always)]
fn check_time(ctx: &SearchContext) {
    if ctx.stats.nodes % 13337 == 0 {
        if ctx.start_time.elapsed() >= ctx.time_limit{
            (*ctx.stop).store(true, Ordering::Relaxed);
        }
    }
}

#[inline(always)]
fn minors_or_majors(pos : &Chess) -> Bitboard {
    let board = pos.board();
    (board.rooks_and_queens() | board.knights() | board.bishops()) & pos.us()
}

