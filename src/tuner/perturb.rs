use std::collections::BTreeMap;
use rand::{RngExt};

use serde_yaml::Value;
use crate::engine::params::{map_to_params, params_to_map, Params};
use crate::tuner::bounds::{Bounds};

/// Perturb the current params into two : theta- and theta+ ready for an iteration of SPSA
/// TODO : Make the reading of the parameters dynamic so that I dont have to manually add every new parameter.
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

        let step = c * bound.step * delta;

        match value {

            Value::Number(n) if n.is_i64() => {

                let v = n.as_i64().unwrap() as f64;

                plus.insert(
                    name.clone(),
                    serde_yaml::to_value((v + step).clamp(bound.min, bound.max) as i64).unwrap(),
                );

                minus.insert(
                    name.clone(),
                    serde_yaml::to_value((v - step).clamp(bound.min, bound.max) as i64).unwrap(),
                );
            }

            Value::Number(n) if n.is_f64() => {

                let v = n.as_f64().unwrap();

                plus.insert(
                    name.clone(),
                    serde_yaml::to_value((v + step).clamp(bound.min, bound.max)).unwrap(),
                );

                minus.insert(
                    name.clone(),
                    serde_yaml::to_value((v - step).clamp(bound.min, bound.max)).unwrap(),
                );
            }


            _ => {}
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

        let step = ak * g * bound.step;

        let val = map.get(&name).unwrap().clone();

        match val {

            Value::Number(n) if n.is_i64() => {

                let v = n.as_i64().unwrap() as f64;

                let new_v = (v + step).clamp(bound.min, bound.max);

                map.insert(name, serde_yaml::to_value(new_v as i64).unwrap());
            }

            Value::Number(n) if n.is_f64() => {

                let v = n.as_f64().unwrap();

                let new_v = (v + step).clamp(bound.min, bound.max);

                map.insert(name, serde_yaml::to_value(new_v).unwrap());
            }

            _ => {}
        }
    }

    map_to_params(map)
}




