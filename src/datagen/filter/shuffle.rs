// Fisher-Yates in-place shuffle with optional seed for reproducibility.

use rand::{RngExt, SeedableRng};
use rand::rngs::SmallRng;

pub fn shuffle<T>(data: &mut Vec<T>, seed: Option<u64>) {
    let mut rng = match seed {
        Some(s) => SmallRng::seed_from_u64(s),
        None    => rand::make_rng(),
    };

    let n = data.len();
    for i in (1..n).rev() {
        let j = rng.random_range(0..=i);
        data.swap(i, j);
    }
}
