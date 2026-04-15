

use std::fs;
use std::io::BufRead;
use rand::RngExt;
use shakmaty::Chess;
use crate::engine::utility::read_position_from_fen;

pub struct EpdBook {
    fens: Vec<String>,
}

impl EpdBook {
    pub fn load(path: &str) -> Self {
        let content = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Cannot open EPD '{}': {}", path, e));

        let fens: Vec<String> = content
            .lines()
            .filter_map(|l| {
                let line = l.trim();
                if line.is_empty() {
                    return None;
                }

                // Split into whitespace parts
                let parts: Vec<&str> = line.split_whitespace().collect();

                if parts.len() < 4 {
                    return None; // not a valid FEN/EPD line
                }

                // Take only the first 4 fields (valid FEN core)
                let fen = format!(
                    "{} {} {} {}",
                    parts[0], parts[1], parts[2], parts[3]
                );

                Some(fen)
            })
            .collect();

        Self { fens }
    }

    /// Pick a random position from the book.
    pub fn random_position(&self, rng: &mut impl rand::Rng) -> Option<Chess> {
        if self.fens.is_empty() {
            return None;
        }

        let fen = &self.fens[rng.random_range(0..self.fens.len())];
        read_position_from_fen(fen)
    }
}