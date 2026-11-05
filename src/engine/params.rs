use serde::{Deserialize, Serialize};


// =====================================================================================================================//
// ALL OUR SEARCH PARAMETERS, CAN BE LOADED AND SAVE TO FROM YAML                                                       //
// =====================================================================================================================//
#[derive(Clone, Serialize, Deserialize)]
pub struct Params {
    pub raz_max_depth: i32,
    pub raz_thr: i32,
    pub raz_improving_margin: i32,

    pub nmp_margin: i32,
    pub nmp_scaling: i32,
    pub nmp_improving_scaling: i32,
    pub nmp_min_depth: i32,
    pub nmp_base_reduction: i32,
    pub nmp_reduction_scaling: i32,
    pub nmp_verif_depth: i32,

    pub snmp_scaling: i32,

    pub lmr_min_searches: i32,
    pub lmr_min_depth: i32,
    pub lmr_red_constant: f32,
    pub lmr_red_scaling: f32,
    pub lmr_history_divisor: i32,
    pub lmr_see_thr: i32,
    pub lmr_corr_scaling: i32,

    pub aspw_min_depth: i32,
    pub aspw_window_size: i32,
    pub aspw_widening_factor: f32,

    pub fp_base: i32,
    pub fp_scaling: i32,
    pub fp_max_depth: i32,
    pub fp_improving_margin: i32,
    pub fp_min_moves_searched: i32,

    pub cont_hist_scaling: i32,
    pub cont_hist_base: i32,
    pub cont_hist_malus_scaling: i32,
    pub cont_hist_malus_base: i32,

    pub lmp_base: i32,
    pub lmp_lin_scaling: i32,
    pub lmp_quad_scaling: i32,
    pub lmp_max_depth: i32,

    pub rfp_scaling: i32,
    pub rfp_improving_scaling: i32,
    pub rfp_max_depth: i32,

    pub hpp_quiet_scaling: i32,
    pub hpp_tactical_scaling: i32,

    pub iir_min_depth: i32,
    pub se_dext_margin: i32,
    pub se_scaling: i32,
    pub se_depth_ok: i32,
    pub se_min_depth: i32,
    pub se_text_margin: i32,
    pub se_max_nr_dext: i32,
    pub hist_prune_margin: i32,
    pub hist_prune_depth: i32,
    pub pc_beta_margin: i32,
    pub pc_depth_divisor: i32,
    pub pc_min_depth: i32,
    pub pc_improving_margin: i32,
    pub pc_see_thr: i32,


}

impl Params {
    pub fn default() -> Self {
        Self {
            // RAZORING
            raz_max_depth: 5,
            raz_thr: 244,
            raz_improving_margin: 0,
            // NULL MOVE PRUNING
            nmp_margin : 123,
            nmp_scaling : 22,
            nmp_improving_scaling: -5,
            nmp_min_depth: 3,
            nmp_base_reduction: 4,
            nmp_reduction_scaling: 4,
            nmp_verif_depth: 12,
            // STATIC NULL MOVE PRUNING
            snmp_scaling: 85,
            // LATE MOVE REDUCTION
            lmr_min_searches: 4,
            lmr_min_depth: 2,
            lmr_red_constant: 0.7335,
            lmr_red_scaling: 2.5967,
            lmr_history_divisor: 8936,
            lmr_see_thr: 4,
            lmr_corr_scaling: 32,
            // ASPIRATION WINDOW
            aspw_min_depth: 5,
            aspw_window_size: 31,
            aspw_widening_factor: 2.1779,
            //FUTILITY PRUNING
            fp_base: 42,
            fp_scaling : 62,
            fp_max_depth: 8,
            fp_improving_margin: 2,
            fp_min_moves_searched: 1,
            // REVERSE FUTILITY PRUNING
            rfp_scaling: 41,
            rfp_improving_scaling: 106,
            rfp_max_depth: 10,
            // LATE MOVE PRUNING
            lmp_base: 4,
            lmp_lin_scaling: 3,
            lmp_quad_scaling: 0,
            lmp_max_depth: 5,
            // N-PLY CONTINUATION HISTORY
            cont_hist_scaling: 356,
            cont_hist_base: 150,
            cont_hist_malus_scaling: 399,
            cont_hist_malus_base: 146,
            // hanging piece pruning
            hpp_quiet_scaling: 21,
            hpp_tactical_scaling: 76,
            // internal iterative deepening
            iir_min_depth: 2,
            se_dext_margin: 16,
            se_scaling: 4,
            se_depth_ok: 3,
            se_min_depth: 10,
            se_text_margin: 97,
            se_max_nr_dext: 8,
            // History pruning
            hist_prune_margin: 994,
            hist_prune_depth: 4,
            // probcut
            pc_beta_margin: 267,
            pc_depth_divisor: 124,
            pc_min_depth: 9,
            pc_improving_margin: 10,
            pc_see_thr: 0,


        }
    }
}
