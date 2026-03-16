mod engine;
mod uci;
mod nnue;
mod databuilder;
mod tests;
mod tuner;

use crate::tuner::bounds::Bounds;
use std::cmp;
use std::io::{self, BufRead};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use shakmaty::{perft, Chess, Color, Move, Position};

use crate::uci::{parser::*, state::*};
use crate::engine::search::search::{search, SearchStats};
use crate::engine::params::Params;
use crate::engine::search::ordering::MoveOrdering;
use crate::engine::search::context::*;
use crate::engine::time_manager::{compute_time_limit, compute_time_limit_2};
use crate::engine::state::*;
use crate::engine::utility::read_position_from_fen;
use crate::nnue::network::{Network};
use crate::engine::types::{MAX_PLY_CONTINUATION_HISTORY, PIECE_VALUES};
use crate::tuner::main::run_spsa;
use crate::tuner::perturb::perturb_params;


fn main() {

    //static NNUE: Network = unsafe { std::mem::transmute(*include_bytes!("../nnue/huge768nowdl/2_output_buckets-100/quantised.bin")) };
    static NNUE: Network = unsafe { std::mem::transmute(*include_bytes!("../nnue/huge1536-0.2-0.9 wdl/2_output_buckets-500/quantised.bin")) };

    let stdin = io::stdin();
    let mut uci_state = UciState::new();
    let params = Params::load_yaml("C:/Users/warre/RustroverProjects/FastPeaPea/src/tuner/config/params.yaml");
    let mut engine_state = EngineState::new(256,params); // TT Size in MB

    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let cmd = parse_command(&line.as_str());

        match cmd {
            UciCommand::Uci => {
                println!("id name FastPeaPea");
                println!("id author Warre G.");
                println!("option name MultiPV type spin default 1 min 1 max 5");
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
                uci_state.stop.store(false, Ordering::Relaxed);

                let max_depth = if let Some(d) = depth { d as usize } else {64};

                let remaining = match engine_state.position.turn() {
                    Color::White => wtime.map(Duration::from_millis),
                    Color::Black => btime.map(Duration::from_millis),
                };

                let increment = match engine_state.position.turn() {
                    Color::White => winc.map(Duration::from_millis),
                    Color::Black => binc.map(Duration::from_millis),
                };

                let time_limit = if let Some(ms) = movetime {
                    Some(Duration::from_millis(ms))
                } else {
                    if max_depth != 64 {
                        Some(Duration::MAX / 10)
                    } else {
                        Some(compute_time_limit_2(
                            &engine_state.position,
                            remaining,
                            increment,
                        ))
                    }
                };

                let ordering = MoveOrdering::new(&PIECE_VALUES);
                let repetition_stack = &engine_state.repetition_stack;

                let nnue_state = NNUEState::new(&engine_state.position, &NNUE);

                let stack = Stack{
                    moves: [None;128],
                    evals: [0;128],
                    double_exts: [0;128],
                };

                let mut ctx = SearchContext {
                    start_time: Instant::now(),
                    time_limit : time_limit.unwrap_or(Duration::from_millis(100)),
                    stop: AtomicBool::new(false),
                    params: &engine_state.params,
                    ordering: &ordering,
                    stats: SearchStats::default(),
                    repetition_stack: repetition_stack.to_vec(),
                    tt: &mut engine_state.tt,
                    nnue: nnue_state,
                    network: &NNUE,
                    killers: [[None; 3]; 128],
                    history: [[[0; 64]; 64]; 2],
                    continuation_history: Box::new([[[[[0; 64]; 6]; 64]; 6]; MAX_PLY_CONTINUATION_HISTORY]),
                    stack,
                    excluded_move: [None; 128],
                };



                let (best_score,best_move,pv) = search(
                    &engine_state.position,
                    &mut ctx,
                    max_depth,
                    time_limit,
                );



                let stats = ctx.stats;
                let tt_occupancy = ctx.tt.tt_occupancy();


                let elapsed = stats.duration.as_secs_f64();
                let elapsed_millis = stats.duration.as_millis();

                let nps = if elapsed > 0.0 {
                    (stats.nodes as f64 / elapsed) as u64
                } else {
                    0
                };

                let pv_string = pv_to_string(pv.line());

                println!(
                    "info depth {:.0} seldepth {} score cp {} nodes {} nps {} hashfull {} time {} SE {} pv {} ",
                    stats.completed_depth,
                    stats.seldepth,
                    best_score,
                    stats.nodes,
                    nps,
                    tt_occupancy,
                    elapsed_millis,
                    stats.singular_extensions,
                    pv_string,

                );


                println!("bestmove {}", move_to_uci(&best_move));
            }

            UciCommand::SetOption { name, value } => {
                if name.as_str().eq_ignore_ascii_case("multipv") {
                    if let Ok(n) = value.as_str().parse::<usize>() {
                        uci_state.multipv = cmp::min(cmp::max(n,1),5);
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
                let base_params = Params::load_yaml(&path);

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


fn pv_to_string(line: &[Option<Move>]) -> String {
    line.iter()
        .filter_map(|mv| *mv)
        .map(|mv| move_to_uci(&mv))
        .collect::<Vec<_>>()
        .join(" ")
}
