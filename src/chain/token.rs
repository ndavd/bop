use num_bigint::BigUint;
use serde::{Deserialize, Serialize};

use super::{Chain, ChainOps};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub symbol: String,
    pub address: String,
    pub decimals: usize,
}

impl Token {
    pub fn hardcode(symbol: &str, address: &str, decimals: usize) -> Self {
        Self {
            symbol: symbol.to_string(),
            address: address.to_string(),
            decimals,
        }
    }
    pub async fn new(address: &str, chain: &Chain) -> Option<Self> {
        let decimals = chain.get_token_decimals(address, 0).await?;
        let symbol = chain.get_token_symbol(address, 0).await?;
        Some(Self {
            symbol,
            address: chain.parse_token_address(address)?,
            decimals,
        })
    }
    pub fn format(&self, value: &BigUint) -> f64 {
        let mut value = value.to_string();
        let mag = value.len() as isize - self.decimals as isize;
        if mag > 0 {
            value.insert(mag as usize, '.');
        } else {
            value = format!("0.{}{value}", "0".repeat(mag.unsigned_abs()));
        }
        value.parse().unwrap()
    }
}
