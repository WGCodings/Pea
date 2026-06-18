use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use shakmaty::{Chess, Move};
use crate::engine::corrhist::CorrectionHistoryTable;
use crate::engine::hash::HashState;
use crate::engine::search::context::NNUEState;
use crate::engine::search::ordering::MoveOrdering;
use crate::engine::search::search::{search, SearchStats};
use crate::engine::state::Engine;
use crate::engine::tt::TranspositionTable;
use crate::engine::types::PIECE_VALUES;
use crate::engine::utility::build_search_context;
use crate::nnue::network::Network;
use crate::uci::parser::move_to_uci;

pub struct Threads;

struct SharedTt(*const TranspositionTable);
unsafe impl Send for SharedTt {}

impl SharedTt {
    fn from(tt: &TranspositionTable) -> Self { SharedTt(tt as *const _) }
    unsafe fn get(&self) -> &TranspositionTable { &*self.0 }
}

// ---------------------------------------------------------------------------

impl Threads {
    pub fn search(
        pos:        &Chess,
        engine:     &mut Engine,
        ordering:   &MoveOrdering,
        max_depth:  usize,
        max_nodes:  u64,
        time_limit: Option<Duration>,
        stop:       Arc<AtomicBool>,
        verbose:     bool,
    ) -> (i32, Move, Vec<Option<Move>>,SearchStats) {
        stop.store(false, Ordering::Relaxed);

        let num_threads     = engine.options.threads as usize;
        let params          = &engine.params;
        let rep_stack       = &engine.repetition_stack;
        let network: &'static Network = engine.net;
        let node_count      = Arc::new(AtomicU64::new(0));
        let effective_limit = time_limit.unwrap_or(Duration::from_millis(100));
        let shared_tt       = SharedTt::from(&engine.tt);
        let corrhist_pawn = engine.corrhist_pawn.clone();
        let mut hash_state = HashState::default();
        hash_state.set_from_position(pos);

        let result = std::thread::scope(|s| {
            for thread_id in 1..num_threads {
                let params    = params.clone();
                let rep_stack = rep_stack.clone();
                let stop      = stop.clone();
                let ordering  = MoveOrdering::new(&PIECE_VALUES);
                let nodes     = node_count.clone();
                let tt_ptr    = SharedTt(shared_tt.0);
                let hash_state = hash_state.clone();

                let offset_depth = max_depth + 3 % thread_id;

                s.spawn(move || {
                    let tt         = unsafe { tt_ptr.get() };
                    let nnue_state = NNUEState::new(pos, network);
                    let mut ctx    = build_search_context(
                        tt, CorrectionHistoryTable::default(), hash_state, &params, &ordering, network,
                        rep_stack, nnue_state, stop, nodes,
                        false, Some(effective_limit),
                    );
                    search(pos, &mut ctx, offset_depth, Some(effective_limit), u64::MAX);
                });
            }

            let nnue_state = NNUEState::new(pos, network);
            let mut main_ctx = build_search_context(
                &engine.tt, corrhist_pawn, hash_state, params, ordering, network,
                rep_stack.clone(), nnue_state,
                stop.clone(), node_count,
                verbose, time_limit,
            );
            search(pos, &mut main_ctx, max_depth, time_limit, max_nodes)
        });

        stop.store(true, Ordering::Relaxed);
        result
    }

    pub fn start_ponder(
        pos:          Chess,
        engine:       &Engine,
        stop:         Arc<AtomicBool>,
        is_pondering: Arc<AtomicBool>,
    ) -> std::thread::JoinHandle<()> {
        eprintln!("DEBUG: ponder started");
        stop.store(false, Ordering::Relaxed);

        let num_threads:    usize             = engine.options.threads as usize;
        let shared_tt:      SharedTt          = SharedTt::from(&engine.tt);
        let network:        &'static Network  = engine.net;
        let params                            = engine.params.clone();
        let rep_stack                         = engine.repetition_stack.clone();
        let node_count                        = Arc::new(AtomicU64::new(0));
        let ponder_limit                      = Duration::MAX / 10;
        let corrhist_pawn = engine.corrhist_pawn.clone();
        let mut hash_state = HashState::default();
        hash_state.set_from_position(&pos);

        for thread_id in 1..num_threads {
            let params    = params.clone();
            let rep_stack = rep_stack.clone();
            let stop      = stop.clone();
            let ordering  = MoveOrdering::new(&PIECE_VALUES);
            let nodes     = node_count.clone();
            let tt_ptr    = SharedTt(shared_tt.0);
            let pos       = pos.clone();
            let offset_depth = 64 + 3 % thread_id;
            let hash_state = hash_state.clone();


            std::thread::spawn(move || {
                let tt         = unsafe { tt_ptr.get() };
                let nnue_state = NNUEState::new(&pos, network);
                let mut ctx    = build_search_context(
                    tt, CorrectionHistoryTable::default(),hash_state, &params, &ordering, network,
                    rep_stack, nnue_state, stop, nodes,
                    false, Some(ponder_limit),
                );
                search(&pos, &mut ctx, offset_depth, Some(ponder_limit), u64::MAX);
            });
        }

        let ordering = MoveOrdering::new(&PIECE_VALUES);
        let tt_ptr   = SharedTt(shared_tt.0);

        std::thread::spawn(move || {
            let tt         = unsafe { tt_ptr.get() };
            let nnue_state = NNUEState::new(&pos, network);
            let mut ctx    = build_search_context(
                tt, corrhist_pawn,hash_state, &params, &ordering, network,
                rep_stack, nnue_state,
                stop, node_count,
                true, Some(ponder_limit),
            );

            let (_, mv, _,_) = search(&pos, &mut ctx, 64, Some(ponder_limit), u64::MAX);

            if is_pondering.load(Ordering::Relaxed) {
                println!("bestmove {}", move_to_uci(&mv));
            }

            eprintln!("DEBUG: ponder thread stopped");
        })
    }
}