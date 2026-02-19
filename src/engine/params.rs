
#[derive(Clone)]
pub struct Params {
    pub piece_values: [f32; 6],

}

impl Params {
    pub fn default() -> Self {
        Self {
            piece_values: [100.0, 320.0, 330.0, 500.0, 900.0, 0.0], // P, N, B, R, Q, K
        }
    }
}
