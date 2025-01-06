use serde::Deserialize;
use std::str::FromStr;
use tonlib_core::TonAddress;

use num_bigint::BigUint;
use reqwest::{Client, Url};
use serde::de::DeserializeOwned;

use crate::utils::{SupportOption, ToSupported};

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
    ) -> Option<T> {
        let mut url = Url::parse(&format!(
            "{}/{}",
            self.properties.rpc_url.to_string(),
            route
        ))
        .unwrap();
        url.query_pairs_mut().extend_pairs(query_pairs);
        self.http_client
            .get(url)
            .headers(self.properties.rpc_headers.clone())
            .send()
            .await
            .ok()?
            .json::<T>()
            .await
            .ok()
    }
}

impl ChainOps for TonChain {
    async fn get_native_token_balance(&self, address: String) -> Option<BigUint> {
        let address = self.parse_wallet_address(&address)?;
        Some(BigUint::from(
            self.api_call::<TonGetAccountResponse>(format!("accounts/{address}"), vec![])
                .await?
                .balance,
        ))
    }
    async fn get_token_balance(&self, token: &Token, address: String) -> SupportOption<BigUint> {
        let address = self.parse_wallet_address(&address).to_supported()?;
        BigUint::from_str(
            self.api_call::<TonGetAccountJettonBalanceResponse>(
                format!("accounts/{}/jettons{}", address, token.address),
                vec![],
            )
            .await
            .to_supported()?
            .balance
            .as_str(),
        )
        .ok()
        .into()
    }
    async fn get_holdings_balance(&self, address: String) -> SupportOption<Vec<(String, BigUint)>> {
        let address = self.parse_wallet_address(&address).to_supported()?;
        self.api_call::<TonGetAccountJettonsBalancesResponse>(
            format!("accounts/{}/jettons", address),
            vec![],
        )
        .await
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
    async fn get_token_decimals(&self, token_address: String) -> SupportOption<usize> {
        usize::from_str(
            &self
                .api_call::<TonGetJettonInfo>(format!("jettons/{token_address}"), vec![])
                .await
                .to_supported()?
                .metadata
                .decimals,
        )
        .ok()
        .into()
    }
    async fn scan_for_tokens(&self, address: String) -> SupportOption<Vec<Token>> {
        let address = self.parse_wallet_address(&address).to_supported()?;
        self.api_call::<TonGetAccountJettonsBalancesResponse>(
            format!("accounts/{}/jettons", address),
            vec![],
        )
        .await
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
