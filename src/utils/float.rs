#![allow(dead_code)]

pub trait ExtendFloat: num_traits::Float {
    fn round_to_fixed(&self, decimals: u8) -> Self;
    fn round_to_fixed_string(&self, decimals: u8) -> String;
}

impl ExtendFloat for f64 {
    fn round_to_fixed(&self, decimals: u8) -> Self {
        let precision = 10.0_f64.powi(decimals as i32);
        (self * precision).round() / precision
    }
    fn round_to_fixed_string(&self, decimals: u8) -> String {
        format!("{:.*}", decimals as usize, self)
    }
}
