use shakmaty::{Chess, Position};

use crate::engine::params::Params;
use crate::engine::search::context::SearchContext;
use crate::engine::search::search::{negamax, quiescence};

const MARGIN_Q: f32 = 60.0;
const MARGIN_NM: f32 = 70.0;
const NEGAMAX_DEPTH: i32 = 4;
fn is_quiet_position(pos: &mut Chess, params : &Params,ctx : &mut SearchContext) -> bool {
    // 1️⃣ No check allowed
    if pos.is_check() {
        return false;
    }

    // 2️⃣ Static evaluation
    let static_eval = 0.0;

    // 3️⃣ Quiescence search
    let q_eval = quiescence(pos, ctx, 0.0, 0.0);

    if (static_eval - q_eval).abs() > MARGIN_Q {
        return false;
    }

    // 4️⃣ Shallow Negamax
    let nm_eval = negamax(pos, ctx,  NEGAMAX_DEPTH as usize, 0, 0.0, 0.0,None);

    if (static_eval - nm_eval).abs() > MARGIN_NM {
        return false;
    }

    true
}
