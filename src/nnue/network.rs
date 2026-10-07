pub const HIDDEN_SIZE: usize = 1536;
pub const L2_SIZE: usize = 32;
pub const L3_SIZE: usize = 32;
pub const NUM_OUTPUT_BUCKETS: usize = 8;
pub const NUM_INPUT_BUCKETS: usize = 4;
const SCALE: f32 = 400.0;
const QA: i16 = 255;
const Q1: i32 = 64;
pub const FT_SHIFT: u32 = 9;
const L1_DEQUANT: f32 = 1.0 / ((QA as f32 * QA as f32 / (1u32 << FT_SHIFT) as f32) * Q1 as f32);
const HALF: usize = HIDDEN_SIZE / 2;
const L1_CHUNKS: usize = HIDDEN_SIZE / 4;

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

/// This is the quantised format that bullet outputs.
#[repr(C)]
pub struct Network {
    pub(crate) feature_weights: [Accumulator; 768 * NUM_INPUT_BUCKETS],
    pub(crate) feature_bias: Accumulator,
    l1_weights: [[i8; HIDDEN_SIZE * L2_SIZE]; NUM_OUTPUT_BUCKETS],
    l1_bias: [[f32; L2_SIZE]; NUM_OUTPUT_BUCKETS],
    l2_weights: [[[f32; L3_SIZE]; L2_SIZE]; NUM_OUTPUT_BUCKETS],
    l2_bias: [[f32; L3_SIZE]; NUM_OUTPUT_BUCKETS],
    l3_weights: [[f32; L3_SIZE]; NUM_OUTPUT_BUCKETS],
    l3_bias: [f32; NUM_OUTPUT_BUCKETS],
}

#[repr(C, align(64))]
struct FtOut([u8; HIDDEN_SIZE]);

// Shared implementations for avx2 and non avx2
impl Network {
    pub fn load() -> &'static Network {
        &NNUE
    }

    fn bucket(pos: &Chess) -> usize {
        let divisor = 32usize.div_ceil(NUM_OUTPUT_BUCKETS);
        (pos.board().occupied().count() - 2) / divisor
    }
    pub fn evaluate(&self, us: &Accumulator, them: &Accumulator, pos: &Chess) -> i32 {
        self.evaluate_bucket(us, them, Self::bucket(pos)) as i32
    }

    pub fn evaluate_bucket(&self, us: &Accumulator, them: &Accumulator, bucket: usize) -> f32 {

        let mut ft = FtOut([0; HIDDEN_SIZE]);
        activate_ft(&us.vals, &mut ft.0[..HALF]);
        activate_ft(&them.vals, &mut ft.0[HALF..]);

        let sums = l1_forward(&ft.0, &self.l1_weights[bucket]);
        let mut l1 = [0f32; L2_SIZE];
        for i in 0..L2_SIZE {
            l1[i] = (sums[i] as f32 * L1_DEQUANT + self.l1_bias[bucket][i]).max(0.0).min(1.0);
        }

        let mut l2 = self.l2_bias[bucket];
        for (i, &x) in l1.iter().enumerate() {
            let w = &self.l2_weights[bucket][i];
            for o in 0..L3_SIZE {
                l2[o] += x * w[o];
            }
        }
        for v in l2.iter_mut() {
            *v = v.max(0.0).min(1.0);
        }

        let w = &self.l3_weights[bucket];
        let mut partial = [0f32; 8];
        for (a, w) in l2.chunks_exact(8).zip(w.chunks_exact(8)) {
            for k in 0..8 {
                partial[k] += w[k] * a[k];
            }
        }
        let out = self.l3_bias[bucket] + partial.iter().sum::<f32>();

        out * SCALE
    }
}

#[cfg(target_feature = "avx2")]
fn activate_ft(acc: &[i16; HIDDEN_SIZE], out: &mut [u8]) {
    use std::arch::x86_64::*;
    unsafe {
        let zero = _mm256_setzero_si256();
        let qa = _mm256_set1_epi16(QA);
        let a_ptr = acc.as_ptr();
        let b_ptr = acc.as_ptr().add(HALF);
        let o_ptr = out.as_mut_ptr();

        for i in (0..HALF).step_by(32) {
            let a0 = _mm256_min_epi16(_mm256_max_epi16(_mm256_load_si256(a_ptr.add(i).cast()), zero), qa);
            let a1 = _mm256_min_epi16(_mm256_max_epi16(_mm256_load_si256(a_ptr.add(i + 16).cast()), zero), qa);
            let b0 = _mm256_min_epi16(_mm256_load_si256(b_ptr.add(i).cast()), qa);
            let b1 = _mm256_min_epi16(_mm256_load_si256(b_ptr.add(i + 16).cast()), qa);

            let p0 = _mm256_mulhi_epi16(_mm256_slli_epi16::<7>(a0), b0);
            let p1 = _mm256_mulhi_epi16(_mm256_slli_epi16::<7>(a1), b1);

            let packed = _mm256_permute4x64_epi64::<0b11_01_10_00>(_mm256_packus_epi16(p0, p1));
            _mm256_storeu_si256(o_ptr.add(i).cast(), packed);
        }
    }
}

#[cfg(not(target_feature = "avx2"))]
fn activate_ft(acc: &[i16; HIDDEN_SIZE], out: &mut [u8]) {
    for i in 0..HALF {
        let a = i32::from(acc[i]).clamp(0, i32::from(QA));
        let b = i32::from(acc[i + HALF]).clamp(0, i32::from(QA));
        out[i] = ((a * b) >> FT_SHIFT) as u8;
    }
}

#[cfg(target_feature = "avx2")]
fn l1_forward(input: &[u8; HIDDEN_SIZE], weights: &[i8; HIDDEN_SIZE * L2_SIZE]) -> [i32; L2_SIZE] {
    use std::arch::x86_64::*;
    const _: () = assert!(L2_SIZE % 8 == 0, "L2_SIZE needs to be a multiple of 8.");

    const REGS: usize = L2_SIZE / 8;
    const CHUNK_BYTES: usize = L2_SIZE * 4;

    unsafe {
        let mut nnz = [0u16; L1_CHUNKS + 8];
        let mut count = 0;
        let in_ptr = input.as_ptr();
        let zero = _mm256_setzero_si256();
        let mut base = _mm_setzero_si128();
        let step = _mm_set1_epi16(8);

        for c in (0..L1_CHUNKS).step_by(8) {
            let v = _mm256_load_si256(in_ptr.add(c * 4).cast());
            let is_zero = _mm256_cmpeq_epi32(v, zero);
            let mask = (!_mm256_movemask_ps(_mm256_castsi256_ps(is_zero)) & 0xFF) as usize;

            let offsets = _mm_load_si128(NNZ_TABLE.0[mask].as_ptr().cast());
            _mm_storeu_si128(nnz.as_mut_ptr().add(count).cast(), _mm_add_epi16(base, offsets));
            count += mask.count_ones() as usize;
            base = _mm_add_epi16(base, step);
        }

        let ones = _mm256_set1_epi16(1);
        let in32 = in_ptr.cast::<i32>();
        let w_ptr = weights.as_ptr();

        let mut acc_a = [_mm256_setzero_si256(); REGS];
        let mut acc_b = [_mm256_setzero_si256(); REGS];

        let mut j = 0;
        while j + 1 < count {
            let ca = nnz[j] as usize;
            let cb = nnz[j + 1] as usize;
            let xa = _mm256_set1_epi32(*in32.add(ca));
            let xb = _mm256_set1_epi32(*in32.add(cb));
            for k in 0..REGS {
                let wa = _mm256_load_si256(w_ptr.add(ca * CHUNK_BYTES + k * 32).cast());
                let wb = _mm256_load_si256(w_ptr.add(cb * CHUNK_BYTES + k * 32).cast());
                acc_a[k] = _mm256_add_epi32(acc_a[k], _mm256_madd_epi16(_mm256_maddubs_epi16(xa, wa), ones));
                acc_b[k] = _mm256_add_epi32(acc_b[k], _mm256_madd_epi16(_mm256_maddubs_epi16(xb, wb), ones));
            }
            j += 2;
        }
        if j < count {
            let c = nnz[j] as usize;
            let x = _mm256_set1_epi32(*in32.add(c));
            for k in 0..REGS {
                let w = _mm256_load_si256(w_ptr.add(c * CHUNK_BYTES + k * 32).cast());
                acc_a[k] = _mm256_add_epi32(acc_a[k], _mm256_madd_epi16(_mm256_maddubs_epi16(x, w), ones));
            }
        }

        let mut out = [0i32; L2_SIZE];
        for k in 0..REGS {
            _mm256_storeu_si256(out.as_mut_ptr().add(k * 8).cast(), _mm256_add_epi32(acc_a[k], acc_b[k]));
        }
        out
    }
}

#[cfg(not(target_feature = "avx2"))]
fn l1_forward(input: &[u8; HIDDEN_SIZE], weights: &[i8; HIDDEN_SIZE * L2_SIZE]) -> [i32; L2_SIZE] {
    let mut out = [0i32; L2_SIZE];
    for c in 0..L1_CHUNKS {
        let x = &input[c * 4..c * 4 + 4];
        if x == [0, 0, 0, 0] {
            continue;
        }
        for (o, sum) in out.iter_mut().enumerate() {
            let w = &weights[(c * L2_SIZE + o) * 4..(c * L2_SIZE + o) * 4 + 4];
            for k in 0..4 {
                *sum += i32::from(x[k]) * i32::from(w[k]);
            }
        }
    }
    out
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

// L1: u8 inputs x i8 weights, skipping 4-byte chunks that are all zero
// Ty to Viridithas for all the simd
#[cfg(target_feature = "avx2")]
#[repr(C, align(16))]
struct NnzTable([[u16; 8]; 256]);
#[cfg(target_feature = "avx2")]
static NNZ_TABLE: NnzTable = {
    let mut table = [[0u16; 8]; 256];
    let mut mask = 0;
    while mask < 256 {
        let mut bits = mask;
        let mut n = 0;
        while bits != 0 {
            table[mask][n] = bits.trailing_zeros() as u16;
            bits &= bits - 1;
            n += 1;
        }
        mask += 1;
    }
    NnzTable(table)
};