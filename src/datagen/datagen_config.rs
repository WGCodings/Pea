// All configuration for the data generation pipeline.

#[derive(Clone)]
pub struct DatagenConfig {
    /// Number of nodes to search per move
    pub nodes_per_move: u64,

    /// Number of threads to use
    pub num_threads: usize,

    /// Number of random moves to play at the start of each game (opening diversity)
    pub random_opening_plies: usize,

    /// Total number of positions to generate across all threads
    pub target_positions: u64,

    /// Score threshold in cp for win adjudication (white-relative)
    pub adjudication_score: i32,

    /// Number of consecutive plies score must exceed threshold
    pub adjudication_plies: usize,

    /// Score threshold in cp for draw adjudication
    pub draw_adjudication_score: i32,

    /// Path to network 0
    pub net_0_path: String,

    /// Path to network 1
    pub net_1_path: String,

    /// Output directory for generated data files
    pub output_dir: String,
}

impl DatagenConfig {
    pub(crate) fn default() -> Self {
        Self {
            nodes_per_move:        5_000,
            num_threads:           15,
            random_opening_plies:  6,
            target_positions:      100_000_000,
            adjudication_score:    2000,
            adjudication_plies:    8,
            draw_adjudication_score: 10,
            net_0_path:            "../../nnue/run1_net_0/run1_net_0-10/quantised.bin".to_string(),
            net_1_path:            "../../nnue/run1_net_1/run1_net_1-10/quantised.bin".to_string(),
            output_dir:            "C:/Users/warre/RustroverProjects/FastPeaPea/nnue/data/run2".to_string(),
        }
    }
}