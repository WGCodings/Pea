use shakmaty::{Chess, Position};

use crate::engine::params::Params;
use crate::engine::search::context::SearchContext;
use crate::engine::search::pv::PvTable;
use crate::engine::search::search::{negamax, quiescence};

const _MARGIN_Q: i32 = 60;
const _MARGIN_NM: i32 = 70;
const _NEGAMAX_DEPTH: i32 = 4;
fn _is_quiet_position(pos: &mut Chess, _params : &Params,ctx : &mut SearchContext) -> bool {

    if pos.is_check() {
        return false;
    }


    let static_eval = 0;


    let q_eval = quiescence(pos, ctx, 0, 0,0);

    if (static_eval - q_eval).abs() > _MARGIN_Q {
        return false;
    }


    let nm_eval = negamax(pos, ctx, _NEGAMAX_DEPTH as usize, 0, 0, 0,  true,&mut PvTable::new());

    if (static_eval - nm_eval).abs() > _MARGIN_NM {
        return false;
    }

    true
}
