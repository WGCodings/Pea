
// Fast buffered reader for raw position files.
// Reads multiple files and yields RawPosition records.

use std::fs::File;
use std::io::{BufRead, BufReader};
use glob::glob;
// ------------------------------------------------------------------ //
// Raw position — mirrors datagen format.rs but lives here standalone  //
// ------------------------------------------------------------------ //

#[derive(Debug, Clone)]
pub struct RawPosition {
    pub fen:       String,
    pub score:     i32,   // white-relative centipawns
    pub wdl:       f32,   // white-relative: 1.0 / 0.5 / 0.0
    pub net_id:    u8,
    pub _nodes:     u64,
    pub _depth:      usize,
    pub pawn_hash: u64,
}

impl RawPosition {
    pub fn from_line(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.splitn(7, '|').collect();
        if parts.len() != 7 { return None; }

        Some(Self {
            fen:       parts[0].to_string(),
            score:     parts[1].parse().ok()?,
            wdl:       parts[2].parse().ok()?,
            net_id:    parts[3].parse().ok()?,
            _nodes:     parts[4].parse().ok()?,
            _depth:     parts[5].parse().ok()?,
            pawn_hash: u64::from_str_radix(parts[6].trim(), 16).ok()?,
        })
    }

    /// Score relative to side to move, extracted from FEN.
    /// FEN field 2 is 'w' or 'b'.
    pub fn stm_score(&self) -> i32 {
        if self.is_white_to_move() { self.score } else { -self.score }
    }

    pub fn is_white_to_move(&self) -> bool {
        // FEN second field is the side to move
        self.fen.as_str().split_whitespace().nth(1).map(|s| s == "w").unwrap_or(true)
    }
}

// ------------------------------------------------------------------ //
// Multi-file reader                                                    //
// ------------------------------------------------------------------ //

/// Read all positions from a list of files.
/// Skips malformed lines with a warning.
/// Streams line by line — does not load everything into memory at once
/// unless you collect into a Vec.
pub fn read_all_files(paths: &[String]) -> impl Iterator<Item = RawPosition> + '_ {
    paths.iter().flat_map(|path| read_file(path.clone()))
}

/// Read positions from a single file, streaming line by line.
pub fn read_file(path: String) -> impl Iterator<Item = RawPosition> {
    let file = File::open(&path)
        .unwrap_or_else(|e| panic!("Cannot open '{}': {}", path, e));

    BufReader::with_capacity(4 * 1024 * 1024, file) // 4MB read buffer
        .lines()
        .filter_map(move |line| {
            let line = line.ok()?;
            let line = line.trim();
            if line.is_empty() { return None; }
            RawPosition::from_line(line)
        })
}

/// Expand a list of paths/patterns to actual file paths.
pub fn expand_inputs(patterns: &[String]) -> Vec<String> {
    let mut files = Vec::new();
    for pattern in patterns {
        let matches: Vec<String> = glob(pattern.as_str())
            .unwrap_or_else(|e| panic!("Invalid pattern '{}': {}", pattern, e))
            .filter_map(|entry| entry.ok())
            .map(|p| p.display().to_string())
            .collect();

        if matches.is_empty() {
            // No glob matches — try as literal path
            files.push(pattern.clone());
        } else {
            files.extend(matches);
        }
    }
    files
}
