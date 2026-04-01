
// Raw position format for data generation output.
// All logic related to the data format lives here.
// A separate convert.rs will handle conversion to bulletformat later.

use std::io::{BufWriter, Write};
use std::fs::{File, OpenOptions};

// ------------------------------------------------------------------ //
// Raw position record                                                  //
// ------------------------------------------------------------------ //

/// One filtered position from a data generation game.
/// Stored as a human-readable / easily parseable text line.
/// Format (pipe-separated):
///   fen|score|wdl|stm|net_id|ply|nodes|pawn_hash
///
/// Fields:
///   fen        — FEN string of the position (without move counters)
///   score      — eval in centipawns from white perspective
///   wdl        — game result from White's perspective: 1.0 / 0.5 / 0.0
///   nodes      — nodes searched when choosing the move
///   pawn_hash  — pawn structure hash (hex) for correction history training
#[derive(Debug, Clone)]
pub struct RawPosition {
    pub fen:       String,
    pub score:     i32,
    pub wdl:       f32,
    pub net_id:    u8,
    pub nodes:     u64,
    pub pawn_hash: u64,
}

impl RawPosition {
    /// Serialize to a single pipe-separated line.
    pub fn to_line(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{:016x}",
            self.fen,
            self.score,
            self.wdl,
            self.net_id,
            self.nodes,
            self.pawn_hash,
        )
    }
}

// ------------------------------------------------------------------ //
// File writer                                                          //
// ------------------------------------------------------------------ //

/// Buffered writer for raw position files.
/// Each thread writes to its own file to avoid locking.
pub struct RawDataWriter {
    writer: BufWriter<File>,
    positions_written: u64,
}

impl RawDataWriter {
    /// Open or create a file for writing. Appends if file exists.
    pub fn open(path: &str) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        Ok(Self {
            writer: BufWriter::with_capacity(1024 * 1024, file), // 1MB buffer
            positions_written: 0,
        })
    }

    /// Write a single position. WDL must be set before calling this.
    pub fn write(&mut self, pos: &RawPosition) -> std::io::Result<()> {
        writeln!(self.writer, "{}", pos.to_line())?;
        self.positions_written += 1;
        Ok(())
    }

    /// Write a batch of positions collected from one game.
    pub fn write_batch(&mut self, positions: &[RawPosition]) -> std::io::Result<()> {
        for pos in positions {
            self.write(pos)?;
        }
        Ok(())
    }

    /// Flush the buffer to disk.
    pub fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }

    pub fn positions_written(&self) -> u64 {
        self.positions_written
    }
}

// ------------------------------------------------------------------ //
// Position file path helpers                                           //
// ------------------------------------------------------------------ //

/// Generate the output file path for a given thread.
pub fn thread_output_path(output_dir: &str, thread_id: usize) -> String {
    format!("{}/positions_thread_{:02}.txt", output_dir, thread_id)
}
