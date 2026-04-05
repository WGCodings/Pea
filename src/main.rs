mod engine;
mod uci;
mod nnue;
mod datagen;
mod tests;
mod tuner;


use crate::tuner::bounds::Bounds;
use std::cmp;
use std::io::{self, BufRead};


use shakmaty::{perft, Chess, Move, Position};
use crate::datagen::datagen_main::run_datagen;
use crate::datagen::init::_generate_random_network;
use crate::uci::{parser::*, state::*};

use crate::engine::params::Params;
use crate::engine::search::ordering::MoveOrdering;

use crate::engine::search::threads::Threads;
use crate::engine::time_manager::{compute_time_limit};
use crate::engine::state::*;
use crate::engine::utility::{ read_position_from_fen};
use crate::nnue::network::{Network};
use crate::engine::types::{PIECE_VALUES};
use crate::tuner::main::run_spsa;
use crate::tuner::perturb::perturb_params;


fn main() {

    // Load in nnue
    static NNUE: Network = unsafe { std::mem::transmute(*include_bytes!("../nnue/run3_net_0/run3_net_0-7/quantised.bin")) };

    //_generate_random_network("C:/Users/warre/RustroverProjects/FastPeaPea/nnue/net_0.bin", 1536, 8);
    //_generate_random_network("C:/Users/warre/RustroverProjects/FastPeaPea/nnue/net_1.bin", 64, 1);
    // Initialize uci state (manages commands) and engine state (manages repetition stack, TT and contains params the engine is using)
    let stdin = io::stdin();
    let mut uci_state = UciState::new();
    let mut engine_state = EngineState::new();



    // Listen to UCI commands
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let cmd = parse_command(&line.as_str());

        match cmd {
            UciCommand::Uci => {
                println!("id name Pea 1.0");
                println!("id author Warre G.");
                println!("option name Hash type spin default 256 min 1 max 1024");
                println!("option name Threads type spin default 1 min 1 max 64");
                println!("option name Move Overhead type spin default 10 min 0 max 1000");
                println!("option name Ponder type check default false");
                println!("uciok");
            }

            UciCommand::IsReady => println!("readyok"),

            UciCommand::UciNewGame => {
                // Stop all running threads
                uci_state.stop();
                engine_state.stop_ponder_thread();
                uci_state.reset_stop();
                // Reset position, tt and repetition stack
                engine_state.position = Chess::new();
                engine_state.tt.clear();
                engine_state.repetition_stack.clear();
            }

            UciCommand::Position { fen, moves } => {
                // Stop any ponder search before updating position
                uci_state.reset_stop();            // reset for next search

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

            UciCommand::Go { wtime, btime, winc, binc, movetime, depth, nodes, ponder} => {

                // Save time for if we ponderhit, then we search with saved time left
                uci_state.save_time(wtime, btime, winc, binc);
                uci_state.reset_stop();

                let max_depth = depth.map_or(64, |d| d as usize );
                let max_nodes = nodes.unwrap_or(u64::MAX);
                let ordering = MoveOrdering::new(&PIECE_VALUES);
                let position = engine_state.position.clone();

                if ponder && uci_state.ponder_enabled  {
                    uci_state.set_pondering(true);
                    // if we get a command like go ponder x, so think on opponent move
                    engine_state.ponder_thread = Some(Threads::start_ponder(
                        position, &engine_state, &NNUE, uci_state.stop.clone(),uci_state.is_pondering.clone()
                    ));

                } else {
                    // think on own move
                    let time_limit = compute_time_limit(
                        &engine_state.position,
                        wtime, btime, winc, binc,
                        movetime, depth,
                        engine_state.overhead,
                    );

                    let (_, best_move, pv) = Threads::search(
                        &position, &mut engine_state, &ordering, &NNUE,
                        max_depth, max_nodes, time_limit, uci_state.stop.clone(),
                    );
                    print_bestmove(best_move, &pv, &mut engine_state, &uci_state);
                }
            }

            UciCommand::PonderHit if uci_state.ponder_enabled => {
                // If we get ponderhit we stop our ponder thread and start a main thread with the tt filled correctly
                //eprintln!("DEBUG: ponderhit received");
                uci_state.set_pondering(false); // dont print bestmove when stopping ponder thread
                uci_state.stop();
                engine_state.stop_ponder_thread();
                uci_state.reset_stop();


                let time_limit = compute_time_limit(
                    &engine_state.position,
                    uci_state.last_wtime, uci_state.last_btime,
                    uci_state.last_winc,  uci_state.last_binc,
                    None, None, engine_state.overhead,
                );
                //println!("time limit : {:?}", time_limit);

                let ordering = MoveOrdering::new(&PIECE_VALUES);
                let position = engine_state.position.clone();
                let (_, best_move, pv) = Threads::search(
                    &position, &mut engine_state, &ordering,
                    &NNUE, 64, u64::MAX, time_limit, uci_state.stop.clone(),
                );

                print_bestmove(best_move, &pv, &mut engine_state, &uci_state);
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
                        let threads = cmp::min(cmp::max(threads, 1), 64) as u8;
                        engine_state.set_threads(threads);
                    }
                }
                if name.as_str().eq_ignore_ascii_case("ponder") {
                    uci_state.ponder_enabled = value.as_str().eq_ignore_ascii_case("true");
                }
            }

            UciCommand::Stop => {
                //eprintln!("DEBUG: stop received");
                uci_state.stop();
                engine_state.stop_ponder_thread();
                uci_state.set_pondering(false);

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
            UciCommand::DataGen =>{
                run_datagen();
            }
            _ => {}
        }
    }
}

fn print_bestmove(best_move: Move, pv: &Vec<Option<Move>>, engine_state: &mut EngineState, uci_state: &UciState) {
    engine_state.ponder_move = pv.get(1).and_then(|m| *m);

    match (uci_state.ponder_enabled, engine_state.ponder_move) {
        (true, Some(pm)) => println!("bestmove {} ponder {}", move_to_uci(&best_move), move_to_uci(&pm)),
        _ => println!("bestmove {}", move_to_uci(&best_move)),
    }
}
