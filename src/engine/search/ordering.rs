use shakmaty::{Chess, Move, MoveList, Position, Role};
use crate::engine::search::context::SearchContext;

#[derive(Clone)]
pub struct MoveOrdering {
    mvv_lva: [[i32; 6]; 6],
}

impl MoveOrdering {
    pub fn new(piece_values: &[f32; 6]) -> Self {
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
        previous_move: Option<&Move>,
        moves: &mut MoveList,
    ) {
        let mut scored: Vec<(i32, Move)> = Vec::with_capacity(moves.len());

        for mv in moves.drain(..) {
            let score = self.score_move(pos, ctx, &mv, pv_move, tt_move, killers,previous_move);
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
        previous_move: Option<&Move>,
    ) -> i32 {

        // 2. TT move
        if Some(mv) == tt_move {
            return 900_000;
        }

        // 3. Captures
        if mv.is_capture() {
            return 500_000 + self.mvv_lva_score(pos, mv);
        }
        /*
        if mv.is_promotion(){
            let promotion_role = mv.promotion().unwrap() as i32;
            return 450_000 + 100*promotion_role;
        }
        */
        // 4. Killer moves
        if killers[0].as_ref() == Some(mv) {
            return 400_000;
        }
        if killers[1].as_ref() == Some(mv) {
            return 399_000;
        }
        if killers[2].as_ref() == Some(mv) {
            return 398_000;
        }
        // 1. PV move
        if Some(mv) == pv_move {
            return 350_000;
        }
        /*
        // 5. Counter move
        if let Some(prev) = previous_move {
            let side = pos.turn() as usize;   // side to move now
            if let Some(counter) = ctx.get_counter_move(prev, side) {
                if counter == *mv {
                    return 350_000;
                }
            }
        }
        */

        ctx.get_history_score(pos.turn() as usize, *mv)

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
