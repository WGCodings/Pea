
use shakmaty::Move;

pub const MAX_PLY: usize = 128;

#[derive(Clone, Copy)]
pub struct PvTable {
    moves: [Option<Move>; MAX_PLY],
    len: usize,
}

impl PvTable {
    pub fn new() -> Self {
        Self {
            moves: [None; MAX_PLY],
            len: 0,
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Prepend mv to child's PV and store as our PV
    pub fn add_child_to_parent(&mut self, mv: Move, child: &PvTable) {

        self.moves[0] = Some(mv);
        self.moves[1..][..child.len].copy_from_slice(&child.moves[..child.len]);
        self.len = child.len + 1;

    }

    pub fn best_move(&self) -> Option<Move> {
        self.moves[0]
    }

    pub fn line(&self) -> &[Option<Move>] {
        &self.moves[..self.len]
    }

    pub fn _len(&self) -> usize {
        self.len
    }
}
