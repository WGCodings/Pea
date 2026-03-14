use std::fs;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::BTreeMap;
#[derive(Clone, Serialize, Deserialize)]
pub struct Params {
    pub raz_max_depth: f32,
    pub raz_thr: f32,

    pub nmp_margin: f32,
    pub nmp_scaling: f32,
    pub nmp_improving_scaling: f32,
    pub nmp_min_depth: f32,
    pub nmp_base_reduction: f32,
    pub nmp_reduction_scaling: f32,

    pub snmp_scaling: f32,

    pub lmr_min_searches: f32,
    pub lmr_min_depth: f32,
    pub lmr_red_constant: f32,
    pub lmr_red_scaling: f32,
    pub lmr_history_divisor: f32,

    pub aspw_min_depth: f32,
    pub aspw_window_size: f32,
    pub aspw_widening_factor: f32,

    pub fp_base: f32,
    pub fp_scaling: f32,
    pub fp_max_depth: f32,
    pub fp_improving_margin: f32,
    pub fp_min_moves_searched: f32,

    pub cont_hist_scaling: f32,
    pub cont_hist_base: f32,
    pub cont_hist_malus_scaling: f32,

    pub lmp_base: f32,
    pub lmp_lin_scaling: f32,
    pub lmp_quad_scaling: f32,
    pub lmp_max_depth: f32,

    pub rfp_scaling: f32,
    pub rfp_improving_scaling: f32,
    pub rfp_max_depth: f32,

    pub hpp_max_depth: f32,
    pub hpp_tactical_scaling: f32,

    pub iir_min_depth: f32,
    pub se_dext_margin: f32,
    pub se_scaling: f32,
    pub se_depth_ok: f32,
    pub se_min_depth: f32
}

impl Params {

    pub fn load_yaml(path: &str) -> Self {
        match fs::read_to_string(path) {
            Ok(file) =>
            { serde_yaml::from_str::<Params>(&file.as_str()).unwrap_or_else(|_| { eprintln!("Failed to parse params.yaml, using defaults.");
                Params::default() }) }
            Err(_) => { eprintln!("Failed to read params.yaml, using defaults.");
                Params::default() } } }
    pub fn save_yaml(&self, path: &str) {
        let yaml = serde_yaml::to_string(self).expect("Failed to serialize params");
        fs::write(path, yaml).expect("Failed to write params.yaml");
    }


    pub fn default() -> Self {
        Self {
            // RAZORING
            raz_max_depth: 5.0,
            raz_thr: 256.0,
            // NULL MOVE PRUNING
            nmp_margin : 120.0,
            nmp_scaling : 20.0,
            nmp_improving_scaling: 0.0,
            nmp_min_depth: 3.0,
            nmp_base_reduction: 4.0,
            nmp_reduction_scaling: 4.0,
            // STATIC NULL MOVE PRUNING
            snmp_scaling: 85.0,
            // LATE MOVE REDUCTION
            lmr_min_searches: 5.0,
            lmr_min_depth: 3.0,
            lmr_red_constant: 0.7844,
            lmr_red_scaling: 2.4695,
            lmr_history_divisor: 8192.0,
            // ASPIRATION WINDOW
            aspw_min_depth: 3.0,
            aspw_window_size: 50.0,
            aspw_widening_factor: 2.0,
            //FUTILITY PRUNING
            fp_base: 40.0,
            fp_scaling : 60.0,
            fp_max_depth: 8.0,
            fp_improving_margin: 0.0,
            fp_min_moves_searched: 1.0,
            // REVERSE FUTILITY PRUNING
            rfp_scaling: 47.0,
            rfp_improving_scaling: 100.0,
            rfp_max_depth: 9.0,
            // LATE MOVE PRUNING

            lmp_base: 4.0,
            lmp_lin_scaling: 4.0,
            lmp_quad_scaling: 0.0,
            lmp_max_depth: 5.0,
            // N-PLY CONTINUATION HISTORY
            cont_hist_scaling: 150.0,
            cont_hist_base: 125.0,
            cont_hist_malus_scaling: 1.0,
            // hanging piece pruning
            hpp_max_depth: 3.0,
            hpp_tactical_scaling: 0.0,
            // internal iterative deepening
            iir_min_depth: 4.0,
            se_dext_margin: 17.0,
            se_scaling: 2.0,
            se_depth_ok: 3.0,
            se_min_depth: 8.0,
        }
    }
}
pub fn params_to_map(params: &Params) -> BTreeMap<String, Value> {
    let value = serde_yaml::to_value(params).unwrap();

    match value {
        Value::Mapping(map) => map
            .into_iter()
            .map(|(k, v)| (k.as_str().unwrap().to_string(), v))
            .collect(),
        _ => panic!("Params must serialize to map"),
    }
}

pub fn map_to_params(map: BTreeMap<String, Value>) -> Params {
    let mapping = map
        .into_iter()
        .map(|(k, v)| (Value::String(k), v))
        .collect();

    serde_yaml::from_value(Value::Mapping(mapping)).unwrap()
}
