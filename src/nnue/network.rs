const L1: usize = 1536;
const HALF: usize = L1 / 2;
const L2: usize = 32;
const L3: usize = 16;
const SCALE: i32 = 400;
const QA: i16 = 255;
const QB: i16 = 64;
const Q: i16 = 64;
const FT_SHIFT: u32 = 8;

const NUM_OUTPUT_BUCKETS : usize = 8;

const KING_BUCKET_LAYOUT: [usize; 64] =  [
    0, 0, 1, 1,1,1,0,0,
    2, 2, 2, 2,2,2,2,2,
    3, 3, 3, 3,3,3,3,3,
    3, 3, 3, 3,3,3,3,3,
    3, 3, 3, 3,3,3,3,3,
    3, 3, 3, 3,3,3,3,3,
    3, 3, 3, 3,3,3,3,3,
    3, 3, 3, 3,3,3,3,3
];
pub const NUM_INPUT_BUCKETS: usize = 4;

#[cfg(target_feature = "avx2")]
use std::arch::x86_64::*;

use shakmaty::{Board, Chess, Color, Position, Role};

static NNUE: Network = unsafe { std::mem::transmute(*include_bytes!("../../nnue/files/quantised.bin")) };

// =====================================================================================================================//
// NNUE NETWORK IS TRAINED BY THE BULLET CRATE AND CODE HAS BEEN REUSED FROM ONE OF THE EXAMPLES TO DO THE INFERENCE
// =====================================================================================================================//


/// Returns the bucket
pub fn get_bucket(board: &Board, perspective: Color) -> (usize,bool) {
    let king_sq = board.king_of(perspective).unwrap();
    let mut sq_idx = king_sq.to_usize();
    if perspective == Color::Black {
        sq_idx ^= 56;
    }

    let is_mirrored = sq_idx % 8 > 3;

    (KING_BUCKET_LAYOUT[sq_idx],is_mirrored)
}

#[inline(always)]
pub fn calculate_index(mut side: usize, mut sq_idx: usize, piece_type: usize, perspective: Color, bucket: usize, is_mirrored : bool) -> usize {
    if perspective == Color::Black {
        side = 1 - side;
        sq_idx ^= 56;
    }

    if is_mirrored{
        sq_idx ^= 7;
    }

    bucket * 768 + side * 384 + piece_type * 64 + sq_idx
}

#[inline(always)]
pub fn accumulator_for_perspective<P: Position>(pos: &P, net: &Network, perspective: Color) -> (Accumulator, usize, bool) {
    let mut acc = Accumulator::new(net);
    let board = pos.board();
    let (bucket, is_mirrored) = get_bucket(board, perspective);

    for square in shakmaty::Square::ALL {
        if let Some(piece) = board.piece_at(square) {
            let sq_idx = shakmaty::Square::to_usize(square);
            let piece_type = role_index(piece.role);
            let side = if piece.color == Color::White { 0 } else { 1 };
            acc.add_feature(calculate_index(side, sq_idx, piece_type, perspective, bucket, is_mirrored), net);
        }
    }
    (acc, bucket, is_mirrored)
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

#[inline]
fn crelu(x: i16, qa: i16) -> i32 {
    i32::from(x).clamp(0, i32::from(qa))
}

/// This is the quantised format that bullet outputs.
#[repr(C)]
pub struct Network {
    feature_weights: [Accumulator; 768 * NUM_INPUT_BUCKETS],
    feature_bias: Accumulator,

    l1_weights: [i8; L1 * NUM_OUTPUT_BUCKETS * L2],
    l1_bias: [i32; NUM_OUTPUT_BUCKETS * L2],

    l2_weights: [i32; L2 * NUM_OUTPUT_BUCKETS * L3],
    l2_bias: [i32; NUM_OUTPUT_BUCKETS * L3],

    l3_weights: [i32; L3 * NUM_OUTPUT_BUCKETS],
    l3_bias: [i32; NUM_OUTPUT_BUCKETS]
}

impl Network {
    #[cfg(not(target_feature = "avx2"))]
    pub fn evaluate(&self, us: &Accumulator, them: &Accumulator, pos: &Chess) -> i32 {
        let bucket = self.bucket(pos);

        let mut hl1 = [0i32; L1];
        for i in 0..HALF {
            let a = crelu(us.vals[i], QA);
            let b = crelu(us.vals[i + HALF], QA);
            hl1[i] = (a * b) >> FT_SHIFT;
        }
        for i in 0..HALF {
            let a = crelu(them.vals[i], QA);
            let b = crelu(them.vals[i + HALF], QA);
            hl1[HALF + i] = (a * b) >> FT_SHIFT;
        }

        let l1_off = bucket * L2 * L1;
        let l1_bias_off = bucket * L2;
        let mut hl2 = [0i32; L2];
        for o in 0..L2 {
            let mut sum: i32 = 0;
            for i in 0..L1 {
                sum += hl1[i] * i32::from(self.l1_weights[l1_off + o * L1 + i]);
            }
            sum += self.l1_bias[l1_bias_off + o];
            hl2[o] = (sum / i32::from(Q)).clamp(0, i32::from(Q));
        }

        let l2_off = bucket * L3 * L2;
        let l2_bias_off = bucket * L3;
        let mut hl3 = [0i32; L3];
        for o in 0..L3 {
            let mut sum: i32 = 0;
            for i in 0..L2 {
                sum += hl2[i] * self.l2_weights[l2_off + o * L2 + i];
            }
            sum += self.l2_bias[l2_bias_off + o];
            hl3[o] = (sum / i32::from(Q).pow(2)).clamp(0, i32::from(Q));
        }

        let l3_off = bucket * L3;
        let mut output: i32 = self.l3_bias[bucket];
        for i in 0..L3 {
            output += hl3[i] * self.l3_weights[l3_off + i];
        }

        output * SCALE / i32::from(Q).pow(4)
    }

    #[cfg(target_feature = "avx2")]
    pub fn evaluate(&self, us: &Accumulator, them: &Accumulator, pos: &Chess) -> i32 {
        let bucket = self.bucket(pos);
        let offset = bucket * 2 * HIDDEN_SIZE;
        let us_weights = &self.output_weights[offset..offset + HIDDEN_SIZE];
        let them_weights = &self.output_weights[offset + HIDDEN_SIZE..offset + 2 * HIDDEN_SIZE];
        unsafe {
            let zero = _mm256_setzero_si256();
            let qa = _mm256_set1_epi16(QA);

            let sum = _mm256_add_epi32(Self::screlu_avx2(&us.vals, us_weights, zero, qa), Self::screlu_avx2(&them.vals, them_weights, zero, qa));

            let mut output = Self::hsum_epi32(sum);

            output /= i32::from(QA);
            output += i32::from(self.output_bias[bucket]);
            output *= SCALE;
            output /= i32::from(QA) * i32::from(QB);
            output
        }
    }

    #[cfg(target_feature = "avx2")]
    #[inline(always)]
    unsafe fn screlu_avx2(
        inputs: &[i16; HIDDEN_SIZE],
        weights: &[i16],
        zero: __m256i,
        qa: __m256i,
    ) -> __m256i {
        let mut acc = _mm256_setzero_si256();
        let in_ptr = inputs.as_ptr();
        let w_ptr = weights.as_ptr();

        for i in (0..HIDDEN_SIZE).step_by(16) {
            let x = _mm256_load_si256(in_ptr.add(i) as *const __m256i);
            let w = _mm256_loadu_si256(w_ptr.add(i) as *const __m256i);

            let clamped = _mm256_min_epi16(_mm256_max_epi16(x, zero), qa);
            let t = _mm256_mullo_epi16(clamped, w);
            let prod = _mm256_madd_epi16(clamped, t);

            acc = _mm256_add_epi32(acc, prod);
        }
        acc
    }
    #[cfg(target_feature = "avx2")]
    #[inline(always)]
    unsafe fn hsum_epi32(v: __m256i) -> i32 {
        let hi = _mm256_extracti128_si256(v, 1);
        let lo = _mm256_castsi256_si128(v);
        let sum128 = _mm_add_epi32(hi, lo);
        let hi64 = _mm_unpackhi_epi64(sum128, sum128);
        let sum64 = _mm_add_epi32(sum128, hi64);
        let hi32 = _mm_shuffle_epi32(sum64, 0b01);
        let sum32 = _mm_add_epi32(sum64, hi32);
        _mm_cvtsi128_si32(sum32)
    }

    pub fn load() -> &'static Network {
        &NNUE
    }

    pub fn _load_from_path(path: &str) -> Box<Network> {
        let bytes = std::fs::read(path)
            .unwrap_or_else(|e| panic!("Failed to read network '{}': {}", path, e));
        assert_eq!(
            bytes.len(),
            std::mem::size_of::<Network>(),
            "Network file '{}' has wrong size", path
        );
        unsafe {
            let mut net = Box::new(std::mem::zeroed::<Network>());
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                net.as_mut() as *mut Network as *mut u8,
                std::mem::size_of::<Network>(),
            );
            net
        }
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
    pub(crate) vals: [i16; L1],
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

    /// Combine remove and add features per move into a list and do them in one go instead as one per one.
    pub fn apply_feature_updates(&mut self, adds: &[usize], removes: &[usize], net: &Network) {

        for &idx in adds {
            for (i, d) in self.vals.iter_mut().zip(&net.feature_weights[idx].vals) {
                *i += *d
            }
        }
        for &idx in removes {
            for (i, d) in self.vals.iter_mut().zip(&net.feature_weights[idx].vals) {
                *i -= *d
            }
        }
    }
}