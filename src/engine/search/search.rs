use std::cmp;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use shakmaty::{Chess, EnPassantMode, Move, Position};
use shakmaty::zobrist::{Zobrist64};


use crate::engine::search::context::{make_move_nnue, unmake_move_nnue, SearchContext};

use crate::engine::time_manager::compute_time_limit;
use crate::engine::tt::Bound;
use crate::engine::types::{DRAW_SCORE, MATE_SCORE};


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

pub fn search(pos: &Chess, ctx: &mut SearchContext, max_depth: usize, time_remaining: Option<Duration>) -> (f32,Move) {

    ctx.start_time = Instant::now();
    ctx.time_limit = time_remaining.unwrap();
    ctx.stop.store(false, Ordering::Relaxed);


    let mut best_score = f32::NEG_INFINITY;
    let mut best_move = None;

    for depth in 1..=max_depth {
        ctx.clear_counter_moves();

        if ctx.start_time.elapsed() > ctx.time_limit{
            break;
        }

        ctx.pv.clear_from(0);
        ctx.multipv.clear();

        let score = negamax(pos, ctx, depth, 0, f32::NEG_INFINITY, f32::INFINITY, None, true);

        if ctx.stop.load(Ordering::Relaxed){
            if best_move == None && depth ==1{
                best_move = ctx.pv.best_move();
            }
            break;
        }

        best_score = score;
        best_move = ctx.pv.best_move();
    }

    ctx.stats.duration = ctx.start_time.elapsed();

    (best_score,best_move.unwrap())


}


#[inline(always)]
pub fn negamax(
    pos: &Chess,
    ctx: &mut SearchContext,
    mut depth: usize,
    ply: usize,
    mut alpha: f32,
    beta: f32,
    prev_move: Option<Move>,
    do_null : bool
) -> f32 {
    ctx.stats.nodes += 1;
    ctx.stats.seldepth = cmp::max(ply as u32, ctx.stats.seldepth);
    ctx.stats.depth_sum += ply as u64;
    ctx.stats.depth_samples += 1;

    let is_root = ply == 0;
    let in_check = pos.is_check();
    let is_pv = beta-alpha >1.0;
    let original_alpha = alpha;
    let mut best_score = f32::NEG_INFINITY;
    let mut best_move = None;

    check_time(ctx);


    let pv_table = &ctx.pv.table;
    let pv_move: Option<Move> = pv_table.get(ply).and_then(|l| l.first()).cloned();

    ctx.pv.clear_from(ply);

    if ctx.stop.load(Ordering::Relaxed){ return 0.0; }

    if pos.is_checkmate() {
        return -MATE_SCORE + ply as f32;
    }

    if (ctx.is_threefold(pos) || ctx.is_50_moves(pos) || pos.is_stalemate() || pos.is_insufficient_material()) && !is_root {
        return DRAW_SCORE;
    }

    if pos.is_check() && ply < 63 && depth <= 2 {
        depth += 1;
    }

    if depth <= 0 {
        ctx.stats.nodes -= 1;
        return quiescence(pos, ctx, alpha, beta);
    }

    let hash = pos.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;
    let mut score = tt_probe(hash, ctx, depth, alpha, beta).unwrap_or(-10000000.0);
    if score != -10000000.0 && ply != 0 {
        return score;
    }

    let static_eval = ctx.network.evaluate(&ctx.nnue.us, &ctx.nnue.them, pos) as f32;
    /*


    if ply > 63{
        return static_eval;
    }

    let nmp_margin : f32= -120.0 + 20.0 * depth as f32;
    if !in_check && depth >= 2 && ply != 0 && static_eval + nmp_margin >= beta && do_null {
        //println!("{}", ply);
        let reduction = (4 + depth / 4).min(depth);

        std::mem::swap(&mut ctx.nnue.us, &mut ctx.nnue.them);

        let child_pos = pos.clone().swap_turn().unwrap();

        let hash_child = child_pos.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;
        ctx.increase_history(hash_child);


        let score = -negamax(&child_pos, ctx, depth - 1 - reduction, ply + 1, -beta, -beta + 1.0, None, false);

        std::mem::swap(&mut ctx.nnue.us, &mut ctx.nnue.them);
        ctx.decrease_history();

        if ctx.stop.load(Ordering::Relaxed){ return 0.0; }

        if score >= beta && score.abs() < MATE_SCORE {
            return beta;
        }
    }
    */


    // Razoring from Snail chess
    if !in_check && !is_pv && depth <= ctx.params.raz_max_depth && static_eval +ctx.params.raz_thr *(depth as f32) < alpha
    {
        let razor_score = quiescence(pos,ctx,alpha,beta);
        if razor_score <= alpha{
            return razor_score;
        }
    }


    let mut moves = pos.legal_moves();
    let mut moves_searched = 0;

    let tt_move = tt_best_move(hash, ctx);

    ctx.ordering.order_moves(pos, ctx, pv_move.as_ref(), tt_move.as_ref(), &ctx.killers[ply], prev_move.as_ref(), &mut moves);

    let mut quiets_searched: Vec<Move> = Vec::new();


    for mv in moves {
        if !mv.is_capture() {
            quiets_searched.push(mv);
        }

        make_move_nnue(pos, &mv, ctx.network, &mut ctx.nnue);

        let mut child_pos = pos.clone();

        child_pos.play_unchecked(mv);


        let hash_child = child_pos.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;

        ctx.increase_history(hash_child);

        // START PVS SEARCH WITH FULL WINDOW FOR THE FIRST MOVE
        if moves_searched == 0{
            score = -negamax(&child_pos, ctx, depth - 1, ply + 1, -beta, -alpha, Some(mv),true);
        }
        else {
            score = -negamax(&child_pos, ctx, depth - 1, ply + 1, -alpha-1.0, -alpha, Some(mv),true);

            if score > alpha{
                score = -negamax(&child_pos, ctx, depth - 1, ply + 1, -beta, -alpha, Some(mv),true);
            }

        }
        moves_searched +=1;

        ctx.decrease_history();
        unmake_move_nnue(ctx.network, &mut ctx.nnue);


        if score > best_score {
            best_score = score;
            best_move = Some(mv);
        }

        if score >= beta {
            if !mv.is_capture() {
                ctx.store_killer(ply, mv);
                if let Some(prev) = prev_move { ctx.store_counter_move(&prev, mv, pos.turn() as usize); }
                ctx.increase_history_bonus(pos.turn() as usize, mv, depth);
                for q in quiets_searched.iter() {
                    if *q != mv {
                        ctx.decrease_history_bonus(pos.turn() as usize, *q, depth);
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
            tt_store(hash, ctx, depth, best_score, original_alpha, beta, best_move);
        }

        best_score
    }

    #[inline(always)]
    pub fn quiescence(
        pos: &Chess,
        ctx: &mut SearchContext,
        mut alpha: f32,
        beta: f32,
    ) -> f32 {
        ctx.stats.nodes += 1;

        check_time(ctx);
        if ctx.stop.load(Ordering::Relaxed){ return 0.0; }

        if ctx.is_threefold(pos) || ctx.is_50_moves(pos) {
            return DRAW_SCORE;
        }

        let hash = pos.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;

        // TT probe for qsearch
        if let Some(score) = tt_probe(hash, ctx, 0, alpha, beta) {
            return score;
        }


        //let stand_pat = evaluate_nnue(pos, ctx.network);
        let stand_pat = ctx.network.evaluate(&ctx.nnue.us, &ctx.nnue.them, pos) as f32;

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

            let score = -quiescence(&child, ctx, -beta, -alpha);

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
            tt_store(hash, ctx, 0, alpha, original_alpha, beta, None);
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
    fn update_pv(ply: usize, mv: Move, best_score: f32, ctx: &mut SearchContext) {
        let child_line = ctx.pv.table[ply + 1].clone();
        ctx.pv.set_pv(ply, mv, &child_line);

        if ply == 0 {
            ctx.multipv.insert(best_score, ctx.pv.pv_line().to_vec());
        }
    }
    #[inline(always)]
    fn tt_probe(key: u64, ctx: &mut SearchContext, depth: usize, alpha: f32, beta: f32, ) -> Option<f32> {
        if let Some(entry) = ctx.tt.probe(key) {
            if entry.depth as usize >= depth {
                match entry.bound {
                    Bound::Exact => return Some(entry.score),
                    Bound::Lower if entry.score >= beta => {
                        return Some(entry.score)
                    }
                    Bound::Upper if entry.score <= alpha => {
                        return Some(entry.score)
                    }
                    _ => {}
                }
            }
        }
        None
    }
    #[inline(always)]
    fn tt_store(key : u64, ctx: &mut SearchContext, depth: usize, best_score: f32, alpha: f32, beta: f32, best_move: Option<Move>, ) {
        let bound = if best_score <= alpha {
            Bound::Upper
        } else if best_score >= beta {
            Bound::Lower
        } else {
            Bound::Exact
        };
        ctx.tt.store(key, depth, best_score, bound, best_move);
    }
    #[inline(always)]
    fn tt_best_move(key : u64, ctx: &mut SearchContext, ) -> Option<Move> {
        ctx.tt
            .probe(key)
            .and_then(|e| e.best_move.clone())
    }
