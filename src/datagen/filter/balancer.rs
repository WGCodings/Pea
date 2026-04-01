// balance.rs
// Balancing logic:
//   1. Pawn hash deduplication — limit positions per pawn structure
//   2. Fraction enforcement — ensure quiet/imbalanced/positive ratios are met

use std::collections::HashMap;

use crate::config::FilterConfig;
use crate::filter::{apply_filters, is_imbalanced, FilterResult, is_balanced};
use crate::reader::RawPosition;

// ------------------------------------------------------------------ //
// Stats                                                                //
// ------------------------------------------------------------------ //

#[derive(Debug)]
pub struct BalanceStats {
    pub total_read:          u64,
    pub dropped_hard_filter: u64,
    pub dropped_pawn_hash:   u64,
    pub kept:                u64,
    pub balanced:            u64,
    pub imbalanced:          u64,
    pub positive_stm:        u64,
}

impl BalanceStats {
    fn default() -> Self {
        Self {
            total_read: 0,
            dropped_hard_filter: 0,
            dropped_pawn_hash: 0,
            kept: 0,
            balanced: 0,
            imbalanced: 0,
            positive_stm: 0,
        }
    }

}

impl BalanceStats {
    pub fn print(&self) {
        println!("--- Filter stats ---");
        println!("  Read:              {:>12}", self.total_read);
        println!("  Dropped (filter):  {:>12}", self.dropped_hard_filter);
        println!("  Dropped (pawnhash):{:>12}", self.dropped_pawn_hash);
        println!("  Kept:              {:>12}", self.kept);
        println!("  Balanced:          {:>12} ({:.1}%)", self.balanced,
                 100.0 * self.balanced as f64 / self.kept.max(1) as f64);
        println!("  Imbalanced:        {:>12} ({:.1}%)", self.imbalanced,
                 100.0 * self.imbalanced as f64 / self.kept.max(1) as f64);
        println!("  Positive STM:      {:>12} ({:.1}%)", self.positive_stm,
                 100.0 * self.positive_stm as f64 / self.kept.max(1) as f64);
    }

    /// Check if balance targets are met. Prints warnings if not.
    pub fn check_targets(&self, config: &FilterConfig) {
        let kept = self.kept.max(1) as f64;
        let quiet_frac     = self.balanced as f64 / kept;
        let imbalance_frac = self.imbalanced as f64 / kept;
        let positive_frac  = self.positive_stm as f64 / kept;

        if quiet_frac < config.min_balanced_fraction {
            println!("WARNING: balanced fraction {:.1}% < target {:.1}%",
                     quiet_frac * 100.0, config.min_balanced_fraction * 100.0);
        }
        if imbalance_frac < config.min_imbalanced_fraction {
            println!("WARNING: imbalanced fraction {:.1}% < target {:.1}%",
                     imbalance_frac * 100.0, config.min_imbalanced_fraction * 100.0);
        }
        if positive_frac < config.min_positive_fraction {
            println!("WARNING: positive STM fraction {:.1}% < target {:.1}%",
                     positive_frac * 100.0, config.min_positive_fraction * 100.0);
        }
    }
}

// ------------------------------------------------------------------ //
// Main balancer                                                        //
// ------------------------------------------------------------------ //

/// Load all positions from an iterator, apply hard filters and pawn hash
/// deduplication, return the kept positions and stats.
///
/// This loads everything into memory. For billions of positions you'd want
/// a two-pass approach — but for typical datasets (tens of millions) this
/// is fine.
pub fn collect_and_filter(
    positions: impl Iterator<Item = RawPosition>,
    config:    &FilterConfig,
) -> (Vec<RawPosition>, BalanceStats) {
    let mut stats     = BalanceStats::default();
    let mut pawn_counts: HashMap<u64, usize> = HashMap::new();
    let mut kept: Vec<RawPosition> = Vec::new();

    for pos in positions {
        stats.total_read += 1;

        // Progress print every 1M lines
        if stats.total_read % 1_000_000 == 0 {
            println!("  Read {}M positions, kept {}...",
                     stats.total_read / 1_000_000, stats.kept);
        }

        // Hard filter
        if apply_filters(&pos, config) != FilterResult::Keep {
            stats.dropped_hard_filter += 1;
            continue;
        }

        // Pawn hash deduplication
        if config.max_per_pawn_hash > 0 {
            let count = pawn_counts.entry(pos.pawn_hash).or_insert(0);
            if *count >= config.max_per_pawn_hash {
                stats.dropped_pawn_hash += 1;
                continue;
            }
            *count += 1;
        }

        // Update stats
        if is_balanced(&pos, config)     { stats.balanced += 1; }
        if is_imbalanced(&pos, config){ stats.imbalanced += 1; }
        if pos.stm_score() > 0        { stats.positive_stm += 1; }

        stats.kept += 1;
        kept.push(pos);
    }

    (kept, stats)
}