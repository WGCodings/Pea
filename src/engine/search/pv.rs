use shakmaty::Move;

#[derive(Clone)]
pub struct PvTable {
    pub table: Vec<Vec<Move>>, // [ply][line]
}

impl PvTable {
    pub fn new(max_depth: usize) -> Self {
        Self {
            table: vec![Vec::new(); max_depth + 1],
        }
    }

    pub fn clear_from(&mut self, ply: usize) {
        self.table[ply].clear();
    }

    pub fn set_pv(&mut self, ply: usize, mv: Move, child_pv: &[Move]) {
        let line = &mut self.table[ply];
        line.clear();
        line.push(mv);
        line.extend_from_slice(child_pv);
    }

    pub fn best_move(&self) -> Option<Move> {
        self.table[0].first().cloned()
    }

    pub fn pv_line(&self) -> &[Move] {
        &self.table[0]
    }
}

pub struct MultiPv {
    pub lines: Vec<(i32, Vec<Move>)>,      // working iteration
    last_completed: Vec<(i32, Vec<Move>)>, // stable storage
    capacity: usize,
}

impl MultiPv {
    pub fn new(capacity: usize) -> Self {
        Self {
            lines: Vec::new(),
            last_completed: Vec::new(),
            capacity,
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn insert(&mut self, score: i32, line: Vec<shakmaty::Move>) {
        self.lines.push((score, line));

        self.lines.sort_by(|a, b| {
            b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal)
        });

        self.lines.truncate(self.capacity);
    }
    pub fn save_completed_iteration(&mut self) {
        self.last_completed = self.lines.clone();
    }
    pub fn get_last_completed(&self) -> &Vec<(i32, Vec<Move>)> {
        &self.last_completed
    }

}
