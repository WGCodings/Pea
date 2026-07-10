use shakmaty::{Chess, Color, Position, Role, Square};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Hash(pub u64);

impl Hash {
    #[inline]
    pub fn toggle_piece(&mut self, role: Role, sq: Square, color: Color) {
        self.0 ^= PIECE_HASHES[color as usize][role as usize-1][sq as usize];
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

    #[inline]
    pub fn materialhash(pos: &Chess) -> Hash {
        let mut hash = Hash(0);
        let board = pos.board();

        for color in [Color::White, Color::Black] {
            for role in [Role::Pawn, Role::Knight, Role::Bishop, Role::Rook, Role::Queen] {
                let count = (board.by_color(color) & board.by_role(role)).count();
                hash.0 ^= PIECE_HASHES[color as usize][role as usize-1][count];
            }
        }
        hash
    }
    #[inline]
    pub fn minors_and_kings_hash(pos: &Chess) -> Hash {
        let mut hash = Hash(0);
        let board = pos.board();
        let minors = board.knights() | board.bishops();

        if minors.is_empty(){
            return hash
        }

        for sq in  minors | board.kings()   {
            let piece = board.piece_at(sq).unwrap();
            let role = board.role_at(sq).unwrap();

            hash.toggle_piece(role, sq, piece.color);
        }
        hash
    }

    #[inline]
    pub fn majors_and_kings_hash(pos: &Chess) -> Hash {
        let mut hash = Hash(0);
        let board = pos.board();
        for sq in  board.rooks_and_queens() | board.kings()   {
            let piece = board.piece_at(sq).unwrap();
            let role = board.role_at(sq).unwrap();

            hash.toggle_piece(role, sq, piece.color);
        }
        hash
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