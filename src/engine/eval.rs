use shakmaty::{Chess, Color, Position};

use crate::nnue::network::{evaluate_position, Network};

pub fn evaluate_nnue(pos: &Chess, net: &Network) -> f32 {
    let score= evaluate_position(pos, &net) as f32;
    if pos.turn() == Color::White {
        score
    } else {
        -score
    }
}




