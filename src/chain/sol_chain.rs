use std::str::FromStr;

use base58::FromBase58;
use curve25519_dalek::edwards::CompressedEdwardsY;
use num_bigint::BigUint;
use num_traits::FromPrimitive;
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use serde_query::Deserialize;

use crate::{chain::*, dexscreener, utils::get_retry_time};

#[derive(Debug, Clone)]
pub struct SolChain {
    properties: ChainProperties,
    http_client: Client,
}

impl From<&Chain> for SolChain {
    fn from(value: &Chain) -> Self {
        Self {
            properties: value.properties.clone(),
            http_client: value.http_client.clone(),
        }
    }
}

impl SolChain {
    async fn rpc_call<T: DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
    ) -> (Option<T>, Option<f32>) {
        let payload = json!({
            "jsonrpc": "2.0",
            "id": "1",
            "method": method,
            "params": params,
        });
        let response = match self
            .http_client
            .post(self.properties.rpc_url.clone())
            .json(&payload)
            .send()
            .await
            .ok()
        {
            Some(x) => x,
            None => return (None, None),
        };
        let seconds = get_retry_time(&response);
        (response.json::<T>().await.ok(), seconds)
    }
    fn to_b58(address: &str) -> Option<Vec<u8>> {
        let address_b58 = address.from_base58().ok()?;
        if address_b58.len() != 32 {
            return None;
        }
        Some(address_b58)
    }
}

#[derive(Deserialize, Debug)]
struct SolGetBalanceResponse {
    #[query(".result.value")]
    value: u64,
}

#[derive(Deserialize, Debug, Clone)]
struct SolGetTokenBalanceResponse {
    #[query(".result.value.[].account.data.parsed.info.tokenAmount.amount")]
    token_amounts: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct SolGetTokenDecimalsResponse {
    #[query(".result.value.data.parsed.info.decimals")]
    decimals: usize,
}

#[derive(Deserialize, Debug, Clone)]
struct SolSplToken {
    #[query(".account.data.parsed.info.mint")]
    mint: String,
    #[query(".account.data.parsed.info.tokenAmount.decimals")]
    decimals: u64,
}

#[derive(Deserialize, Debug, Clone)]
struct SolGetTokenAccountsResponse {
    #[query(".result.value")]
    value: Vec<SolSplToken>,
}

impl ChainOps for SolChain {
    async fn get_native_token_balance(&self, address: String) -> (Option<BigUint>, Option<f32>) {
        let (balance, wait_time) = self
            .rpc_call::<SolGetBalanceResponse>("getBalance", json!([address]))
            .await;
        (balance.and_then(|b| BigUint::from_u64(b.value)), wait_time)
    }
    async fn get_token_balance(
        &self,
        token: &Token,
        address: String,
    ) -> (Option<BigUint>, Option<f32>) {
        let params = json!([
            address,
            { "mint": token.address },
            { "encoding": "jsonParsed" },
        ]);
        let (balances, wait_time) = self
            .rpc_call::<SolGetTokenBalanceResponse>("getTokenAccountsByOwner", params)
            .await;
        if balances.is_some() && balances.clone().unwrap().token_amounts.len() == 0 {
            return (Some(BigUint::ZERO), wait_time);
        }
        (
            balances.and_then(|b| BigUint::from_str(&b.token_amounts[0]).ok()),
            wait_time,
        )
    }
    async fn get_holdings_balance(
        &self,
        _address: String,
    ) -> SupportOption<Vec<(String, BigUint)>> {
        SupportOption::Unsupported
    }
    async fn get_token_decimals(&self, token_address: String) -> Option<usize> {
        let params = json!([
            token_address,
            { "encoding": "jsonParsed" },
        ]);
        Some(
            self.rpc_call::<SolGetTokenDecimalsResponse>("getAccountInfo", params)
                .await
                .0?
                .decimals,
        )
    }
    async fn scan_for_tokens(&self, address: String) -> SupportOption<Vec<Token>> {
        let params = json!([
            address,
            { "programId": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA" },
            { "encoding": "jsonParsed" },
        ]);
        let tokens_data = self
            .rpc_call::<SolGetTokenAccountsResponse>("getTokenAccountsByOwner", params)
            .await
            .0
            .to_supported()?
            .value;
        let token_addresses = tokens_data.iter().map(|token| token.mint.clone()).collect();
        let pairs = dexscreener::get_pairs(token_addresses)
            .await
            .to_supported()?;
        SupportOption::SupportedSome(
            tokens_data
                .iter()
                .filter_map(|token| {
                    pairs.iter().find_map(|pair| {
                        (pair.base_token.address == token.mint).then(|| Token {
                            address: token.mint.clone(),
                            decimals: token.decimals as usize,
                            symbol: pair.base_token.symbol.clone(),
                        })
                    })
                })
                .collect(),
        )
    }
    fn parse_wallet_address(&self, address: &str) -> Option<String> {
        let address_b58 = SolChain::to_b58(address)?;
        CompressedEdwardsY::from_slice(&address_b58)
            .ok()?
            .decompress()?;
        Some(address.to_string())
    }
    fn parse_token_address(&self, address: &str) -> Option<String> {
        SolChain::to_b58(address)?;
        Some(address.to_string())
    }
}
