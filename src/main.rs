mod engine;
mod uci;
mod nnue;
mod datagen;
mod tests;
mod tuner;

use crate::uci::handler::UciHandler;

fn main() {
    UciHandler::new().run();
}