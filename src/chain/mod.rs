pub mod chain_properties;
pub mod chain_type;
pub mod evm_chain;
pub mod sol_chain;
pub mod token;
pub mod ton_chain;

use std::str::FromStr;

use chain_properties::ChainProperties;
use chain_type::ChainType;
use evm_chain::EvmChain;
use num_bigint::BigUint;
use reqwest::{header::HeaderMap, Client, Url};
use sol_chain::SolChain;
use token::Token;
use ton_chain::TonChain;

use crate::{dexscreener, utils::support_option::SupportOption};

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
    async fn get_native_token_balance(&self, address: &str) -> (Option<BigUint>, Option<f32>);
    async fn get_token_balance(
        &self,
        token: &Token,
        address: &str,
    ) -> (Option<BigUint>, Option<f32>);
    async fn get_token_decimals(&self, token_address: &str) -> Option<usize>;
    async fn get_token_symbol(&self, token_address: &str) -> Option<String> {
        let pairs = dexscreener::pairs::get_pairs(vec![token_address]).await?;
        (pairs.len() != 0).then(|| pairs[0].base_token.symbol.clone())
    }
    async fn get_holdings_balance(&self, address: &str) -> SupportOption<Vec<(String, BigUint)>>;
    async fn scan_for_tokens(&self, address: &str) -> SupportOption<Vec<Token>>;
    fn parse_wallet_address(&self, address: &str) -> Option<String>;
    fn parse_token_address(&self, address: &str) -> Option<String> {
        self.parse_wallet_address(address)
    }
}

macro_rules! chain_ops_method {
    ($self:expr, $method:ident, $($args:expr),*; await) => {
        match $self.chain_type {
            ChainType::Evm => EvmChain::from($self).$method($($args),*).await,
            ChainType::Solana => SolChain::from($self).$method($($args),*).await,
            ChainType::Ton => TonChain::from($self).$method($($args),*).await,
        }
    };
    ($self:expr, $method:ident, $($args:expr),*) => {
        match $self.chain_type {
            ChainType::Evm => EvmChain::from($self).$method($($args),*),
            ChainType::Solana => SolChain::from($self).$method($($args),*),
            ChainType::Ton => TonChain::from($self).$method($($args),*),
        }
    };
}

impl ChainOps for Chain {
    async fn get_native_token_balance(&self, address: &str) -> (Option<BigUint>, Option<f32>) {
        chain_ops_method!(self, get_native_token_balance, address; await)
    }
    async fn get_token_balance(
        &self,
        token: &Token,
        address: &str,
    ) -> (Option<BigUint>, Option<f32>) {
        chain_ops_method!(self, get_token_balance, token, address; await)
    }
    async fn get_holdings_balance(&self, address: &str) -> SupportOption<Vec<(String, BigUint)>> {
        chain_ops_method!(self, get_holdings_balance, address; await)
    }
    async fn get_token_decimals(&self, token_address: &str) -> Option<usize> {
        chain_ops_method!(self, get_token_decimals, token_address; await)
    }
    async fn get_token_symbol(&self, token_address: &str) -> Option<String> {
        chain_ops_method!(self, get_token_symbol, token_address; await)
    }
    async fn scan_for_tokens(&self, address: &str) -> SupportOption<Vec<Token>> {
        chain_ops_method!(self, scan_for_tokens, address; await)
    }
    fn parse_wallet_address(&self, address: &str) -> Option<String> {
        chain_ops_method!(self, parse_wallet_address, address)
    }
    fn parse_token_address(&self, address: &str) -> Option<String> {
        chain_ops_method!(self, parse_token_address, address)
    }
}
