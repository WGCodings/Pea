use shakmaty::{Chess, Move, MoveList, Position, Role};
use crate::engine::search::context::SearchContext;
use crate::engine::search::see::see;
use crate::engine::types::MAX_PLY_CONTINUATION_HISTORY;

#[derive(Clone)]
pub struct MoveOrdering {
    mvv_lva: [[i32; 6]; 6],
}

impl MoveOrdering {
    pub fn new(piece_values: &[i32; 6]) -> Self {
        let mut table = [[0; 6]; 6];

        for attacker in 0..6 {
            for victim in 0..6 {
                // Higher = better
                table[attacker][victim] =
                    (piece_values[victim] as i32 + 6)
                        - (piece_values[attacker] as i32 / 100);
            }
        }

        Self { mvv_lva: table }
    }

    #[inline(always)]
    pub fn order_moves(
        &self,
        pos: &Chess,
        ctx : &SearchContext,
        pv_move: Option<&Move>,
        tt_move: Option<&Move>,
        killers: &[Option<Move>; 3],
        ply : usize,
        moves: &mut MoveList,
    ) {
        let mut scored: Vec<(i32, Move)> = Vec::with_capacity(moves.len());

        for mv in moves.drain(..) {
            let score = self.score_move(pos, ctx, &mv, pv_move, tt_move, killers,ply);
            scored.push((score, mv));
        }

        // Sort descending by score
        scored.sort_unstable_by(|a, b| b.0.cmp(&a.0));

        // Rebuild move list
        moves.extend(scored.into_iter().map(|(_, mv)| mv));
    }
    #[inline(always)]
    fn score_move(
        &self,
        pos: &Chess,
        ctx: &SearchContext,
        mv: &Move,
        pv_move: Option<&Move>,
        tt_move: Option<&Move>,
        killers: &[Option<Move>; 3],
        ply : usize,
    ) -> i32 {


        // ============================================================
        // 1. TT move (highest priority)
        // ============================================================
        if Some(mv) == tt_move {
            return 1_000_000;
        }

        // ============================================================
        // 2. PV move
        // ============================================================
        if Some(mv) == pv_move {
            return 900_000;
        }

        // ============================================================
        // 3. Captures
        // ============================================================
        if mv.is_capture() {
            let see = see(pos, *mv);
            if see > 0{
                return 800_000 + see as i32
            }
            else if see == 0{
                return 750_000
            }
            else {
                return 5000 + see as i32;
            }

        }

        // ============================================================
        // 4. Killer moves
        // ============================================================
        if killers[0].as_ref() == Some(mv) {
            return 700_000;
        }
        if killers[1].as_ref() == Some(mv) {
            return 699_000;
        }
        if killers[2].as_ref() == Some(mv) {
            return 698_000;
        }
        /*
          if mv.is_promotion(){
              let promotion_role = mv.promotion().unwrap() as i32;
              return 300_000 + 100*promotion_role;
          }
         */

        // ============================================================
        // 5. Quiet move ordering:
        //    Continuation history + normal history
        // ============================================================
        let side = pos.turn() as usize;
        let piece = mv.role() as usize-1;
        let from = mv.from().unwrap().to_usize();
        let to    = mv.to() as usize;

        let mut score = ctx.history[side][from][to] as i32;

        for i in 0..MAX_PLY_CONTINUATION_HISTORY {
            if ply > i {
                if let Some(prev) = ctx.move_stack[ply - 1 - i] {
                    let prev_piece = prev.role() as usize - 1;
                    let prev_to    = prev.to() as usize;


                    score += ctx.continuation_history[i][prev_piece][prev_to][piece][to] as i32;
                }
            }
        }


        score
    }





    #[inline(always)]
    pub fn order_captures(&self, pos: &Chess, moves: &mut [Move]) {
        moves.sort_by_key(|mv| -self.mvv_lva_score(pos, mv));
    }
    #[inline(always)]
    pub fn mvv_lva_score(&self, pos: &Chess, mv: &Move) -> i32 {
        let board = pos.board();

        let attacker_role = board
            .role_at(mv.from().unwrap())
            .expect("attacker must exist");

        let victim_role = board
            .role_at(mv.to())
            .unwrap_or(Role::Pawn); // en passant

        let attacker = attacker_role as usize - 1;
        let victim = victim_role as usize - 1;

        self.mvv_lva[attacker][victim]
    }
}
