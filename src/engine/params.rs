use std::fs;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::BTreeMap;

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

    pub fn load_yaml(path: &str) -> Self {
        match fs::read_to_string(path) {
            Ok(file) =>
            { serde_yaml::from_str::<Params>(&file.as_str()).unwrap_or_else(|_| {Params::default() }) }
            Err(_) => { Params::default() } } }
    pub fn save_yaml(&self, path: &str) {
        let yaml = serde_yaml::to_string(self).expect("Failed to serialize params");
        fs::write(path, yaml).expect("Failed to write params_patch.yaml");
    }


    pub fn default() -> Self {
        Self {
            // RAZORING
            raz_max_depth: 8,
            raz_thr: 221,
            raz_improving_margin: 42,
            // NULL MOVE PRUNING
            nmp_margin : 159,
            nmp_scaling : 22,
            nmp_improving_scaling: -57,
            nmp_min_depth: 5,
            nmp_base_reduction: 7,
            nmp_reduction_scaling: 6,
            nmp_verif_depth: 13,
            // STATIC NULL MOVE PRUNING
            snmp_scaling: 52,
            // LATE MOVE REDUCTION
            lmr_min_searches: 2,
            lmr_min_depth: 0,
            lmr_red_constant: 0.8755,
            lmr_red_scaling: 2.2928,
            lmr_history_divisor: 3607,
            lmr_see_thr: -13,
            // ASPIRATION WINDOW
            aspw_min_depth: 4,
            aspw_window_size: 19,
            aspw_widening_factor: 2.2465,
            //FUTILITY PRUNING
            fp_base: 55,
            fp_scaling : 67,
            fp_max_depth: 9,
            fp_improving_margin: 0,
            fp_min_moves_searched: 5,
            // REVERSE FUTILITY PRUNING
            rfp_scaling: 69,
            rfp_improving_scaling: 74,
            rfp_max_depth: 11,
            // LATE MOVE PRUNING
            lmp_base: 9,
            lmp_lin_scaling: 0,
            lmp_quad_scaling: 1,
            lmp_max_depth: 5,
            // N-PLY CONTINUATION HISTORY
            cont_hist_scaling: 500,
            cont_hist_base: 145,
            cont_hist_malus_scaling: 3,
            // hanging piece pruning
            hpp_quiet_scaling: 15,
            hpp_tactical_scaling: 45,
            // internal iterative deepening
            iir_min_depth: 4,
            se_dext_margin: 27,
            se_scaling: 2,
            se_depth_ok: 2,
            se_min_depth: 10,
            se_text_margin: 128,
            se_max_nr_dext: 10,
            // History pruning
            hist_prune_margin: 663,
            hist_prune_depth: 2,
            // probcut
            pc_beta_margin: 269,
            pc_depth_divisor: 10,
            pc_min_depth: 5,
            pc_improving_margin: 48,
            pc_see_thr: 115,
        }
    }
}
pub(crate) fn params_to_map(params: &Params) -> BTreeMap<String, Value> {
    let value = serde_yaml::to_value(params).unwrap();

    match value {
        Value::Mapping(map) => map
            .into_iter()
            .map(|(k, v)| (k.as_str().unwrap().to_string(), v))
            .collect(),
        _ => panic!("Params must serialize to map"),
    }
}

pub(crate) fn map_to_params(map: BTreeMap<String, Value>) -> Params {
    let mapping = map
        .into_iter()
        .map(|(k, v)| (Value::String(k), v))
        .collect();

    serde_yaml::from_value(Value::Mapping(mapping)).unwrap()
}
