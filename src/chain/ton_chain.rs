use serde::Deserialize;
use std::str::FromStr;
use tonlib_core::TonAddress;

use num_bigint::BigUint;
use reqwest::{Client, Url};
use serde::de::DeserializeOwned;

use crate::utils::{
    retry::get_retry_time,
    support_option::{SupportOption, ToSupported},
};

use super::{Chain, ChainOps, ChainProperties, Token};

#[derive(Debug)]
pub struct TonChain {
    pub properties: ChainProperties,
    http_client: Client,
}

impl From<&Chain> for TonChain {
    fn from(value: &Chain) -> Self {
        Self {
            properties: value.properties.clone(),
            http_client: value.http_client.clone(),
        }
    }
}

#[derive(Deserialize, Debug)]
struct TonGetAccountResponse {
    balance: u64,
}

#[derive(Deserialize, Debug, Clone)]
struct TonJetton {
    address: String,
    symbol: String,
    decimals: usize,
}

#[derive(Deserialize, Debug, Clone)]
struct TonGetAccountJettonBalanceResponse {
    balance: String,
    jetton: TonJetton,
}

#[derive(Deserialize, Debug, Clone)]
struct TonGetAccountJettonsBalancesResponse {
    balances: Vec<TonGetAccountJettonBalanceResponse>,
}

#[derive(Deserialize, Debug, Clone)]
struct JettonMetadata {
    decimals: String,
}

#[derive(Deserialize, Debug, Clone)]
struct TonGetJettonInfo {
    metadata: JettonMetadata,
}

impl TonChain {
    fn parse_address_to_base64(address: &str, is_token: bool) -> Option<String> {
        TonAddress::from_base64_url(address)
            .ok()
            .or(TonAddress::from_hex_str(address).ok())
            .map(|a| a.to_base64_url_flags(!is_token, false))
    }
    async fn api_call<T: DeserializeOwned>(
        &self,
        route: String,
        query_pairs: Vec<(&str, &str)>,
    ) -> (Option<T>, Option<f32>) {
        let mut url = Url::parse(&format!("{}/{}", self.properties.rpc_urls[0], route)).unwrap();
        url.query_pairs_mut().extend_pairs(query_pairs);
        let response = match self
            .http_client
            .get(url)
            .headers(self.properties.rpc_headers.clone())
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
}

impl ChainOps for TonChain {
    async fn get_native_token_balance(
        &self,
        address: &str,
        _rpc_index: usize,
    ) -> (Option<BigUint>, Option<f32>) {
        let (balance, wait_time) = self
            .api_call::<TonGetAccountResponse>(format!("accounts/{address}"), vec![])
            .await;
        (balance.map(|b| BigUint::from(b.balance)), wait_time)
    }
    async fn get_token_balance(
        &self,
        token: &Token,
        address: &str,
        _rpc_index: usize,
    ) -> (Option<BigUint>, Option<f32>) {
        let (balance, wait_time) = self
            .api_call::<TonGetAccountJettonBalanceResponse>(
                format!("accounts/{}/jettons{}", address, token.address),
                vec![],
            )
            .await;
        (
            balance.and_then(|b| BigUint::from_str(b.balance.as_str()).ok()),
            wait_time,
        )
    }
    async fn get_holdings_balance(
        &self,
        address: &str,
        _rpc_index: usize,
    ) -> SupportOption<Vec<(String, BigUint)>> {
        let address = self.parse_wallet_address(address).to_supported()?;
        self.api_call::<TonGetAccountJettonsBalancesResponse>(
            format!("accounts/{address}/jettons"),
            vec![],
        )
        .await
        .0
        .to_supported()?
        .balances
        .iter()
        .map(|b| {
            Some((
                self.parse_token_address(&b.jetton.address)?,
                BigUint::from_str(&b.balance).ok()?,
            ))
        })
        .collect::<Option<_>>()
        .into()
    }
    async fn get_token_decimals(&self, token_address: &str, _rpc_index: usize) -> Option<usize> {
        usize::from_str(
            &self
                .api_call::<TonGetJettonInfo>(format!("jettons/{token_address}"), vec![])
                .await
                .0?
                .metadata
                .decimals,
        )
        .ok()
    }
    async fn scan_for_tokens(&self, address: &str, _rpc_index: usize) -> SupportOption<Vec<Token>> {
        let address = self.parse_wallet_address(address).to_supported()?;
        self.api_call::<TonGetAccountJettonsBalancesResponse>(
            format!("accounts/{address}/jettons"),
            vec![],
        )
        .await
        .0
        .to_supported()?
        .balances
        .iter()
        .map(|b| {
            Some(Token {
                address: self.parse_token_address(&b.jetton.address)?,
                symbol: b.jetton.symbol.clone(),
                decimals: b.jetton.decimals,
            })
        })
        .collect::<Option<_>>()
        .into()
    }
    fn parse_token_address(&self, address: &str) -> Option<String> {
        Self::parse_address_to_base64(address, true)
    }
    fn parse_wallet_address(&self, address: &str) -> Option<String> {
        Self::parse_address_to_base64(address, false)
    }
}
