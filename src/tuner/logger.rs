use serde_yaml::Value;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;

pub fn log_yaml_to_csv(iter: usize, yaml_path: &str, csv_path: &str, elo: f64) {

    let mut yaml_string = String::new();
    File::open(yaml_path)
        .unwrap()
        .read_to_string(&mut yaml_string)
        .unwrap();

    let yaml: Value = serde_yaml::from_str(&yaml_string.as_str()).unwrap();
    let map = yaml.as_mapping().unwrap();

    let file_exists = Path::new(csv_path).exists();

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(csv_path)
        .unwrap();

    if !file_exists {

        let mut header = String::from("iteration,elo");

        for (key, _) in map {
            header.push(',');
            header.push_str(key.as_str().unwrap());
        }

        header.push('\n');
        file.write_all(header.as_bytes()).unwrap();
    }

    let mut row = format!("{},{}", iter, elo);

    for (_, value) in map {

        row.push(',');

        match value {
            Value::Number(n) => row.push_str(&n.to_string()),
            Value::String(s) => row.push_str(s),
            _ => row.push_str("0"),
        }
    }

    row.push('\n');

    file.write_all(row.as_bytes()).unwrap();
}

pub fn _csv_to_yaml(
    csv_path: &str,
    iteration: usize,
    yaml_out: &str,
) -> Result<(), Box<dyn std::error::Error>> {

    let file = File::open(csv_path)?;
    let reader = BufReader::new(file);

    let mut lines = reader.lines();

    // read header
    let header_line = lines.next().ok_or("Empty CSV")??;
    let headers: Vec<String> = header_line.split(',').map(|s| s.to_string()).collect();

    // find iteration row
    for line in lines {

        let line = line?;
        let values: Vec<&str> = line.split(',').collect();

        let iter_value: usize = values[0].parse()?;

        if iter_value == iteration {

            let mut yaml = File::create(yaml_out)?;

            for (i, header) in headers.iter().enumerate() {

                if header == "iteration" || header == "elo" {
                    continue;
                }

                let value = values.get(i).unwrap_or(&"0");

                writeln!(yaml, "{}: {}", header, value)?;
            }

            println!("YAML written to {}", yaml_out);
            return Ok(());
        }
    }

    Err(format!("Iteration {} not found in CSV", iteration).into())
}

pub fn elo_from_wdl(w: u32, l: u32, d: u32) -> f64 {

    let total = (w + l + d) as f64;

    if total == 0.0 {
        return 0.0;
    }

    let mut score = (w as f64 + 0.5 * d as f64) / total;

    // avoid infinities
    score = score.clamp(0.001, 0.999);

    -400.0 * ((1.0 / score - 1.0).log10())
}