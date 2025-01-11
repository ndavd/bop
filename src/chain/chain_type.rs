use std::str::FromStr;

use serde::{Deserialize, Serialize};

pub static CHAIN_TYPES: &[ChainType; 3] = &[ChainType::Evm, ChainType::Solana, ChainType::Ton];

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum ChainType {
    Evm,
    Solana,
    Ton,
}

impl ChainType {
    pub fn label(&self) -> String {
        match self {
            Self::Evm => "EVM",
            Self::Solana => "Solana",
            Self::Ton => "Ton",
        }
        .to_string()
    }
}

impl ToString for ChainType {
    fn to_string(&self) -> String {
        match self {
            Self::Evm => "evm",
            Self::Solana => "sol",
            Self::Ton => "ton",
        }
        .to_string()
    }
}

impl FromStr for ChainType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "evm" => Ok(Self::Evm),
            "sol" => Ok(Self::Solana),
            "ton" => Ok(Self::Ton),
            x => Err(format!("{x:?} is not a valid chain-type")),
        }
    }
}
