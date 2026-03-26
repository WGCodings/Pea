use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use shakmaty::{Chess, Move};
use crate::engine::search::context::NNUEState;
use crate::engine::search::ordering::MoveOrdering;
use crate::engine::search::search::{search};
use crate::engine::state::EngineState;
use crate::engine::tt::TranspositionTable;
use crate::engine::types::PIECE_VALUES;
use crate::engine::utility::{build_main_context, build_search_context};
use crate::nnue::network::Network;
use crate::uci::parser::move_to_uci;

pub struct Threads;

// =====================================================================================================================//
// HANDLES DIFFERNET THREADS NEEDED FOR LAZY SMP                                                                        //
// =====================================================================================================================//

impl Threads {
    pub fn search(
        pos: &Chess,
        engine_state: &mut EngineState,
        ordering: &MoveOrdering,
        network: &'static Network,
        max_depth: usize,
        time_limit: Option<Duration>,
        stop: Arc<AtomicBool>,
    ) -> (i32, Move, Vec<Option<Move>>) {



        // Shared stop signal sahred by threads
        (*stop).store(false, Ordering::Relaxed);

        let num_threads         = engine_state.threads as usize;
        let tt_ptr              = &engine_state.tt as *const TranspositionTable as usize;
        let params                     = engine_state.params.clone();
        let rep_stack                  = engine_state.repetition_stack.clone();
        let node_count  = Arc::new(AtomicU64::new(0));

        //eprintln!("DEBUG: search started, threads={}", num_threads);

        // Spawn helper threads
        let threads: Vec<_> = (1..num_threads).map(|thread_id| {
            let pos       = pos.clone();
            let params    = params.clone();
            let rep_stack = rep_stack.clone();
            let stop      = stop.clone();
            let ordering  = MoveOrdering::new(&PIECE_VALUES);
            let nodes = node_count.clone();

            std::thread::spawn(move || {
                let tt = unsafe { &*(tt_ptr as *const TranspositionTable) };
                let nnue_state = NNUEState::new(&pos, network);
                let mut ctx = build_search_context(
                    tt, &params, &ordering, network,
                    rep_stack, nnue_state, stop, nodes, false,time_limit);
                // Offset depth to diversify search
                let offset_depth = max_depth+3%thread_id;
                //println!("Starting helper thread {}...", thread_id);
                search(&pos, &mut ctx, offset_depth, time_limit);
            })
        }).collect();

        // Main thread
        let mut main_ctx = build_main_context(
            engine_state, ordering, network,
            stop.clone(), node_count.clone(), time_limit);
        //println!("Starting main thread.");
        let (score, mv, pv) = search(pos, &mut main_ctx, max_depth, time_limit);

        // Stop helpers
        (*stop).store(true, Ordering::Relaxed);
        for thread in threads { thread.join().ok(); }

        //eprintln!("DEBUG: search finished.");

        (score, mv, pv)
    }

    // =====================================================================================================================//
    //  PONDERING THREAD, DOES NOT WORK 100% CORRECT YET                                                                    //
    // =====================================================================================================================//
    // TODO SIMPLIFY THESE FUNCTIONS OR COMBINE

    pub fn start_ponder(
        pos: Chess,
        engine_state: &EngineState,
        network: &'static Network,
        stop: Arc<AtomicBool>,
        is_pondering: Arc<AtomicBool>,
    ) -> std::thread::JoinHandle<()> {

        eprintln!("DEBUG: ponder started");

        (*stop).store(false, Ordering::Relaxed);

        let tt_ptr                = &engine_state.tt as *const TranspositionTable as usize;
        let params                      = engine_state.params.clone();
        let rep_stack                   = engine_state.repetition_stack.clone();
        let node_count    = Arc::new(AtomicU64::new(0));
        let ordering        = MoveOrdering::new(&PIECE_VALUES);
        let num_threads           = engine_state.threads as usize;

        // Spawn helper ponder threads
        for _ in 1..num_threads {
            let pos       = pos.clone();
            let params    = params.clone();
            let rep_stack = rep_stack.clone();
            let stop      = stop.clone();
            let ordering  = MoveOrdering::new(&PIECE_VALUES);
            let nodes     = node_count.clone();

            std::thread::spawn(move || {
                let tt = unsafe { &*(tt_ptr as *const TranspositionTable) };
                let nnue_state = NNUEState::new(&pos, network);
                let mut ctx = build_search_context(
                    tt, &params, &ordering, network,
                    rep_stack, nnue_state, stop, nodes, false,
                    Some(Duration::MAX / 10),
                );
                search(&pos, &mut ctx, 64, Some(Duration::MAX / 10));
                eprintln!("DEBUG: helper ponder thread stopped");
            });
        }



        // Main ponder thread
        std::thread::spawn(move || {
            let tt = unsafe { &*(tt_ptr as *const TranspositionTable) };
            let nnue_state = NNUEState::new(&pos, network);
            let mut ctx = build_search_context(
                tt, &params, &ordering, network,
                rep_stack, nnue_state, stop, node_count, true,
                Some(Duration::MAX / 10),
            );
            let (_, mv, _) = search(&pos, &mut ctx, 64, Some(Duration::MAX / 10));

            let is_pondering = (*is_pondering).load(Ordering::Relaxed);

            if is_pondering {
                println!("bestmove {}", move_to_uci(&mv));
            }

            eprintln!("DEBUG: main ponder thread stopped");
        })
    }
}