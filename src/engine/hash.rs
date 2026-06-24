use shakmaty::{Chess, Color, Move, Position, Role, Square};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Hash(pub u64);

impl Hash {
    #[inline]
    pub fn toggle_piece(&mut self, role: Role, sq: Square, color: Color) {
        self.0 ^= PIECE_HASHES[color as usize][role as usize][sq as usize];
    }

    #[inline]
    pub fn pawnhash(pos: &Chess) -> Hash {
        let mut hash = Hash(0);
        let board = pos.board();
        for sq in board.pawns() {
            let piece = board.piece_at(sq).unwrap();
            hash.toggle_piece(Role::Pawn, sq, piece.color);
        }
        hash
    }
}
/// Contains all hashes for the positon and for corrhist indexing
/// Passed to SearchContext
#[derive(Clone)]
pub struct HashState {
    pub pawn_hash: Hash,
    pawn_hash_stack : Vec<Hash>,
}

/// Init the state. Keep for every Hash a stack for unmakes
impl Default for HashState {
    fn default() -> Self {
        Self {
            pawn_hash: Hash(0),
            pawn_hash_stack: Vec::with_capacity(256),
        }
    }
}

impl HashState {
    /// Calculate hashes from scratch for a given position
    pub fn set_from_position(&mut self, pos: &Chess) {
        self.pawn_hash = Hash::pawnhash(pos);
    }

    /// Incrementally update all hashes for corrhist and postition zobrist later
    /// More will be added later for different corrhist hashes
    pub fn make_move_hash(&mut self, pos: &Chess, mv: &Move) {
        // Push to stack
        self.pawn_hash_stack.push(self.pawn_hash);
        match mv {
            Move::Normal { role, from, to, capture, promotion } => {
                // Remove pawn from origin square
                if *role == Role::Pawn {
                    let color = pos.turn();
                    self.pawn_hash.toggle_piece(Role::Pawn, *from, color);
                    if promotion.is_none() {
                        self.pawn_hash.toggle_piece(Role::Pawn, *to, color);
                    }
                }

                // Remove captured piece if any
                if let Some(captured_role) = capture {
                    if *captured_role == Role::Pawn {
                        let captured_color = !pos.turn();
                        self.pawn_hash.toggle_piece(Role::Pawn, *to, captured_color);
                    }
                }

            }

            Move::EnPassant { from, to } => {
                let color = pos.turn();

                let captured_sq = Square::from_coords(to.file(), from.rank());

                self.pawn_hash.toggle_piece(Role::Pawn, *from, color);         // remove our pawn
                self.pawn_hash.toggle_piece(Role::Pawn, *to, color);           // add our pawn at dest
                self.pawn_hash.toggle_piece(Role::Pawn, captured_sq, !color); // remove captured pawn
            }
            _ => {}
        }
    }

    /// Unmake move by popping the stack
    pub fn unmake_move_hash(&mut self) {
        if let Some(prev) = self.pawn_hash_stack.pop() {
            self.pawn_hash = prev;
        }
    }
}

// Precomputed hashes for all squares/roles/color combos
static PIECE_HASHES: [[[u64; 64]; 6]; 2] = generate_piece_hashes();

const fn generate_piece_hashes() -> [[[u64; 64]; 6]; 2] {
    let mut table = [[[0u64; 64]; 6]; 2];
    let mut seed: u64 = 0xDEADBEEFCAFEBABE;

    let mut color = 0;
    while color < 2 {
        let mut role = 0;
        while role < 6 {
            let mut sq = 0;
            while sq < 64 {
                seed = splitmix64(seed);
                table[color][role][sq] = seed;
                sq += 1;
            }
            role += 1;
        }
        color += 1;
    }
    table
}

// Algorithm to generate unique hashes
const fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}