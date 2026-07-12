use std::cmp;
use std::time::Duration;
use shakmaty::{perft, Chess, Position};

use crate::engine::params::Params;
use crate::engine::search::ordering::MoveOrdering;
use crate::engine::state::Engine;
use crate::engine::time_manager::compute_time_limit;
use crate::engine::types::PIECE_VALUES;
use crate::engine::utility::{read_position_from_fen};
use crate::engine::search::threads::Threads;
use crate::tuner::bounds::Bounds;
use crate::tuner::main::run_spsa;
use crate::tuner::perturb::perturb_params;
use crate::datagen::datagen_main::run_datagen;
use crate::uci::parser::{move_to_uci, uci_to_move, UciCommand};
use crate::uci::state::UciState;
const BENCH_FENS: &str = include_str!("../../assets/bench.txt");

// ---------------------------------------------------------------------------
// UciHandler owns both uci state and engine state.
// main() creates one instance and calls run().
// ---------------------------------------------------------------------------

pub struct UciHandler {
    uci:    UciState,
    engine: Engine,
}

impl UciHandler {
    pub fn new() -> Self {
        Self {
            uci:    UciState::new(),
            engine: Engine::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Main loop
    // -----------------------------------------------------------------------

    pub fn run(&mut self) {

        // bench mode for OpenBench — run a fixed search
        // Must change to proper bench suite but i am lazy
        if std::env::args().nth(1).as_deref() == Some("bench") {
            self.on_bench();
            return;
        }

        use std::io::{self, BufRead};
        let stdin = io::stdin();

        for line in stdin.lock().lines() {
            let line = line.unwrap();
            let cmd  = crate::uci::parser::parse_command(&line);

            if self.handle(cmd) == LoopControl::Break {
                break;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Dispatch — returns Break only on `quit`
    // -----------------------------------------------------------------------

    fn handle(&mut self, cmd: UciCommand) -> LoopControl {
        match cmd {
            UciCommand::Uci         => self.on_uci(),
            UciCommand::IsReady     => self.on_isready(),
            UciCommand::UciNewGame  => self.on_ucinewgame(),
            UciCommand::Position { fen, moves } => self.on_position(fen, moves),
            UciCommand::Go { wtime, btime, winc, binc, movetime, depth, nodes }
            => self.on_go(wtime, btime, winc, binc, movetime, depth, nodes),
            UciCommand::Stop        => self.on_stop(),
            UciCommand::SetOption { name, value } => self.on_setoption(name, value),
            UciCommand::Perft { depth } => self.on_perft(depth),
            UciCommand::Bench => self.on_bench(),
            UciCommand::Quit        => return LoopControl::Break,
            // Non-standard tuner / datagen commands
            UciCommand::LoadParams    { path }    => self.on_load_params(path),
            UciCommand::SaveParams    { path }    => self.on_save_params(path),
            UciCommand::PerturbParams { path, c } => self.on_perturb_params(path, c),
            UciCommand::RunSPSA       => { run_spsa(); }
            UciCommand::DataGen       => { run_datagen(); }
            _                         => {}
        }
        LoopControl::Continue
    }

    // -----------------------------------------------------------------------
    // UCI command handlers
    // -----------------------------------------------------------------------

    fn on_bench(&mut self) {
        use std::time::Instant;

        let ordering  = MoveOrdering::new(&PIECE_VALUES);

        let mut total_nodes: u64 = 0;
        let mut total_time:  u128 = 0;

        for fen in BENCH_FENS.lines().filter(|l| !l.trim().is_empty()) {
            let position = match read_position_from_fen(fen) {
                Some(p) => p,
                None    => continue,
            };

            self.engine.position = position.clone();
            self.engine.init_history();
            self.engine.tt.clear();

            let timer = Instant::now();
            let (_, _, _,stats) = Threads::search(
                &position,
                &mut self.engine,
                &ordering,
                11,
                u64::MAX,
                Some(Duration::MAX/10),
                &self.uci,
                false
            );
            total_time  += timer.elapsed().as_millis();
            total_nodes += stats.nodes;
        }

        let nps = total_nodes * 1000 / (total_time as u64).max(1);
        println!("Bench: {total_nodes} nodes {nps} nps");
    }

    fn on_uci(&self) {
        println!("id name Pea 9.1");
        println!("id author Warre G.");
        println!("option name Hash type spin default 16 min 1 max 1024");
        println!("option name Threads type spin default 1 min 1 max 128");
        println!("option name Move Overhead type spin default 10 min 0 max 1000");
        println!("option name UCI_ShowWDL type check default true");
        println!("option name NormalizeScore type check default true");
        println!("uciok");
    }

    fn on_isready(&self) {
        println!("readyok");
    }

    fn on_ucinewgame(&mut self) {
        self.uci.stop();
        self.uci.reset_stop();

        self.engine.position = Chess::new();
        self.engine.tt.clear();
        self.engine.repetition_stack.clear();
        self.engine.corrhist_pawn.clear();
    }

    fn on_position(&mut self, fen: Option<String>, moves: Vec<String>) {
        self.uci.reset_stop();

        self.engine.position = match fen {
            Some(f) => read_position_from_fen(&f).unwrap(),
            None    => Chess::new(),
        };
        self.engine.init_history();

        for mv in moves {
            let m = uci_to_move(&self.engine.position, &mv);
            self.engine.position.play_unchecked(m);
            self.engine.push_history();
        }
    }

    fn on_go(
        &mut self,
        wtime: Option<u64>, btime: Option<u64>,
        winc:  Option<u64>, binc:  Option<u64>,
        movetime: Option<u64>, depth: Option<u32>,
        nodes: Option<u64>,
    ) {
        self.uci.save_time(wtime, btime, winc, binc);
        self.uci.reset_stop();

        let max_depth = depth.map_or(64, |d| d as usize);
        let max_nodes = nodes.unwrap_or(u64::MAX);
        let ordering  = MoveOrdering::new(&PIECE_VALUES);
        let position  = self.engine.position.clone();

        let time_limit = compute_time_limit(
            &self.engine.position,
            wtime, btime, winc, binc,
            movetime, depth,
            self.engine.options.move_overhead,
        );

        let (_, best_move, _,_) = Threads::search(
            &position, &mut self.engine, &ordering,
            max_depth, max_nodes, time_limit, &self.uci,true
        );

        println!("bestmove {}", move_to_uci(&best_move));
    }

    fn on_stop(&mut self) {
        self.uci.stop();
    }

    fn on_setoption(&mut self, name: String, value: String) {
        let n = name.as_str();
        let v = value.as_str();

        if n.eq_ignore_ascii_case("multipv") {
            if let Ok(x) = v.parse::<usize>() {
                self.uci.multipv = cmp::min(cmp::max(x, 1), 5);
            }
        } else if n.eq_ignore_ascii_case("hash") {
            if let Ok(mb) = v.parse::<usize>() {
                self.engine.resize_tt(cmp::min(cmp::max(mb, 1), 1024));
            }
        } else if n.eq_ignore_ascii_case("move overhead") {
            if let Ok(ms) = v.parse::<u64>() {
                self.engine.set_move_overhead(cmp::min(cmp::max(ms, 1), 1000));
            }
        } else if n.eq_ignore_ascii_case("threads") {
            if let Ok(t) = v.parse::<u8>() {
                self.engine.set_threads(cmp::min(cmp::max(t, 1), 128));
            }
        } else if n.eq_ignore_ascii_case("normalizescore") {
            self.uci.normalize_score = v.eq_ignore_ascii_case("true");
        }
        else if n.eq_ignore_ascii_case("uci_showwdl") {
            self.uci.uci_show_wdl = v.eq_ignore_ascii_case("true");
        }
    }

    fn on_perft(&self, depth: u32) {
        let start   = std::time::Instant::now();
        let nodes   = perft(&self.engine.position, depth);
        let elapsed = start.elapsed().as_millis();
        let nps     = (1000 * nodes as u128 / elapsed.max(1)) as u64;

        println!("nodes {}", nodes);
        println!("time {}", elapsed);
        println!("nps {}", nps);
        println!("perftok");
    }

    // -----------------------------------------------------------------------
    // Tuner / datagen commands (non-standard)
    // -----------------------------------------------------------------------

    fn on_load_params(&mut self, path: String) {
        self.engine.params = Params::load_yaml(&path);
    }

    fn on_save_params(&self, path: String) {
        self.engine.params.save_yaml(&path);
    }

    fn on_perturb_params(&self, path: String, c: f64) {
        let base   = Params::load_yaml(&path);
        let bounds = Bounds::load_yaml("src/tuner/config/bounds.yaml");
        let (theta_plus, theta_minus, _) = perturb_params(&base, &bounds, c);
        theta_plus .save_yaml("src/tuner/config/theta_plus.yaml");
        theta_minus.save_yaml("src/tuner/config/theta_minus.yaml");
    }
}

// ---------------------------------------------------------------------------

#[derive(PartialEq)]
enum LoopControl { Continue, Break }