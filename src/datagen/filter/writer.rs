// writer.rs
// Writes positions in bullet training format.
//
// Bullet format (pipe-separated, one position per line):
//   FEN | score | WDL
//
// Where:
//   FEN   — standard FEN string
//   score — eval in centipawns, white-relative
//   WDL   — white-relative result: 1.0 / 0.5 / 0.0

use std::fs::File;
use std::io::{BufWriter, Write};

use crate::reader::RawPosition;

pub struct BulletWriter {
    writer:            BufWriter<File>,
    positions_written: u64,
}

impl BulletWriter {
    pub fn open(path: &str) -> std::io::Result<Self> {
        let file = File::create(path)?;
        Ok(Self {
            writer: BufWriter::with_capacity(4 * 1024 * 1024, file),
            positions_written: 0,
        })
    }

    pub fn write(&mut self, pos: &RawPosition) -> std::io::Result<()> {
        // bullet format: FEN | score | WDL
        writeln!(self.writer, "{} | {} | {}", pos.fen, pos.score, pos.wdl)?;
        self.positions_written += 1;
        Ok(())
    }

    pub fn write_all(&mut self, positions: &[RawPosition]) -> std::io::Result<()> {
        for pos in positions {
            self.write(pos)?;
        }
        Ok(())
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }

    pub fn positions_written(&self) -> u64 {
        self.positions_written
    }
}