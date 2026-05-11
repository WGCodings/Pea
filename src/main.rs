mod engine;
mod uci;
mod nnue;
mod datagen;
mod tests;
mod tuner;

use crate::engine::search::ordering::MoveOrdering;
use crate::engine::search::threads::Threads;
use crate::engine::state::Engine;
use crate::engine::types::PIECE_VALUES;
use crate::uci::handler::UciHandler;

fn main() {
    // bench mode for OpenBench — run a fixed search and exit
    if std::env::args().nth(1).as_deref() == Some("bench") {
        let mut engine  = Engine::new();
        let ordering    = MoveOrdering::new(&PIECE_VALUES);
        let position    = engine.position.clone();
        let stop        = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        Threads::search(
            &position, &mut engine, &ordering,
            13, 100_000_000,
            Some(std::time::Duration::from_secs(100)),
            stop,
        );
        return;
    }

    UciHandler::new().run();
}