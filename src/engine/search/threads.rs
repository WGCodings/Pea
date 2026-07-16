use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use shakmaty::{Chess, Move};
use crate::engine::corrhist::CorrectionHistoryTable;
use crate::engine::history::HistoryTables;
use crate::engine::search::context::NNUEState;
use crate::engine::search::ordering::MoveOrdering;
use crate::engine::search::search::{search, SearchStats};
use crate::engine::state::Engine;
use crate::engine::tt::TranspositionTable;
use crate::engine::types::PIECE_VALUES;
use crate::engine::utility::build_search_context;
use crate::nnue::network::Network;
use crate::uci::state::UciState;

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
        uci:         &UciState,
        verbose:     bool,
    ) -> (i32, Move, Vec<Option<Move>>,SearchStats) {

        let stop = uci.stop.clone();
        stop.store(false, Ordering::Relaxed);

        let num_threads     = engine.options.threads as usize;
        let params          = &engine.params;
        let rep_stack       = &engine.repetition_stack;
        let network: &'static Network = engine.net;
        let node_count      = Arc::new(AtomicU64::new(0));
        let effective_limit = time_limit.unwrap_or(Duration::from_millis(100));
        let shared_tt       = SharedTt::from(&engine.tt);
        let corrhist_pawn = engine.corrhist_pawn.clone();
        let corrhist_material = engine.corrhist_material.clone();
        let corrhist_minor = engine.corrhist_minor.clone();
        let corrhist_major = engine.corrhist_major.clone();
        let history_tables = engine.history_tables.clone();

        /*
        let nonzero: Vec<i32> = corrhist_pawn.table.iter()
            .flatten()
            .filter(|&&x| x != 0)
            .copied()
            .collect();
        eprintln!("nonzero: {}, max_abs: {}, mean_abs: {}",
                  nonzero.len(),
                  nonzero.iter().map(|x| x.abs()).max().unwrap_or(0),
                  nonzero.iter().map(|x| x.abs()).sum::<i32>() / nonzero.len().max(1) as i32
        );

         */


        let result = std::thread::scope(|s| {
            for thread_id in 1..num_threads {
                let params    = params.clone();
                let rep_stack = rep_stack.clone();
                let stop      = stop.clone();
                let ordering  = MoveOrdering::new(&PIECE_VALUES);
                let nodes     = node_count.clone();
                let tt_ptr    = SharedTt(shared_tt.0);

                let offset_depth = max_depth + 3 % thread_id;

                s.spawn(move || {
                    let tt         = unsafe { tt_ptr.get() };
                    let nnue_state = NNUEState::new(pos, network);
                    let mut ctx    = build_search_context(
                        tt,
                        CorrectionHistoryTable::new(256,32), // Pawn correction
                        CorrectionHistoryTable::new(256,32), // Material correction
                        CorrectionHistoryTable::new(256,0),  // Minor piece correction
                        CorrectionHistoryTable::new(256,0),  // Major piece correction
                        HistoryTables::new(),
                        &params, &ordering, network,
                        rep_stack, nnue_state, stop, nodes,
                        false, Some(effective_limit),
                    );
                    search(pos, &mut ctx, uci, offset_depth, Some(effective_limit), u64::MAX);
                });
            }

            let nnue_state = NNUEState::new(pos, network);
            let mut main_ctx = build_search_context(
                &engine.tt, corrhist_pawn, corrhist_material, corrhist_minor, corrhist_major, history_tables, params, ordering, network,
                rep_stack.clone(), nnue_state,
                stop.clone(), node_count,
                verbose, time_limit,
            );
            let result = search(pos, &mut main_ctx, uci, max_depth, time_limit, max_nodes);

            // Store history tables inside engine for next search call
            engine.history_tables = main_ctx.history;
            engine.corrhist_pawn = main_ctx.corrhist_pawn;
            engine.corrhist_material = main_ctx.corrhist_material;

            result
        });



        stop.store(true, Ordering::Relaxed);
        result
    }
}