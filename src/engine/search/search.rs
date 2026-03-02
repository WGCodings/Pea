use std::cmp;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use shakmaty::{Bitboard, Chess, EnPassantMode, Move, Position};
use shakmaty::zobrist::{Zobrist64};
use crate::engine::eval::evaluate;
use crate::engine::search::context::{make_move_nnue, unmake_move_nnue, SearchContext};
use crate::engine::tt::{tt_best_move, tt_probe, tt_store};
use crate::engine::types::{DRAW_SCORE, MATE_SCORE, MAX_INF, MIN_INF};


pub struct SearchStats {
    pub nodes: u64,
    pub depth_sum: u64,
    pub depth_samples: u64,
    pub seldepth: u32,
    pub duration: Duration,
}

impl SearchStats {
    pub(crate) fn default() -> SearchStats {
        Self {
            nodes: 0,
            depth_sum: 0,
            depth_samples: 0,
            seldepth: 0,
            duration: Duration::ZERO,
        }
    }
}

pub fn search(pos: &Chess, ctx: &mut SearchContext, max_depth: usize, time_remaining: Option<Duration>, ) -> (i32, Move) {

    ctx.start_time = Instant::now();
    ctx.time_limit = time_remaining.unwrap();
    ctx.stop.store(false, Ordering::Relaxed);

    let mut best_score = MIN_INF;
    let mut best_move = None;

    let mut prev_score = 0;

    for depth in 1..=max_depth {

        if ctx.start_time.elapsed() > ctx.time_limit {
            break;
        }

        ctx.pv.clear_from(0);
        ctx.multipv.clear();

        let mut alpha ;
        let mut beta ;
        let mut score;

        // =====================================================================================================================//
        // ASPIRATION SEARCH                                                                                                    //
        // =====================================================================================================================//
        if depth >= ctx.params.aspw_min_depth {
            let mut window = ctx.params.aspw_window_size;

            alpha = prev_score - window;
            beta = prev_score + window;

            loop {
                score = negamax(pos, ctx, depth, 0, alpha, beta, true);

                if ctx.stop.load(Ordering::Relaxed) {
                    break;
                }

                if score <= alpha {
                    alpha -= window;
                } else if score >= beta {
                    beta += window;
                } else {
                    break;
                }

                window *= ctx.params.aspw_widening_factor;

                alpha = cmp::max(alpha,MIN_INF);
                beta = cmp::min(beta,MAX_INF);
            }

        } else {
            score = negamax(pos, ctx, depth, 0, MIN_INF, MAX_INF, true);
        }

        if ctx.stop.load(Ordering::Relaxed) {
            break;
        }

        ctx.multipv.save_completed_iteration();

        best_score = score;
        prev_score = score;
        best_move = ctx.pv.best_move();
    }

    ctx.stats.duration = ctx.start_time.elapsed();

    (best_score, best_move.expect("No legal move found"))
}



#[inline(always)]
pub fn negamax(
    pos: &Chess,
    ctx: &mut SearchContext,
    mut depth: usize,
    ply: usize,
    mut alpha: i32,
    beta: i32,
    do_null : bool
) -> i32 {
    ctx.stats.nodes += 1;
    ctx.stats.seldepth = cmp::max(ply as u32, ctx.stats.seldepth);
    ctx.stats.depth_sum += ply as u64;
    ctx.stats.depth_samples += 1;

    let is_root = ply == 0;
    let in_check = pos.is_check();
    let is_pv = beta-alpha >1;

    let original_alpha = alpha;
    let mut best_score = MIN_INF;
    let mut best_move = None;

    check_time(ctx);

    let pv_table = &ctx.pv.table;
    let pv_move: Option<Move> = pv_table.get(ply).and_then(|l| l.first()).cloned();



    ctx.pv.clear_from(ply);

    if pos.is_checkmate() {
        return -MATE_SCORE + ply as i32;
    }

    if (ctx.is_threefold(pos) || ctx.is_50_moves(pos) || pos.is_stalemate() || pos.is_insufficient_material()) && !is_root {
        return DRAW_SCORE;
    }

    if in_check && ply < 63 && depth <= 2 {
        depth += 1;
    }

    if ctx.stop.load(Ordering::Relaxed){ return DRAW_SCORE; }



    if depth <= 0 {
        ctx.stats.nodes -= 1;
        return quiescence(pos, ctx, alpha, beta,ply);
    }

    let hash = pos.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;
    let mut score;

    if let Some(score) = tt_probe(hash, ctx, depth, alpha, beta,ply) {
        if !is_root {
            return score;
        }
    }


    let static_eval = evaluate(pos,ctx.network,&ctx.nnue.us, &ctx.nnue.them);

    // Safety
    if ply > 63{
        return static_eval;
    }

    let do_pruning = minors_or_majors(pos).count() >0;
    let mut can_futility_prune = false;


    // =====================================================================================================================//
    // REVERSE FUTILITY PRUNING                                                                                             //
    // =====================================================================================================================//
    let futility = 47*depth as i32;
    if do_pruning && false && !is_pv && !in_check && depth <= 9 && !is_root && static_eval - futility   >=beta {
        return (static_eval + beta)/2;
    }

    // =====================================================================================================================//
    // STATIC NULL MOVE PRUNING                                                                                             //
    // =====================================================================================================================//
    if  do_pruning && !in_check && !is_pv && beta.abs() < MATE_SCORE {
        let score_margin = ctx.params.snmp_scaling * depth as i32;
        if static_eval-score_margin >= beta {
            return static_eval-score_margin
        }
    }

    // =====================================================================================================================//
    // NULL MOVE PRUNING                                                                                                    //
    // =====================================================================================================================//
    let nmp_margin : i32= -ctx.params.nmp_margin + ctx.params.nmp_scaling * depth as i32;
    if  do_pruning && !in_check && !is_pv && !is_root &&
        static_eval + nmp_margin >= beta &&
        do_null && minors_or_majors(pos).count() >0 &&
        depth >=ctx.params.nmp_min_depth {

        let reduction = (ctx.params.nmp_base_reduction + depth/ctx.params.nmp_reduction_scaling).min(depth);

        std::mem::swap(&mut ctx.nnue.us, &mut ctx.nnue.them);

        let child_pos = pos.clone().swap_turn().unwrap();

        let hash_child = child_pos.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;
        ctx.increase_history(hash_child);

        let score = -negamax(&child_pos, ctx, depth - reduction, ply + 1, -beta, -beta + 1, false);

        std::mem::swap(&mut ctx.nnue.us, &mut ctx.nnue.them);
        ctx.decrease_history();

        if ctx.stop.load(Ordering::Relaxed){ return DRAW_SCORE;}

        if score >= beta && score.abs() < MATE_SCORE {
            return beta;
        }
    }


    // =====================================================================================================================//
    // RAZORING                                                                                                             //
    // =====================================================================================================================//
    if  do_pruning && !in_check && !is_pv && depth <= ctx.params.raz_max_depth && static_eval +ctx.params.raz_thr *(depth as i32) < alpha
    {
        let razor_score = quiescence(pos,ctx,alpha,beta,ply);
        if razor_score <= alpha{
            return razor_score;
        }
    }

    // =====================================================================================================================//
    // FUTILITY PRUNING PART 1                                                                                              //
    // =====================================================================================================================//
    if  depth <= ctx.params.fp_max_depth && !is_pv && !in_check && alpha.abs() < MATE_SCORE && beta.abs() < MATE_SCORE {
        let margin = ctx.params.fp_margins[depth];
        can_futility_prune = static_eval+margin <= alpha;
    }


    let mut moves = pos.legal_moves();
    let mut moves_searched : i32 = 0;

    let tt_move = tt_best_move(hash, ctx);

    ctx.ordering.order_moves(pos, ctx, pv_move.as_ref(), tt_move.as_ref(), &ctx.killers[ply], ply, &mut moves);

    let mut quiets_searched: Vec<Move> = Vec::new();


    for mv in moves {

        if !mv.is_capture() {
            quiets_searched.push(mv);
        }
        moves_searched +=1;



        let mut child_pos = pos.clone();
        child_pos.play_unchecked(mv);

        let is_quiet = !mv.is_capture() && !mv.is_promotion() && !child_pos.is_check();

        // =====================================================================================================================//
        // LATE MOVE PRUNING                                                                                                    //
        // =====================================================================================================================//
        let lmp_moves= ctx.params.lmp_base+depth as i32 * ctx.params.lmp_lin_scaling + depth as i32 * depth as i32 * ctx.params.lmp_quad_scaling;
        if depth <= ctx.params.lmp_max_depth
            && !is_pv
            && !in_check
            && moves_searched > lmp_moves
            && is_quiet {
            continue;
        }

        // =====================================================================================================================//
        // FUTILITY PRUNING PART 2                                                                                              //
        // =====================================================================================================================//
        if can_futility_prune && moves_searched >1 && is_quiet{
            continue;
        }

        make_move_nnue(pos, &mv, ctx.network, &mut ctx.nnue);

        let hash_child = child_pos.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;

        ctx.increase_history(hash_child);

        // Push move into stack for continuation history
        ctx.move_stack[ply] = Some(mv);

        // =====================================================================================================================//
        // START PVS SEARCH WITH FULL WINDOW FOR THE FIRST MOVE AND LMR FOR LATE MOVES                                          //
        // =====================================================================================================================//
        if moves_searched == 1{
            score = -negamax(&child_pos, ctx, depth - 1, ply + 1, -beta, -alpha, true);
        }
        else {
            let mut reduction = 0;
            if moves_searched >=ctx.params.lmr_min_searches &&  depth >= ctx.params.lmr_min_depth && is_quiet  && !is_pv && !in_check{
                reduction = (ctx.params.lmr_red_constant+(depth as f32).ln() * (moves_searched as f32).ln()/ctx.params.lmr_red_scaling) as usize;
            }

            score = -negamax(&child_pos, ctx, depth - 1 -reduction , ply + 1, -alpha-1, -alpha, true);

            if score > alpha && reduction >0{
                score = -negamax(&child_pos, ctx, depth - 1, ply + 1, -alpha-1, -alpha, true);
                if score > alpha{
                    score = -negamax(&child_pos, ctx, depth - 1, ply + 1, -beta, -alpha, true);
                }
            }
            else if score > alpha && score < beta {
                score = -negamax(&child_pos, ctx, depth - 1, ply + 1, -beta, -alpha, true);
            }
        }

        // Pop move from stack
        ctx.move_stack[ply] = None;

        ctx.decrease_history();
        unmake_move_nnue(ctx.network, &mut ctx.nnue);


        if score > best_score {
            best_score = score;
            best_move = Some(mv);
        }

        if score >= beta {
            if !mv.is_capture() {

                ctx.store_killer(ply, mv);

                let side = pos.turn() as usize;

                let bonus = ctx.params.cont_hist_scaling * depth as i32 - ctx.params.cont_hist_base;

                ctx.update_quiet_history(side, mv, bonus, &quiets_searched,ctx.params); // Update normal history values and malus for quiets searched

                ctx.update_continuation_history(ply, mv, bonus/2); // Update continuation history

                for &q in quiets_searched.iter() {
                    if q != mv {
                        ctx.update_continuation_history(ply, q, -bonus/(2*ctx.params.cont_hist_malus_scaling)); // Update malus for continuation history
                    }
                }
            }
            break;
        }
        if score > alpha {
            alpha = score;
            update_pv(ply, mv, best_score, ctx);
        }
    }

    if !ctx.stop.load(Ordering::Relaxed) {
        tt_store(hash, ctx, depth, best_score, original_alpha, beta, best_move,ply);
    }
    best_score
    }

    #[inline(always)]
    pub fn quiescence(
        pos: &Chess,
        ctx: &mut SearchContext,
        mut alpha: i32,
        beta: i32,
        ply : usize
    ) -> i32 {
        ctx.stats.nodes += 1;

        check_time(ctx);
        if ctx.stop.load(Ordering::Relaxed){ return DRAW_SCORE; }

        if ctx.is_threefold(pos) || ctx.is_50_moves(pos) {
            return DRAW_SCORE;
        }

        let hash = pos.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;

        // TT probe for qsearch
        if let Some(score) = tt_probe(hash, ctx, 0, alpha, beta,ply) {
            return score;
        }


        //let stand_pat = evaluate_nnue(pos, ctx.network);
        let stand_pat = evaluate(pos,ctx.network,&ctx.nnue.us, &ctx.nnue.them);

        if stand_pat >= beta {
            return beta;
        }

        if stand_pat > alpha {
            alpha = stand_pat;
        }

        let original_alpha = alpha;

        let mut moves = pos.capture_moves();

        ctx.ordering.order_captures(pos, &mut moves);

        for mv in moves {
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

        if !ctx.stop.load(Ordering::Relaxed) {
            tt_store(hash, ctx, 0, alpha, original_alpha, beta, None,ply);
        }
        alpha
    }

    #[inline(always)]
    fn check_time(ctx: &SearchContext) {
        if ctx.stats.nodes % 13337 == 0 {
            if ctx.start_time.elapsed() >= ctx.time_limit {
                ctx.stop.store(true, Ordering::Relaxed);
            }
        }
    }
    #[inline(always)]
    fn update_pv(ply: usize, mv: Move, best_score: i32, ctx: &mut SearchContext) {
        let child_line = ctx.pv.table[ply + 1].clone();
        ctx.pv.set_pv(ply, mv, &child_line);

        if ply == 0 {
            ctx.multipv.insert(best_score, ctx.pv.pv_line().to_vec());
        }
    }

    #[inline(always)]
    fn minors_or_majors(pos : &Chess) -> Bitboard {
        let board = pos.board();
        (board.rooks_and_queens() | board.knights() | board.bishops()) & pos.us()
    }
