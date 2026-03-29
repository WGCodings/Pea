
use shakmaty::{Chess, EnPassantMode, Move, Position};
use shakmaty::zobrist::{Zobrist64};
use crate::engine::params::Params;
use crate::engine::tt::TranspositionTable;

// =====================================================================================================================//
// STATE OF THE ENGINE, SURVIVES DURING WHOLE GAME
// NEEDED FOR TT THREADS PARAMS PONDERING POSITION ETC
// INITIALISES ENGINE
// =====================================================================================================================//
pub struct EngineState {
    pub position: Chess,
    pub params : Params,
    pub repetition_stack: Vec<u64>,
    pub tt: TranspositionTable,
    // uci options
    pub overhead : u64,
    pub threads : u8,
    // Pondering
    pub ponder_move:   Option<Move>,

    pub ponder_thread: Option<std::thread::JoinHandle<()>>,

}

impl EngineState {
    pub fn new() -> Self {
        let params = Params::load_yaml("src/tuner/config/params_patch.yaml");
        let position = Chess::new();
        let mut repetition_stack: Vec<u64> = Vec::with_capacity(256);

        let hash = position.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;

        repetition_stack.push(hash);

        Self {
            position,
            repetition_stack,
            tt: TranspositionTable::new(256), // Default 256mb
            params,
            overhead: 10, // 10 ms overhead on each move
            threads: 1,
            ponder_move: None,
            ponder_thread: None,
        }
    }

    pub fn init_history(&mut self) {
        self.repetition_stack.clear();
        self.increase_history()
    }
    pub fn increase_history(&mut self) {
        let hash = self.position.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;
        self.repetition_stack.push(hash);
    }
    pub fn init_tt(&mut self, tt_size: usize) {
        self.tt = TranspositionTable::new(tt_size);
    }
    pub fn set_overhead(&mut self, overhead: u64) {
        self.overhead = overhead;
    }
    pub fn set_threads(&mut self, threads: u8) {
        self.threads = threads;
    }
    pub fn stop_ponder_thread(&mut self) {
        if let Some(h) = self.ponder_thread.take() {
            h.join().ok();
        }
    }
}