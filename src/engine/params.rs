
#[derive(Clone)]
pub struct Params {
    pub raz_max_depth: usize,
    pub raz_thr: i32,

    pub nmp_margin: i32,
    pub nmp_scaling: i32,
    pub nmp_improving_scaling: i32,
    pub nmp_min_depth: usize,
    pub nmp_base_reduction: usize,
    pub nmp_reduction_scaling: usize,

    pub snmp_scaling: i32,

    pub lmr_min_searches: i32,
    pub lmr_min_depth: usize,
    pub lmr_red_constant: f32,
    pub lmr_red_scaling: f32,

    pub aspw_min_depth: usize,
    pub aspw_window_size: i32,
    pub aspw_widening_factor: i32,

    pub fp_base: i32,
    pub fp_scaling: i32,
    pub fp_max_depth: usize,
    pub fp_improving_margin: i32,

    pub cont_hist_scaling: i32,
    pub cont_hist_base: i32,
    pub cont_hist_malus_scaling: i32,

    pub lmp_base: i32,
    pub lmp_lin_scaling: i32,
    pub lmp_quad_scaling: i32,
    pub lmp_max_depth: usize,

    pub rfp_scaling: i32,
    pub rfp_improving_scaling: i32,
    pub rfp_max_depth: usize,
    
}

impl Params {
    pub fn default() -> Self {
        Self {
            // RAZORING
            raz_max_depth: 5,
            raz_thr: 256,
            // NULL MOVE PRUNING
            nmp_margin : 120,
            nmp_scaling : 20,
            nmp_improving_scaling: 0,
            nmp_min_depth: 3,
            nmp_base_reduction: 4,
            nmp_reduction_scaling: 4,
            // STATIC NULL MOVE PRUNING
            snmp_scaling: 85,
            // LATE MOVE REDUCTION
            lmr_min_searches: 5,
            lmr_min_depth: 3,
            lmr_red_constant: 0.7844,
            lmr_red_scaling: 2.4695,
            // ASPIRATION WINDOW
            aspw_min_depth: 3,
            aspw_window_size: 50,
            aspw_widening_factor: 2,
            //FUTILITY PRUNING
            fp_base: 40,
            fp_scaling : 60,
            fp_max_depth: 8,
            fp_improving_margin: 0,
            // REVERSE FUTILITY PRUNING
            rfp_scaling: 47,
            rfp_improving_scaling: 100,
            rfp_max_depth: 9,
            // LATE MOVE PRUNING
            lmp_base: 4,
            lmp_lin_scaling: 1,
            lmp_quad_scaling: 3,
            lmp_max_depth: 5,
            // N-PLY CONTINUATION HISTORY
            cont_hist_scaling: 150,
            cont_hist_base: 125,
            cont_hist_malus_scaling: 1,
        }
    }
}
