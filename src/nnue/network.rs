const HIDDEN_SIZE: usize = 64;
const SCALE: i32 = 400;
const QA: i16 = 255;
const QB: i16 = 64;

const NUM_OUTPUT_BUCKETS : usize = 1;


use shakmaty::{Chess, Color, Position, Role};


// =====================================================================================================================//
// NNUE NETWORK IS TRAINED BY THE BULLET CRATE AND CODE HAS BEEN REUSED FROM ONE OF THE EXAMPLES TO DO THE INFERENCE
// =====================================================================================================================//

#[inline(always)]
pub fn accumulators_from_position<P: Position>(
    pos: &P,
    net: &Network,
) -> (Accumulator, Accumulator) {
    let mut us = Accumulator::new(net);
    let mut them = Accumulator::new(net);

    let perspective = pos.turn();

    for square in shakmaty::Square::ALL {

        if let Some(piece) = pos.board().piece_at(square) {

            let sq_idx = shakmaty::Square::to_usize(square);

            let piece_type : usize = role_index(piece.role); // 0 for pawn, 1 for knight etc

            let side : usize = if piece.color == Color::White {0} else {1};

            let feature_index_stm = calculate_index(side,sq_idx,piece_type,perspective);
            let feature_index_nstm = calculate_index(side,sq_idx,piece_type,!perspective);


            us.add_feature(feature_index_stm, net);

            them.add_feature(feature_index_nstm, net);

        }
    }

    (us, them)
}
#[inline(always)]
pub fn calculate_index(mut side: usize, mut sq_idx: usize, piece_type : usize, perspective : Color) -> usize{
    if perspective == Color::Black {
        side = 1-side;
        sq_idx ^= 0b111000;
    }
    (side*6 + piece_type)*64 + sq_idx

}

#[inline(always)]
pub fn role_index(role: Role) -> usize {
    match role {
        Role::Pawn => 0,
        Role::Knight => 1,
        Role::Bishop => 2,
        Role::Rook => 3,
        Role::Queen => 4,
        Role::King => 5,
    }
}

#[inline]
/// Square Clipped ReLU - Activation Function.
/// Note that this takes the i16s in the accumulator to i32s.
/// Range is 0.0 .. 1.0 (in other words, 0 to QA*QA quantized).
pub fn screlu(x: i16) -> i32 {
    let y = i32::from(x).clamp(0, i32::from(QA));
    y * y
}

/// This is the quantised format that bullet outputs.
#[repr(C)]
pub struct Network {
    /// Column-Major `HIDDEN_SIZE x 768` matrix.
    /// Values have quantization of QA.
    feature_weights: [Accumulator; 768],
    /// Vector with dimension `HIDDEN_SIZE`.
    /// Values have quantization of QA.
    feature_bias: Accumulator,
    /// Column-Major `1 x (2 * HIDDEN_SIZE)`
    /// matrix, we use it like this to make the
    /// code nicer in `Network::evaluate`.
    /// Values have quantization of QB.
    output_weights: [i16; 2 * HIDDEN_SIZE*NUM_OUTPUT_BUCKETS],
    /// Scalar output bias.
    /// Value has quantization of QA * QB.
    output_bias: [i16;NUM_OUTPUT_BUCKETS]
}

impl Network {
    /// Calculates the output of the network, starting from the already
    /// calculated hidden layer (done efficiently during makemoves).
    pub fn evaluate(&self, us: &Accumulator, them: &Accumulator, pos : &Chess) -> i32 {
        let mut output = 0;
        let bucket = self.bucket(pos);
        let offset = bucket * 2 * HIDDEN_SIZE;

        let us_weights = &self.output_weights[offset .. offset + HIDDEN_SIZE];
        let them_weights = &self.output_weights[offset + HIDDEN_SIZE .. offset + 2 * HIDDEN_SIZE];

        // Side-To-Move
        for (&input, &weight) in us.vals.iter().zip(us_weights) {
            output += screlu(input) * i32::from(weight);
        }

        // Not-Side-To-Move
        for (&input, &weight) in them.vals.iter().zip(them_weights) {
            output += screlu(input) * i32::from(weight);
        }

        output /= i32::from(QA);

        output += i32::from(self.output_bias[bucket]);

        output *= SCALE;

        output /= i32::from(QA) * i32::from(QB);

        output
    }


    fn bucket(&self, pos: &Chess) -> usize {
        let divisor = 32usize.div_ceil(NUM_OUTPUT_BUCKETS);
        (pos.board().occupied().count() - 2) / divisor
    }

    /*
    fn queen_bucket(&self, pos: &Chess) -> usize {
        // Non-pawn material count
        let board = pos.board();
        let pawn_count = board.pawns().count();
        let npm_count = board.occupied().count() - pawn_count;

        // N is NUM_OUTPUT_BUCKETS / 3
        const N: usize = NUM_OUTPUT_BUCKETS / 3;
        let divisor = 16usize.div_ceil(N);
        let material_bucket = ((npm_count - 2) / divisor).min(N - 1);

        // Queen bucket
        let queen_count = board.queens().count();
        let queen_bucket = queen_count.min(2);

        material_bucket * 3 + queen_bucket
    }


     */

}

/// A column of the feature-weights matrix.
/// Note the `align(64)`.
#[derive(Clone, Copy)]
#[repr(C, align(64))]
pub struct Accumulator {
    pub(crate) vals: [i16; HIDDEN_SIZE],
}

impl Accumulator {
    /// Initialised with bias so we can just efficiently
    /// operate on it afterwards.
    pub fn new(net: &Network) -> Self {
        net.feature_bias
    }

    /// Add a feature to an accumulator.
    pub fn add_feature(&mut self, feature_idx: usize, net: &Network) {
        for (i, d) in self.vals.iter_mut().zip(&net.feature_weights[feature_idx].vals) {
            *i += *d
        }
    }

    /// Remove a feature from an accumulator.
    pub fn remove_feature(&mut self, feature_idx: usize, net: &Network) {
        for (i, d) in self.vals.iter_mut().zip(&net.feature_weights[feature_idx].vals) {
            *i -= *d
        }
    }
}