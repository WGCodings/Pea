use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct UciState {
    pub multipv: usize,
    pub ponder_enabled: bool,

    pub is_pondering: Arc<AtomicBool>,
    pub stop: Arc<AtomicBool>,

    pub last_wtime: Option<u64>,
    pub last_btime: Option<u64>,
    pub last_winc: Option<u64>,
    pub last_binc: Option<u64>,
}

impl UciState {
    pub fn new() -> Self {
        Self {
            multipv: 1,
            ponder_enabled: false,
            is_pondering: Arc::new(AtomicBool::new(false)),
            stop: Arc::new(AtomicBool::new(false)),
            last_wtime: None,
            last_btime: None,
            last_winc: None,
            last_binc: None,
        }
    }

    pub fn save_time(&mut self, wtime: Option<u64>, btime: Option<u64>, winc: Option<u64>, binc: Option<u64>) {
        self.last_wtime = wtime;
        self.last_btime = btime;
        self.last_winc  = winc;
        self.last_binc  = binc;
    }

    pub fn stop(&self) {
        (*self.stop).store(true, Ordering::Relaxed);
    }

    pub fn reset_stop(&self) {
        (*self.stop).store(false, Ordering::Relaxed);
    }

    pub fn set_pondering(&self, val: bool) {
        (*self.is_pondering).store(val, Ordering::Relaxed);
    }
}