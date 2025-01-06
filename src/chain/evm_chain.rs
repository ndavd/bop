use std::time::Duration;

use reqwest::{Response, StatusCode};
use sha3::{Digest, Keccak256};

use num_bigint::BigUint;
use num_traits::ToPrimitive;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::time::sleep;

use crate::chain::*;

pub struct EvmChain {
    properties: ChainProperties,
    http_client: Client,
}

#[derive(Deserialize, Debug)]
struct EthCallResponse {
    result: String,
}

impl From<&Chain> for EvmChain {
    fn from(value: &Chain) -> Self {
        Self {
            properties: value.properties.clone(),
            http_client: value.http_client.clone(),
        }
    }
}

impl EvmChain {
    async fn rpc_call(&self, method: &str, params: Value) -> Option<String> {
        let payload = json!({
            "jsonrpc": "2.0",
            "id": "1",
            "method": method,
            "params": params,
        });
        let mut response: Response;
        loop {
            response = self
                .http_client
                .post(self.properties.rpc_url.clone())
                .json(&payload)
                .send()
                .await
                .ok()?;
            let status = response.status();
            if status != StatusCode::TOO_MANY_REQUESTS {
                break;
            }
            eprintln!("evm {status}");
            if let Some(retry_after) = response.headers().get("Retry-After") {
                let seconds: u64 = retry_after.to_str().unwrap_or("0").parse().unwrap_or(0);
                eprintln!("Retrying after {}...", seconds);
                sleep(Duration::from_secs_f32(seconds as f32 * 1.5)).await;
            }
        }
        Some(response.json::<EthCallResponse>().await.ok()?.result)
    }
}

impl ChainOps for EvmChain {
    async fn get_native_token_balance(&self, address: String) -> Option<BigUint> {
        let balance_hex = self
            .rpc_call("eth_getBalance", json!([address, "latest"]))
            .await?;
        BigUint::parse_bytes(&balance_hex.as_bytes()[2..], 16)
    }
    async fn get_token_balance(&self, token: &Token, address: String) -> SupportOption<BigUint> {
        let params = json!([
            {
                "to": token.address,
                "data": format!("0x70a08231000000000000000000000000{}", &address[2..])
            },
            "latest"
        ]);
        let balance_hex = self.rpc_call("eth_call", params).await.to_supported()?;
        BigUint::parse_bytes(&balance_hex.as_bytes()[2..], 16).into()
    }
    async fn get_holdings_balance(
        &self,
        _address: String,
    ) -> SupportOption<Vec<(String, BigUint)>> {
        SupportOption::Unsupported
    }
    async fn get_token_decimals(&self, token_address: String) -> SupportOption<usize> {
        let params = json!([
            {
                "to": token_address,
                "data": "0x313ce567",
            },
            "latest"
        ]);
        let decimals_hex = self.rpc_call("eth_call", params).await.to_supported()?;
        BigUint::parse_bytes(&decimals_hex.as_bytes()[2..], 16)
            .to_supported()?
            .to_usize()
            .into()
    }
    async fn scan_for_tokens(&self, _address: String) -> SupportOption<Vec<Token>> {
        SupportOption::Unsupported
    }
    fn parse_wallet_address(&self, address: &str) -> Option<String> {
        if !address.starts_with("0x") {
            return None;
        }
        let address = &address[2..];
        if address.len() != 40 {
            return None;
        }
        if !address.chars().all(|c| c.is_digit(16)) {
            return None;
        }
        let mut hasher = Keccak256::new();
        hasher.update(address.to_lowercase());
        let hash = hasher.finalize();
        let mut checksummed_address = String::from("0x");
        for (i, c) in address.chars().enumerate() {
            if (hash[i / 2] >> (4 - (i % 2) * 4) & 0xf) > 7 {
                checksummed_address.push_str(&c.to_uppercase().to_string());
            } else {
                checksummed_address.push(c);
            }
        }
        Some(checksummed_address)
    }
}
