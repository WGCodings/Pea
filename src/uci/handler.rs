use std::cmp;
use shakmaty::{perft, Chess, Position};

use crate::engine::params::Params;
use crate::engine::search::ordering::MoveOrdering;
use crate::engine::state::Engine;
use crate::engine::time_manager::compute_time_limit;
use crate::engine::types::PIECE_VALUES;
use crate::engine::utility::{print_bestmove, read_position_from_fen};
use crate::engine::search::threads::Threads;
use crate::tuner::bounds::Bounds;
use crate::tuner::main::run_spsa;
use crate::tuner::perturb::perturb_params;
use crate::datagen::datagen_main::run_datagen;
use crate::uci::parser::{uci_to_move, UciCommand};
use crate::uci::state::UciState;

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
            UciCommand::Go { wtime, btime, winc, binc, movetime, depth, nodes, ponder }
            => self.on_go(wtime, btime, winc, binc, movetime, depth, nodes, ponder),
            UciCommand::PonderHit if self.uci.ponder_enabled
            => self.on_ponderhit(),
            UciCommand::Stop        => self.on_stop(),
            UciCommand::SetOption { name, value } => self.on_setoption(name, value),
            UciCommand::Perft { depth } => self.on_perft(depth),
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

    fn on_uci(&self) {
        println!("id name Pea 9.0");
        println!("id author Warre G.");
        println!("option name Hash type spin default 256 min 1 max 1024");
        println!("option name Threads type spin default 1 min 1 max 128");
        println!("option name Move Overhead type spin default 10 min 0 max 1000");
        println!("option name Ponder type check default false");
        println!("uciok");
    }

    fn on_isready(&self) {
        println!("readyok");
    }

    fn on_ucinewgame(&mut self) {
        self.uci.stop();
        self.engine.stop_ponder_thread();
        self.uci.reset_stop();

        self.engine.position = Chess::new();
        self.engine.tt.clear();
        self.engine.repetition_stack.clear();
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
        nodes: Option<u64>, ponder: bool,
    ) {
        self.uci.save_time(wtime, btime, winc, binc);
        self.uci.reset_stop();

        let max_depth = depth.map_or(64, |d| d as usize);
        let max_nodes = nodes.unwrap_or(u64::MAX);
        let ordering  = MoveOrdering::new(&PIECE_VALUES);
        let position  = self.engine.position.clone();

        if ponder && self.uci.ponder_enabled {
            self.uci.set_pondering(true);
            self.engine.ponder_thread = Some(Threads::start_ponder(
                position,
                &self.engine,
                self.uci.stop.clone(),
                self.uci.is_pondering.clone(),
            ));
        } else {
            let time_limit = compute_time_limit(
                &self.engine.position,
                wtime, btime, winc, binc,
                movetime, depth,
                self.engine.options.move_overhead,
            );

            let (_, best_move, pv) = Threads::search(
                &position, &mut self.engine, &ordering,
                max_depth, max_nodes, time_limit, self.uci.stop.clone(),
            );
            print_bestmove(best_move, &pv, &mut self.engine, &self.uci);
        }
    }

    fn on_ponderhit(&mut self) {
        self.uci.set_pondering(false);
        self.uci.stop();
        self.engine.stop_ponder_thread();
        self.uci.reset_stop();

        let time_limit = compute_time_limit(
            &self.engine.position,
            self.uci.last_wtime, self.uci.last_btime,
            self.uci.last_winc,  self.uci.last_binc,
            None, None,
            self.engine.options.move_overhead,
        );

        let ordering = MoveOrdering::new(&PIECE_VALUES);
        let position = self.engine.position.clone();
        let (_, best_move, pv) = Threads::search(
            &position, &mut self.engine, &ordering,
            64, u64::MAX, time_limit, self.uci.stop.clone(),
        );
        print_bestmove(best_move, &pv, &mut self.engine, &self.uci);
    }

    fn on_stop(&mut self) {
        self.uci.stop();
        self.engine.stop_ponder_thread();
        self.uci.set_pondering(false);
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
        } else if n.eq_ignore_ascii_case("ponder") {
            self.uci.ponder_enabled = v.eq_ignore_ascii_case("true");
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