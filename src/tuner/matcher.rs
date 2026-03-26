use std::process::Command;
use std::str;

#[derive(Debug)]
pub struct MatchResult {
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
}

pub fn run_match(
    engine1_yaml: &str,
    engine2_yaml: &str,
    engine1_name: &str,
    engine2_name: &str,
    games: u32,
) -> MatchResult {
    let engine1 = "C:/Users/warre/RustroverProjects/FastPeaPea/target/release/theta_minus/FastPeaPea.exe";
    let engine2 = "C:/Users/warre/RustroverProjects/FastPeaPea/target/release/theta_plus/FastPeaPea.exe";
    Command::new(engine1).args(["loadparams", engine1_yaml]);
    Command::new(engine2).args(["loadparams", engine2_yaml]);

    let output = Command::new("C:/Users/warre/Downloads/fastchess-windows-arm64/fastchess-windows-arm64/fastchess.exe")
        .args([
            "-engine", &format!("cmd={}", engine1).as_str(),&format!("name={}",engine1_name).as_str(),
            "-engine", &format!("cmd={}", engine2).as_str(),&format!("name={}",engine2_name).as_str(),
            "-each", "tc=4+0.075",
            "-rounds", &games.to_string().as_str(),
            "-repeat",
            "-concurrency", "10",
            "-recover",
            "-openings",
            "file=C:/Users/warre/Downloads/fastchess-windows-arm64/fastchess-windows-arm64/book.epd", "format=epd"
        ])
        .output()
        .expect("Failed to start fastchess");

    let stdout = str::from_utf8(&output.stdout.as_slice()).unwrap();

    parse_wdl(stdout)
}

fn parse_wdl(output: &str) -> MatchResult {

    let mut wins = 0;
    let mut losses = 0;
    let mut draws = 0;

    for line in output.lines() {
        println!("{}", line);
        if line.contains("Games")
            || line.contains("Elo")
            || line.contains("Results")
            || line.contains("LOS")
            || line.contains("Ptnml(0-2)") {

        }
        if line.contains("Games") {
            let parts: Vec<&str> = line.split(',').collect();

            for part in parts {
                let part = part.trim();

                if part.starts_with("Wins:") {
                    wins = part["Wins:".len()..].trim().parse().unwrap_or(0);
                } else if part.starts_with("Losses:") {
                    losses = part["Losses:".len()..].trim().parse().unwrap_or(0);
                } else if part.starts_with("Draws:") {
                    draws = part["Draws:".len()..].trim().parse().unwrap_or(0);
                }
            }
        }
    }

    MatchResult { wins, losses, draws }
}
