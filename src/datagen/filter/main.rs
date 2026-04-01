// main.rs
// Standalone filter and balancer tool for NNUE training data.
//
// Usage:
//   filter --input file1.txt file2.txt --output train.txt [options]
//
// Options:
//   --input   <files...>     Input raw position files (required)
//   --output  <file>         Output bullet format file (required)
//   --target  <n>            Target number of output positions (0 = all)
//   --max-score <n>          Drop positions with |score| > n (default 3000)
//   --imbalance-threshold <n> Quiet/imbalanced boundary in cp (default 100)
//   --max-per-pawn-hash <n>  Max positions per pawn structure (default 4, 0 = off)
//   --no-shuffle             Disable shuffle
//   --seed <n>               Random seed for reproducible shuffle

mod config;
mod reader;
mod filter;

mod shuffle;
mod writer;
mod balancer;

use std::time::Instant;
use config::FilterConfig;

use reader::read_all_files;
use shuffle::shuffle;
use writer::BulletWriter;
use crate::balancer::collect_and_filter;

fn main() {
    let (input_patterns, output_file, config) = parse_args();

    // Expand glob patterns to actual file list
    let input_files = reader::expand_inputs(&input_patterns);

    if input_files.is_empty() {
        eprintln!("Error: no files found matching input patterns");
        std::process::exit(1);
    }

    println!("=== Pea Filter Tool ===");
    println!("Input files:  {:?}", input_files);
    println!("Output file:  {}", output_file);
    println!("Max score:    {}cp", config.max_score);
    println!("Imbalance:    {}cp threshold", config.imbalance_threshold);
    println!("Pawn hash:    max {} per hash", config.max_per_pawn_hash);
    println!("Net id:       {}", config.net_id.map_or("all".to_string(), |id| id.to_string()));
    println!("Target:       {}", if config.target_positions == 0 {
        "all".to_string()
    } else {
        config.target_positions.to_string()
    });
    println!("Shuffle:      {}", config.shuffle);
    println!();

    let start = Instant::now();

    // ---------------------------------------------------------------- //
    // Pass 1: read, hard-filter, pawn hash dedup                        //
    // ---------------------------------------------------------------- //
    println!("Pass 1: reading and filtering...");
    let positions_iter = read_all_files(&input_files);
    let (mut positions, stats) = collect_and_filter(positions_iter, &config);

    println!();
    stats.print();
    stats.check_targets(&config);
    println!();

    // ---------------------------------------------------------------- //
    // Truncate to target if requested                                    //
    // ---------------------------------------------------------------- //
    if config.target_positions > 0 && positions.len() > config.target_positions {
        // Shuffle first so truncation is random, not biased toward file order
        if config.shuffle {
            println!("Shuffling {} positions...", positions.len());
            shuffle(&mut positions, config.shuffle_seed);
        }
        positions.truncate(config.target_positions);
        println!("Truncated to {} positions", positions.len());
    } else if config.shuffle {
        println!("Shuffling {} positions...", positions.len());
        shuffle(&mut positions, config.shuffle_seed);
    }

    // ---------------------------------------------------------------- //
    // Write output                                                       //
    // ---------------------------------------------------------------- //
    println!("Writing to {}...", output_file);
    let mut writer = BulletWriter::open(&output_file.as_str())
        .unwrap_or_else(|e| panic!("Cannot create output file '{}': {}", output_file, e));

    writer.write_all(&positions)
        .expect("Failed to write output");
    writer.flush().expect("Failed to flush");

    let elapsed = start.elapsed();
    println!();
    println!("=== Done ===");
    println!("Written:  {} positions", writer.positions_written());
    println!("Time:     {:.1}s", elapsed.as_secs_f32());
    println!(
        "Speed:    {:.0} pos/s",
        writer.positions_written() as f64 / elapsed.as_secs_f64().max(0.001)
    );
}

// ------------------------------------------------------------------ //
// Argument parsing                                                     //
// ------------------------------------------------------------------ //

fn parse_args() -> (Vec<String>, String, FilterConfig) {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut config      = FilterConfig::default();
    let mut input_files = Vec::new();
    let mut output_file = String::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--input" => {
                i += 1;
                while i < args.len() && !args[i].starts_with("--") {
                    input_files.push(args[i].clone());
                    i += 1;
                }
                continue;
            }
            "--output" => {
                output_file = args[i + 1].clone();
                i += 1;
            }
            "--net-id" => {
                config.net_id = args[i + 1].parse().ok().map(|n: u8| n);
                i += 1;
            }
            "--target" => {
                config.target_positions = args[i + 1].parse().unwrap_or(0);
                i += 1;
            }
            "--max-score" => {
                config.max_score = args[i + 1].parse().unwrap_or(config.max_score);
                i += 1;
            }
            "--imbalance-threshold" => {
                config.imbalance_threshold = args[i + 1].parse().unwrap_or(config.imbalance_threshold);
                i += 1;
            }
            "--max-per-pawn-hash" => {
                config.max_per_pawn_hash = args[i + 1].parse().unwrap_or(config.max_per_pawn_hash);
                i += 1;
            }
            "--no-shuffle" => {
                config.shuffle = false;
            }
            "--seed" => {
                config.shuffle_seed = args[i + 1].parse().ok();
                i += 1;
            }
            "--min-quiet-fraction" => {
                config.min_balanced_fraction = args[i + 1].parse().unwrap_or(config.min_balanced_fraction);
                i += 1;
            }
            "--min-imbalance-fraction" => {
                config.min_imbalanced_fraction = args[i + 1].parse().unwrap_or(config.min_imbalanced_fraction);
                i += 1;
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
            }
        }
        i += 1;
    }

    if input_files.is_empty() {
        eprintln!("Error: --input required");
        std::process::exit(1);
    }
    if output_file.is_empty() {
        eprintln!("Error: --output required");
        std::process::exit(1);
    }

    (input_files, output_file, config)
}