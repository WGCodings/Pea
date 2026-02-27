
#[derive(Clone)]
pub struct Params {
    pub piece_values: [f32; 6],

    pub raz_max_depth: usize,
    pub raz_thr: f32,
    pub nmp_margin: f32,
    pub nmp_scaling: f32,
    pub nmp_min_depth: usize,
    pub nmp_base_reduction: usize,
    pub nmp_reduction_scaling: usize,
    pub snmp_scaling: f32,
    pub lmr_min_searches: i32,
    pub lmr_min_depth: usize,
    pub lmr_red_constant: f32,
    pub lmr_red_scaling: f32
}

impl Params {
    pub fn default() -> Self {
        Self {
            piece_values: [100.0, 320.0, 330.0, 500.0, 900.0, 0.0], // P, N, B, R, Q, K
            // RAZORING
            raz_max_depth: 5,
            raz_thr: 256.0,
            // NULL MOVE PRUNING
            nmp_margin : 120.0,
            nmp_scaling : 20.0,
            nmp_min_depth: 3,
            nmp_base_reduction: 4,
            nmp_reduction_scaling: 4,
            // STATIC NULL MOVE PRUNING
            snmp_scaling: 85.0,
            // LATE MOVE REDUCTION
            lmr_min_searches: 4,
            lmr_min_depth: 3,
            lmr_red_constant: 0.7844,
            lmr_red_scaling: 2.4695,
        }
    }
}
