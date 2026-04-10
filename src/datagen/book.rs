

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
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.trim().to_string())
            .collect();

        Self { fens }
    }

    /// Pick a random position from the book.
    pub fn random_position(&self, rng: &mut impl rand::Rng) -> Option<Chess> {
        let fen = &self.fens[rng.random_range(0..self.fens.len())];
        read_position_from_fen(fen)
    }
}