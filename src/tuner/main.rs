

use crate::engine::params::Params;
use crate::tuner::bounds::Bounds;
use crate::tuner::logger::{_csv_to_yaml, elo_from_wdl, log_yaml_to_csv};
use crate::tuner::matcher::run_match;
use crate::tuner::perturb::{apply_update, perturb_params};

pub fn run_spsa() {

    let theta_minus_path = "src/tuner/config/theta_minus.yaml";
    let theta_plus_path = "src/tuner/config/theta_plus.yaml";
    let games_per_iteration = 10;

    let mut base_params = Params::load_yaml("src/tuner/config/best_params.yaml");
    let bounds = Bounds::load_yaml("src/tuner/config/bounds.yaml");
    let total_iterations = 3000;
    let a = 0.01;
    let biga = 0.1;
    let c = 0.05;
    let alpha = 0.602;
    let gamma = 0.101;
    //let x = _csv_to_yaml("src/tuner/logging/spsa_params.csv",1920,"src/tuner/config/params_1.yaml");

    for iter in 1921 ..total_iterations {

        println!("Iteration {}", iter);



        if iter % 10 == 0 {
            // every now and then run match against my base version to see improvements
            println!("Playing match against base version.");


            let result = run_match("src/tuner/config/best_params.yaml", "src/tuner/config/params.yaml", "BEST","BASE",games_per_iteration);

            let elo = elo_from_wdl(result.wins, result.losses, result.draws);

            log_yaml_to_csv(iter, "src/tuner/config/best_params.yaml", "src/tuner/logging/spsa_params.csv",elo); // Log parameters to plot
        }

        let ak = a/(iter as f64+biga).powf(alpha);
        let ck = c/(iter as f64).powf(gamma);
        let typical_step = ak / (2.0 * ck);  // gradient step per unit score

        println!("ak: {:.5} ck: {:.5}", ak, ck);
        println!("typical normalized step: {:.6}", typical_step);


        // perturb
        let (theta_plus, theta_minus, deltas) = perturb_params(&base_params, &bounds, ck);

        // 2 save them
        theta_plus.save_yaml(theta_plus_path);
        theta_minus.save_yaml(theta_minus_path);


        // 3 run match
        let result = run_match(
            theta_plus_path,
            theta_minus_path,
            "THETA+",
            "THETA-",
            games_per_iteration
        );


        println!("Result: W:{} L:{} D:{}", result.wins, result.losses, result.draws);

        // 4 convert WDL to score
        let score = (result.wins as f64 - result.losses as f64) / games_per_iteration as f64;

        println!("Score: {}", score);

        // 5 SPSA update
        base_params = apply_update(
            &base_params,
            &bounds,
            ak,
            ck,
            score,
            deltas
        );


        // 6 save new params
        base_params.save_yaml("src/tuner/config/best_params.yaml");
    }
    println!("Done!");
}
