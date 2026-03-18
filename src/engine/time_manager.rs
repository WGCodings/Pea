use std::time::{Duration, Instant};
use shakmaty::Position;
use shakmaty::Move;

pub struct TimeManager {
    pub base_time: Duration,
    pub current_limit: Duration,
    pub start_time: Instant,
    best_move_stability: u32,
    score_stability: u32,
    prev_score: i32,
    prev_best_move: Option<Move>,
}

impl TimeManager {
    pub fn new(base_time: Duration, start_time: Instant) -> Self {
        Self {
            base_time,
            current_limit: base_time,
            start_time,
            best_move_stability: 0,
            score_stability: 0,
            prev_score: 0,
            prev_best_move: None,
        }
    }

    pub fn update(&mut self, score: i32, best_move: Option<Move>) {
        // Check score drop BEFORE updating prev_score
        let score_dropped = score < self.prev_score - 30;
        let score_jumped = score > self.prev_score + 30;

        if self.prev_best_move == best_move {
            self.best_move_stability += 1;
        } else {
            self.best_move_stability = 0;
        }

        if (score - self.prev_score).abs() <= 15 {
            self.score_stability += 1;
        } else {
            self.score_stability = 0;
        }

        self.prev_score = score;
        self.prev_best_move = best_move;

        let mut scale: f32 = 1.0;

        // Only reduce time if very stable for many depths
        match self.best_move_stability {
            0 => scale *= 1.2,        // move changed, need more time
            1..=3 => scale *= 1.0,    // neutral
            4..=6 => scale *= 0.9,    // fairly stable
            _ => scale *= 0.75,       // very stable, safe to stop early
        }

        // Score instability, be conservative
        if self.score_stability == 0 {
            scale *= 1.1;
        } else if self.score_stability >= 6 {
            scale *= 0.9;
        }

        // Score dropped significantly, we're in trouble
        if score_dropped {
            scale *= 1.4;
        }

        // Score jumped, we found something good, can be more confident
        if score_jumped {
            scale *= 0.9;
        }

        scale = scale.clamp(0.6, 2.5);

        self.current_limit = Duration::from_secs_f64(self.base_time.as_secs_f64() * scale as f64).min(self.base_time * 3);
    }

    pub fn should_stop(&self) -> bool {
        self.start_time.elapsed() > self.current_limit
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}
pub fn compute_time_limit(
    pos: &impl Position,
    remaining: Option<Duration>,
    increment: Option<Duration>,
) -> Duration {
    let remaining = match remaining {
        Some(t) => t,
        None => return Duration::from_secs(1),
    };

    let increment = increment.unwrap_or(Duration::ZERO);

    // More aggressive base: use 1/30 instead of 1/50
    // This gives more time per move while still leaving plenty in reserve
    let base = remaining / 30;

    // Increment is almost free time — use 80% of it
    let inc_bonus = increment.mul_f32(0.8);

    let mut time = base + inc_bonus;

    // Complexity adjustment
    let move_count = pos.legal_moves().len() as u32;
    let complexity_factor = (move_count as f32 / 25.0).clamp(0.8, 1.2);
    time = time.mul_f32(complexity_factor);

    // Safety clamps
    let min = Duration::from_millis(20);
    // Never use more than 25% of remaining in one move
    let max = (remaining / 4).max(min);

    time.clamp(min, max)
}