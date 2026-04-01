
// Hard per-position filters. Each filter is a separate function.
// A position must pass ALL filters to be kept.

use crate::config::FilterConfig;
use crate::reader::RawPosition;

#[derive(Debug, PartialEq)]
pub enum FilterResult {
    Keep,
    ScoreTooHigh,
    ScoreTooLow,
    WrongNetId
}

/// Apply all hard filters to a single position.
pub fn apply_filters(pos: &RawPosition, config: &FilterConfig) -> FilterResult {
    let abs_score = pos.score.unsigned_abs() as i32;

    if abs_score > config.max_score {
        return FilterResult::ScoreTooHigh;
    }
    if config.min_score > 0 && abs_score < config.min_score as i32 {
        return FilterResult::ScoreTooLow;
    }
    if let Some(net_id) = config.net_id {
        if pos.net_id != net_id {
            return FilterResult::WrongNetId;
        }
    }

    FilterResult::Keep
}

/// Quick check — is this position balanced, so -100 < score < 100 for example
pub fn is_balanced(pos: &RawPosition, config: &FilterConfig) -> bool {
    pos.score.abs() <= config.imbalance_threshold
}

/// Quick check — is this position materially imbalanced
pub fn is_imbalanced(pos: &RawPosition, config: &FilterConfig) -> bool {
    pos.score.abs() > config.imbalance_threshold
}