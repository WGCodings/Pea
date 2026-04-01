// network_init.rs
use rand::{RngExt};
use std::io::Write;

const QA: i16 = 255;
const QB: i16 = 64;

pub fn _generate_random_network(
    path: &str,
    hidden_size: usize,
    output_buckets: usize,
) {
    let mut rng = rand::rng();
    let mut file = std::fs::File::create(path).unwrap();

    let input_weights  = hidden_size * 768;
    let input_biases   = hidden_size;
    let out_weights = hidden_size * 2 * output_buckets;
    let out_biases  = output_buckets;

    let total = input_weights + input_biases + out_weights + out_biases;
    let mut weights: Vec<i16> = Vec::with_capacity(total);

    let max_input  = 1.98 * QA as f32 / 768.0_f32.sqrt();
    let max_output = 1.98 * QB as f32 / (hidden_size as f32 * 2.0).sqrt();
    let max_bias_out = 1.98 * QA as f32 * QB as f32 / (hidden_size as f32 * 2.0).sqrt();

    // Input weights
    for _ in 0..input_weights {
        weights.push(rng.random_range(-max_input..max_input) as i16);
    }
    // Input biases
    for _ in 0..input_biases {
        weights.push(rng.random_range(-max_input..max_input) as i16);
    }
    // Output weights
    for _ in 0..out_weights {
        weights.push(rng.random_range(-max_output..max_output) as i16);
    }
    // Output biases
    for _ in 0..out_biases {
        weights.push(rng.random_range(-max_bias_out..max_bias_out) as i16);
    }

    // Write raw bytes
    let mut bytes: Vec<u8> = weights.iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();

    // Pad to match Network struct size including alignment padding

    let network_size = 98752;
    println!("{}", network_size);
    println!("{}", bytes.len());
    while bytes.len() < network_size {
        bytes.push(0);
    }

    file.write_all(&bytes).unwrap();
    println!("Written {} i16 values to {}", total, path);
}