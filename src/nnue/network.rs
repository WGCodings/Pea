const HIDDEN_SIZE: usize = 512;
const SCALE: i32 = 400;
const QA: i16 = 255;
const QB: i16 = 64;

use shakmaty::{Color, Position, Role};


pub fn evaluate_position<P: Position>(pos: &P, net: &Network) -> i32 {
    let (us, them) = accumulators_from_position(pos, net);

    let mut eval = net.evaluate(&us, &them);

    // Convert to white perspective if needed
    if pos.turn() == Color::Black {
        eval = -eval;
    }

    eval
}


fn role_index(role: Role) -> usize {
    match role {
        Role::Pawn => 0,
        Role::Knight => 1,
        Role::Bishop => 2,
        Role::Rook => 3,
        Role::Queen => 4,
        Role::King => 5,
    }
}

fn accumulators_from_position<P: Position>(
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

fn calculate_index(mut side: usize, mut sq_idx: usize, piece_type : usize, perspective : Color) -> usize{
    if perspective == Color::Black {
        side = 1-side;
        sq_idx ^= 0b111000;
    }
    (side*6 + piece_type)*64 + sq_idx

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
    output_weights: [i16; 2 * HIDDEN_SIZE],
    /// Scalar output bias.
    /// Value has quantization of QA * QB.
    output_bias: i16,
}

impl Network {
    /// Calculates the output of the network, starting from the already
    /// calculated hidden layer (done efficiently during makemoves).
    pub fn evaluate(&self, us: &Accumulator, them: &Accumulator) -> i32 {
        // Initialise output.
        let mut output = 0;

        // Side-To-Move Accumulator -> Output.
        for (&input, &weight) in us.vals.iter().zip(&self.output_weights[..HIDDEN_SIZE]) {
            output += screlu(input) * i32::from(weight);
        }

        // Not-Side-To-Move Accumulator -> Output.
        for (&input, &weight) in them.vals.iter().zip(&self.output_weights[HIDDEN_SIZE..]) {
            output += screlu(input) * i32::from(weight);
        }

        // Reduce quantization from QA * QA * QB to QA * QB.
        output /= i32::from(QA);

        // Add bias.
        output += i32::from(self.output_bias);

        // Apply eval scale.
        output *= SCALE;

        // Remove quantisation altogether.
        output /= i32::from(QA) * i32::from(QB);

        output
    }
}

/// A column of the feature-weights matrix.
/// Note the `align(64)`.
#[derive(Clone, Copy)]
#[repr(C, align(64))]
pub struct Accumulator {
    vals: [i16; HIDDEN_SIZE],
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