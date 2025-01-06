pub mod evm_chain;
pub mod sol_chain;
pub mod ton_chain;

use std::{fmt::Display, str::FromStr};

use evm_chain::EvmChain;
use num_bigint::BigUint;
use reqwest::{header::HeaderMap, Client, Url};
use serde::{Deserialize, Serialize};
use sol_chain::SolChain;
use ton_chain::TonChain;

use crate::{
    dexscreener,
    utils::{SupportOption, ToSupported},
};

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
    pub async fn new(address: String, chain: &Chain) -> SupportOption<Self> {
        let decimals = chain.get_token_decimals(address.clone()).await?;
        let symbol = chain.get_token_symbol(address.clone()).await?;
        SupportOption::SupportedSome(Self {
            symbol,
            address: chain.parse_token_address(&address).to_supported()?,
            decimals,
        })
    }
    pub fn format(&self, value: &BigUint) -> f64 {
        let mut value = value.to_string();
        let mag = value.len() as isize - self.decimals as isize;
        if mag > 0 {
            value.insert(mag as usize, '.');
        } else {
            value = format!("0.{}{value}", "0".repeat(mag.abs() as usize));
        }
        value.parse().unwrap()
    }
}

#[derive(Debug, Clone)]
pub struct ChainProperties {
    pub rpc_url: Url,
    pub rpc_headers: HeaderMap,
    pub name: String,
    pub native_token: Token,
}

impl Display for ChainProperties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.native_token.symbol)
    }
}

impl ChainProperties {
    pub fn get_id(&self) -> String {
        self.name.replace(" ", "").trim().to_lowercase()
    }
}

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

#[derive(Debug, Clone)]
pub struct Chain {
    pub chain_type: ChainType,
    pub properties: ChainProperties,
    http_client: Client,
}

impl Chain {
    pub fn new(
        chain_type: ChainType,
        rpc_url: &str,
        name: &str,
        native_token_symbol: &str,
        native_token_address: &str,
        native_token_decimals: usize,
    ) -> Self {
        let properties = ChainProperties {
            rpc_url: Url::from_str(rpc_url).unwrap(),
            rpc_headers: HeaderMap::new(),
            name: name.to_string(),
            native_token: Token::hardcode(
                native_token_symbol,
                native_token_address,
                native_token_decimals,
            ),
        };
        Self {
            chain_type,
            properties,
            http_client: Client::new(),
        }
    }
}

pub trait ChainOps {
    async fn get_native_token_balance(&self, address: String) -> Option<BigUint>;
    async fn get_token_balance(&self, token: &Token, address: String) -> SupportOption<BigUint>;
    async fn get_holdings_balance(&self, address: String) -> SupportOption<Vec<(String, BigUint)>>;
    async fn get_token_decimals(&self, token_address: String) -> SupportOption<usize>;
    async fn get_token_symbol(&self, token_address: String) -> SupportOption<String> {
        let pairs = dexscreener::get_pairs(vec![token_address])
            .await
            .to_supported()?;
        SupportOption::SupportedSome(
            (pairs.len() != 0)
                .then(|| pairs[0].base_token.symbol.clone())
                .to_supported()?,
        )
    }
    async fn scan_for_tokens(&self, address: String) -> SupportOption<Vec<Token>>;
    fn parse_wallet_address(&self, address: &str) -> Option<String>;
    fn parse_token_address(&self, address: &str) -> Option<String> {
        self.parse_wallet_address(address)
    }
}

impl ChainOps for Chain {
    async fn get_native_token_balance(&self, address: String) -> Option<BigUint> {
        match self.chain_type {
            ChainType::Evm => EvmChain::from(self).get_native_token_balance(address).await,
            ChainType::Solana => SolChain::from(self).get_native_token_balance(address).await,
            ChainType::Ton => TonChain::from(self).get_native_token_balance(address).await,
        }
    }
    async fn get_token_balance(&self, token: &Token, address: String) -> SupportOption<BigUint> {
        match self.chain_type {
            ChainType::Evm => EvmChain::from(self).get_token_balance(token, address).await,
            ChainType::Solana => SolChain::from(self).get_token_balance(token, address).await,
            ChainType::Ton => TonChain::from(self).get_token_balance(token, address).await,
        }
    }
    async fn get_holdings_balance(&self, address: String) -> SupportOption<Vec<(String, BigUint)>> {
        match self.chain_type {
            ChainType::Evm => EvmChain::from(self).get_holdings_balance(address).await,
            ChainType::Solana => SolChain::from(self).get_holdings_balance(address).await,
            ChainType::Ton => TonChain::from(self).get_holdings_balance(address).await,
        }
    }
    async fn get_token_decimals(&self, token_address: String) -> SupportOption<usize> {
        match self.chain_type {
            ChainType::Evm => EvmChain::from(self).get_token_decimals(token_address).await,
            ChainType::Solana => SolChain::from(self).get_token_decimals(token_address).await,
            ChainType::Ton => TonChain::from(self).get_token_decimals(token_address).await,
        }
    }
    async fn get_token_symbol(&self, token_address: String) -> SupportOption<String> {
        match self.chain_type {
            ChainType::Evm => EvmChain::from(self).get_token_symbol(token_address).await,
            ChainType::Solana => SolChain::from(self).get_token_symbol(token_address).await,
            ChainType::Ton => TonChain::from(self).get_token_symbol(token_address).await,
        }
    }
    async fn scan_for_tokens(&self, address: String) -> SupportOption<Vec<Token>> {
        match self.chain_type {
            ChainType::Evm => EvmChain::from(self).scan_for_tokens(address).await,
            ChainType::Solana => SolChain::from(self).scan_for_tokens(address).await,
            ChainType::Ton => TonChain::from(self).scan_for_tokens(address).await,
        }
    }
    fn parse_wallet_address(&self, address: &str) -> Option<String> {
        match self.chain_type {
            ChainType::Evm => EvmChain::from(self).parse_wallet_address(address),
            ChainType::Solana => SolChain::from(self).parse_wallet_address(address),
            ChainType::Ton => TonChain::from(self).parse_wallet_address(address),
        }
    }
    fn parse_token_address(&self, address: &str) -> Option<String> {
        match self.chain_type {
            ChainType::Evm => EvmChain::from(self).parse_token_address(address),
            ChainType::Solana => SolChain::from(self).parse_token_address(address),
            ChainType::Ton => TonChain::from(self).parse_token_address(address),
        }
    }
}
