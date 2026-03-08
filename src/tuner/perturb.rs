use std::collections::BTreeMap;
use rand::{RngExt};

use serde_yaml::Value;
use crate::engine::params::{map_to_params, params_to_map, Params};
use crate::tuner::bounds::{Bounds};


/// Perturb the current params into two : theta- and theta+ ready for an iteration of SPSA
pub fn perturb_params(
    base: &Params,
    bounds: &Bounds,
    c: f64,
) -> (Params, Params, BTreeMap<String, f64>) {

    let base_map = params_to_map(base);

    let mut plus = base_map.clone();
    let mut minus = base_map.clone();

    let mut deltas = BTreeMap::new();

    let mut rng = rand::rng();

    for (name, value) in base_map {

        let bound = bounds.params.get(&name).unwrap();

        if !bound.include {
            continue;
        }

        let delta = if rng.random_bool(0.5) { 1.0 } else { -1.0 };
        deltas.insert(name.clone(), delta);

        let step = c * delta;

        if let Value::Number(n) = value {

            let v = n.as_f64().unwrap();

            // normalize
            let x = normalize(v, bound.min, bound.max);

            // perturb in normalized space
            let x_plus = (x + step).clamp(0.0, 1.0);
            let x_minus = (x - step).clamp(0.0, 1.0);

            // denormalize
            let plus_real = denormalize(x_plus, bound.min, bound.max);
            let minus_real = denormalize(x_minus, bound.min, bound.max);

            plus.insert(name.clone(), serde_yaml::to_value(plus_real).unwrap());
            minus.insert(name.clone(), serde_yaml::to_value(minus_real).unwrap());
        }

    }

    (
        map_to_params(plus),
        map_to_params(minus),
        deltas,
    )
}

pub fn apply_update(
    base: &Params,
    bounds: &Bounds,
    ak: f64,
    ck: f64,
    score: f64,
    deltas: BTreeMap<String,f64>,
) -> Params {

    let mut map = params_to_map(base);

    for (name, delta) in deltas {

        let bound = bounds.params.get(&name).unwrap();

        let g = score / (2.0 * ck * delta);

        let step = ak * g;

        let val = map.get(&name).unwrap().clone();

        if let Value::Number(n) = val {

            let v = n.as_f64().unwrap();

            // normalize
            let x = normalize(v, bound.min, bound.max);

            // SPSA update in normalized space
            let x_new = (x + step).clamp(0.0, 1.0);

            // denormalize
            let real_new = denormalize(x_new, bound.min, bound.max);

            map.insert(name, serde_yaml::to_value(real_new).unwrap());
        }

    }

    map_to_params(map)
}
fn normalize(x: f64, min: f64, max: f64) -> f64 {
    (x - min) / (max - min)
}

fn denormalize(x: f64, min: f64, max: f64) -> f64 {
    min + x * (max - min)
}



