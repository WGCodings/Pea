use shakmaty::{Chess, EnPassantMode, Move, Position};
use shakmaty::zobrist::Zobrist64;
use crate::engine::corrhist::{CorrectionHistoryTable, MaterialKey, MinorsAndKingsKey, PawnKey};
use crate::engine::params::Params;
use crate::engine::tt::TranspositionTable;
use crate::nnue::network::Network;

// ---------------------------------------------------------------------------
// Engine options — purely configuration, no runtime state
// ---------------------------------------------------------------------------

pub struct Options {
    /// Number of search threads.
    pub threads: u8,
    /// Move overhead in milliseconds subtracted from time budget.
    pub move_overhead: u64,
}

impl Options {
    fn default() -> Self {
        Self {
            threads: 1,
            move_overhead: 10,
        }
    }
}

// ---------------------------------------------------------------------------
// Engine — survives the whole game, owns all persistent state
// ---------------------------------------------------------------------------

pub struct Engine {
    pub position:         Chess,
    pub params:           Params,
    pub repetition_stack: Vec<u64>,
    pub tt:               TranspositionTable,
    pub corrhist_pawn:    CorrectionHistoryTable<PawnKey>,
    pub corrhist_material:   CorrectionHistoryTable<MaterialKey>,
    pub corrhist_minor:   CorrectionHistoryTable<MinorsAndKingsKey>,
    pub options:          Options,
    pub net:               &'static Network,
    // Pondering
    pub ponder_move:      Option<Move>,
    pub ponder_thread:    Option<std::thread::JoinHandle<()>>,
}

impl Engine {
    pub fn new() -> Self {
        let params   = Params::load_yaml("src/tuner/config/params_patch.yaml");
        let position = Chess::new();
        let net      = Self::load_network();

        let mut repetition_stack: Vec<u64> = Vec::with_capacity(256);
        let hash = position.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;
        repetition_stack.push(hash);

        Self {
            position,
            repetition_stack,
            tt:            TranspositionTable::new(16),
            corrhist_pawn:      CorrectionHistoryTable::new(256,32),
            corrhist_material:      CorrectionHistoryTable::new(256,32),
            corrhist_minor:      CorrectionHistoryTable::new(256,40),
            params,
            net,
            ponder_move:   None,
            ponder_thread: None,
            options:       Options::default(),

        }
    }

    fn load_network() -> &'static Network {
        Network::load()
    }

    // -----------------------------------------------------------------------
    // History / repetition stack
    // -----------------------------------------------------------------------

    pub fn init_history(&mut self) {
        self.repetition_stack.clear();
        self.push_history();
    }

    pub fn push_history(&mut self) {
        let hash = self.position.zobrist_hash::<Zobrist64>(EnPassantMode::Legal).0;
        self.repetition_stack.push(hash);
    }

    // -----------------------------------------------------------------------
    // Setters driven by setoption
    // -----------------------------------------------------------------------

    pub fn resize_tt(&mut self, mb: usize) {
        self.tt = TranspositionTable::new(mb);
    }

    pub fn set_move_overhead(&mut self, ms: u64) {
        self.options.move_overhead = ms;
    }

    pub fn set_threads(&mut self, n: u8) {
        self.options.threads = n;
    }

    // -----------------------------------------------------------------------
    // Pondering helpers
    // -----------------------------------------------------------------------

    pub fn stop_ponder_thread(&mut self) {
        if let Some(handle) = self.ponder_thread.take() {
            handle.join().ok();
        }
    }
}