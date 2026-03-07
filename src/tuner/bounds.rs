
use std::fs;
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct ParamBound {
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub include: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Bounds {
    pub params: BTreeMap<String, ParamBound>,
}




impl Bounds {
    pub fn load_yaml(path: &str) -> Self {
        let file = fs::File::open(path).unwrap();
        serde_yaml::from_reader(file).unwrap()
    }
}
