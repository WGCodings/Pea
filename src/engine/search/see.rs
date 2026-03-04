use shakmaty::{
    attacks::{bishop_attacks, rook_attacks},
    Bitboard, Board, Chess, Color, Move, Position, Role, Square,
};

/// Piece values used by SEE.
/// Indexing uses `role as usize - 1`.
const PIECE_VALUES: [i16; 7] = [
    100,   // Pawn
    300,   // Knight
    300,   // Bishop
    500,   // Rook
    900,   // Queen
    10000, // King (large finite value)
    0,     // Empty square (quiet move target)
];

/// Static Exchange Evaluation (SEE).
///
/// Returns the material gain/loss of playing `mv` assuming optimal
/// captures on the target square by both sides.
///
/// Handles:
/// - Promotions
/// - En passant
/// - X-ray attacks
/// - Quiet moves
///
pub fn see(pos: &Chess, mv: Move) -> i16 {
    let board = pos.board();
    let to = mv.to();
    let from = mv.from().unwrap();
    let mut occupied = board.occupied();

    let is_quiet = !mv.is_capture() && !mv.is_promotion();

    // Handle en passant
    if mv.is_en_passant() {
        let ep_sq = Square::from_coords(to.file(), from.rank());
        occupied &= !Bitboard::from_square(ep_sq);
    }

    // Initial captured piece value
    let mut gain = [0i16; 32];
    gain[0] = target_value(board, mv);

    // Add promotion bonus
    if let Some(promo) = mv.promotion() {
        gain[0] += piece_value(promo) - piece_value(Role::Pawn);
    }

    // Initial attacking piece (promotion changes role)
    let mut attacker_role = promoted_role(board, mv);
    let mut attacker_bb = Bitboard::from_square(from);

    let mut side = !pos.turn(); // opponent moves next
    let mut depth = 0usize;
    let mut seen = Bitboard::EMPTY;

    // All attackers to target square
    let mut attackers =
        board.attacks_to(to, Color::White, occupied)
            | board.attacks_to(to, Color::Black, occupied);

    let slider_mask = occupied & !(board.knights() | board.kings());

    loop {
        depth += 1;

        // Remove last attacker
        attackers &= !attacker_bb;
        occupied &= !attacker_bb;
        seen |= attacker_bb;


        if !(attacker_bb & slider_mask).is_empty() {
            attackers |= xray_attackers(board, to, occupied) & !seen;
        }

        // No more captures by side to move
        if (attackers & board.by_color(side)).is_empty() {
            gain[depth] = 0;
            depth -= 1;
            break;
        }

        // Find least valuable attacker
        if let Some((bb, role)) = least_valuable_attacker(board, attackers, side) {
            gain[depth] =
                piece_value(attacker_role) - gain[depth - 1];

            attacker_bb = bb;
            attacker_role = effective_role_with_promotion(role, side, to, &mut gain[depth]);

            side = !side;
        } else {
            break;
        }
    }

    while depth > 0 {
        gain[depth - 1] = -std::cmp::max(-gain[depth - 1], gain[depth]);
        depth -= 1;
    }

    // If we have a quiet move then we just calculate the see from the opponent. But then we take the gain on first index and reverse sign
    if is_quiet {
        -gain[1]
    } else {
        gain[0]
    }
}

//
// ───────────────────────── Helper Functions ─────────────────────────
//

#[inline]
fn piece_value(role: Role) -> i16 {
    PIECE_VALUES[role as usize - 1]
}

/// Returns value of initially captured piece
#[inline]
fn target_value(board: &Board, mv: Move) -> i16 {
    if mv.is_en_passant() {
        piece_value(Role::Pawn)
    } else {
        board
            .role_at(mv.to())
            .map(piece_value)
            .unwrap_or(PIECE_VALUES[6])
    }
}

/// Returns role of attacking piece after initial promotion
#[inline]
fn promoted_role(board: &Board, mv: Move) -> Role {
    mv.promotion().unwrap_or_else(|| board.role_at(mv.from().unwrap()).unwrap())
}

/// Handles promotion inside recursive capture chain
/// If a pawn captures on final rank, it is treated as a queen
#[inline]
fn effective_role_with_promotion(
    role: Role,
    side: Color,
    target: Square,
    gain_entry: &mut i16,
) -> Role {
    if role == Role::Pawn && is_promotion_square(side, target) {
        // add promotion bonus
        *gain_entry += piece_value(Role::Queen) - piece_value(Role::Pawn);
        Role::Queen
    } else {
        role
    }
}

#[inline]
fn is_promotion_square(side: Color, sq: Square) -> bool {
    match side {
        Color::White => sq.rank() == shakmaty::Rank::Eighth,
        Color::Black => sq.rank() == shakmaty::Rank::First,
    }
}

/// Computes x-ray attackers after a piece is removed.
#[inline]
fn xray_attackers(board: &Board, sq: Square, occupied: Bitboard) -> Bitboard {
    let bishops = board.bishops();
    let rooks = board.rooks();
    let queens = board.queens();

    (bishop_attacks(sq, occupied) & (bishops | queens))
        | (rook_attacks(sq, occupied) & (rooks | queens))
}

/// Returns least valuable attacker of given color.
#[inline]
fn least_valuable_attacker(
    board: &Board,
    attackers: Bitboard,
    color: Color,
) -> Option<(Bitboard, Role)> {
    const ROLES: [Role; 6] = [
        Role::Pawn,
        Role::Knight,
        Role::Bishop,
        Role::Rook,
        Role::Queen,
        Role::King,
    ];

    for role in ROLES {
        let subset =
            attackers & board.by_role(role) & board.by_color(color);

        if let Some(sq) = subset.first() {
            return Some((Bitboard::from_square(sq), role));
        }
    }
    None
}
