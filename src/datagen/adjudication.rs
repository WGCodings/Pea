
// Logic for deciding which positions to keep for training.
// All filtering rules live here so they can be adjusted independently.

use shakmaty::{Chess, Position, Move};
use crate::datagen::datagen_config::DatagenConfig;

/// Reasons a position can be filtered out.
#[derive(Debug, PartialEq)]
pub enum FilterResult {
    Keep,
    InCheck,
    IsCapture,
    IsMateScore,
    IsPromotion,
}

/// Decides whether a position should be recorded for training.
///
/// Rules:
///   - Skip if side to move is in check (noisy, unstable eval)
///   - Skip if the move played was a capture (noisy)
///   - Skip if the move played was a promotion (rare, noisy)
///   - Skip if the eval is a mate score (not useful for training)
pub fn filter_position(pos: &Chess, best_move: &Move, score: i16, mate_threshold: i16) -> FilterResult {
    if pos.is_check() {
        return FilterResult::InCheck;
    }
    if best_move.is_capture() {
        return FilterResult::IsCapture;
    }
    if best_move.is_promotion() {
        return FilterResult::IsPromotion;
    }
    if score.abs() >= mate_threshold {
        return FilterResult::IsMateScore;
    }
    FilterResult::Keep
}
pub fn check_adjudication(score_history: &[i32], config: &DatagenConfig) -> Option<f32> {
    let n = config.adjudication_plies;
    if score_history.len() < n { return None; }

    let recent = &score_history[score_history.len() - n..];

    // Win adjudication — all recent scores strongly favour White
    if recent.iter().all(|&s| s >  config.adjudication_score) {
        return Some(1.0);
    }
    // Win adjudication — all recent scores strongly favour Black
    if recent.iter().all(|&s| s < -config.adjudication_score) {
        return Some(0.0);
    }
    // Draw adjudication — score has been near 0 for a while
    if recent.iter().all(|&s| s.abs() <= config.draw_adjudication_score) {
        return Some(0.5);
    }

    None
}

/// Determine WDL from White's perspective given a terminal position.
pub fn terminal_wdl(pos: &Chess) -> f32 {
    let legal_moves = pos.legal_moves();

    if legal_moves.is_empty() {
        if pos.is_check() {
            // Checkmate — side to move loses
            if pos.turn() == shakmaty::Color::White {
                0.0 // White is mated
            } else {
                1.0 // Black is mated
            }
        } else {
            0.5 // Stalemate
        }
    } else {
        0.5 // Should not be called on non-terminal position
    }
}