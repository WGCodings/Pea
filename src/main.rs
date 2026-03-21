mod engine;
mod uci;
mod nnue;
mod databuilder;
mod tests;
mod tuner;

use crate::tuner::bounds::Bounds;
use std::cmp;
use std::io::{self, BufRead};
use std::sync::atomic::{Ordering};


use shakmaty::{perft, Chess,  Position};

use crate::uci::{parser::*, state::*};
use crate::engine::search::search::{search, };
use crate::engine::params::Params;
use crate::engine::search::ordering::MoveOrdering;
use crate::engine::time_manager::{compute_time_limit};
use crate::engine::state::*;
use crate::engine::utility::{build_search_context, read_position_from_fen};
use crate::nnue::network::{Network};
use crate::engine::types::{PIECE_VALUES};
use crate::tuner::main::run_spsa;
use crate::tuner::perturb::perturb_params;


fn main() {

    /// Load in nnue and params from yaml file.
    static NNUE: Network = unsafe { std::mem::transmute(*include_bytes!("../nnue/huge1536-0.2-0.9 wdl/2_output_buckets-500/quantised.bin")) };



    /// Initialize uci state (manages commands) and engine state (manages repetition stack, TT and contains params the engine is using)
    let stdin = io::stdin();
    let mut uci_state = UciState::new();
    let mut engine_state = EngineState::new();



    /// Listen to UCI commands
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let cmd = parse_command(&line.as_str());

        match cmd {
            UciCommand::Uci => {
                println!("id name Pea");
                println!("id author Warre G.");
                println!("option name Hash type spin default 256 min 1 max 1024");
                println!("option name Threads type spin default 1 min 1 max 16");
                println!("option name Move Overhead type spin default 10 min 0 max 1000");
                println!("uciok");
            }

            UciCommand::IsReady => println!("readyok"),

            UciCommand::UciNewGame => {
                uci_state.position = Chess::new();
                engine_state.position = Chess::new();
                engine_state.tt.clear();
                engine_state.repetition_stack.clear();
            }

            UciCommand::Position { fen, moves } => {
                if let Some(fen) = fen {
                    engine_state.position = read_position_from_fen(&fen.as_str()).unwrap();
                } else {
                    engine_state.position = Chess::new();
                }
                engine_state.init_history();

                for mv in moves {
                    let m = uci_to_move(&engine_state.position, &mv.as_str());

                    engine_state.position.play_unchecked(m);
                    engine_state.increase_history();
                }
            }

            UciCommand::Go { wtime, btime, winc, binc, movetime, depth} => {
                let max_depth = depth.map_or(64, |d| d as usize );
                let time_limit = compute_time_limit(
                    &engine_state.position,
                    wtime, btime, winc, binc,
                    movetime, depth,
                    engine_state.overhead,
                );

                let ordering = MoveOrdering::new(&PIECE_VALUES);
                let position = &engine_state.position.clone();

                let mut ctx = build_search_context(&mut engine_state, &ordering, &NNUE, time_limit);

                let (_,best_move,_) = search(position, &mut ctx, max_depth, time_limit);

                println!("bestmove {}", move_to_uci(&best_move));
            }

            UciCommand::SetOption { name, value } => {
                if name.as_str().eq_ignore_ascii_case("multipv") {
                    if let Ok(n) = value.as_str().parse::<usize>() {
                        uci_state.multipv = cmp::min(cmp::max(n, 1), 5);
                    }
                }
                if name.as_str().eq_ignore_ascii_case("hash") {
                    if let Ok(mb) = value.as_str().parse::<usize>() {
                        let mb = cmp::min(cmp::max(mb, 1), 1024);
                        engine_state.init_tt(mb);
                    }
                }
                if name.as_str().eq_ignore_ascii_case("move overhead") {
                    if let Ok(overhead) = value.as_str().parse::<usize>() {
                        let overhead = cmp::min(cmp::max(overhead, 1), 1000) as u64;
                        engine_state.set_overhead(overhead);
                    }
                }
                if name.as_str().eq_ignore_ascii_case("threads") {
                    if let Ok(threads) = value.as_str().parse::<usize>() {
                        let threads = cmp::min(cmp::max(threads, 1), 16) as u8;
                        engine_state.set_threads(threads);
                    }
                }
            }

            UciCommand::Stop => {
                uci_state.stop.store(true, Ordering::Relaxed);
            }

            UciCommand::Perft { depth } => {
                let start = std::time::Instant::now();
                let nodes = perft(&engine_state.position, depth);
                let elapsed = start.elapsed().as_millis();
                let nps = (1000*nodes as u128 / elapsed) as u64;

                println!("nodes {}", nodes);
                println!("time {:.3}", elapsed);
                println!("nps {}", nps);
                println!("perftok");
            }

            UciCommand::Quit => break,

            // These are technically not uci commands but they are used for the tuner.
            UciCommand::LoadParams {path} =>{
                let params = Params::load_yaml(&path.as_str());
                engine_state.params = params;
            }
            UciCommand::SaveParams {path} =>{
                engine_state.params.save_yaml(&path.as_str());
            }
            UciCommand::PerturbParams { path, c } => {

                // load base parameters
                let base_params = Params::load_yaml(&path.as_str());

                // load bounds
                let bounds = Bounds::load_yaml("src/tuner/config/bounds.yaml");

                // perturb
                let (theta_plus, theta_minus,_) = perturb_params(&base_params, &bounds, c);

                // save
                theta_plus.save_yaml("src/tuner/config/theta_plus.yaml");
                theta_minus.save_yaml("src/tuner/config/theta_minus.yaml");
            },
            UciCommand::RunSPSA =>{
                run_spsa();
            },

            _ => {}
        }
    }
}


