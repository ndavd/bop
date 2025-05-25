use sha3::{Digest, Keccak256};

use num_bigint::BigUint;
use num_traits::ToPrimitive;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{chain::*, utils::retry::get_retry_time};

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
    async fn rpc_call(
        &self,
        method: &str,
        params: Value,
        rpc_index: usize,
    ) -> (Option<String>, Option<f32>) {
        let payload = json!({
            "jsonrpc": "2.0",
            "id": "1",
            "method": method,
            "params": params,
        });
        let response = match self
            .http_client
            .post(self.properties.rpc_urls[rpc_index % self.properties.rpc_urls.len()].clone())
            .json(&payload)
            .send()
            .await
            .ok()
        {
            Some(x) => x,
            None => return (None, None),
        };
        let seconds = get_retry_time(&response);
        (
            response
                .json::<EthCallResponse>()
                .await
                .ok()
                .map(|x| x.result),
            seconds,
        )
    }
}

impl ChainOps for EvmChain {
    async fn get_native_token_balance(
        &self,
        address: &str,
        rpc_index: usize,
    ) -> (Option<BigUint>, Option<f32>) {
        let (balance_hex, wait_time) = self
            .rpc_call("eth_getBalance", json!([address, "latest"]), rpc_index)
            .await;
        (
            balance_hex.and_then(|b| BigUint::parse_bytes(&b.as_bytes()[2..], 16)),
            wait_time,
        )
    }
    async fn get_token_balance(
        &self,
        token: &Token,
        address: &str,
        rpc_index: usize,
    ) -> (Option<BigUint>, Option<f32>) {
        let params = json!([
            {
                "to": token.address,
                "data": format!("0x70a08231000000000000000000000000{}", &address[2..])
            },
            "latest"
        ]);
        let (balance_hex, wait_time) = self.rpc_call("eth_call", params, rpc_index).await;
        (
            balance_hex.and_then(|b| BigUint::parse_bytes(&b.as_bytes()[2..], 16)),
            wait_time,
        )
    }
    async fn get_holdings_balance(
        &self,
        _address: &str,
        _rpc_index: usize,
    ) -> SupportOption<Vec<(String, BigUint)>> {
        SupportOption::Unsupported
    }
    async fn get_token_decimals(&self, token_address: &str, rpc_index: usize) -> Option<usize> {
        let params = json!([
            {
                "to": token_address,
                "data": "0x313ce567",
            },
            "latest"
        ]);
        let decimals_hex = self.rpc_call("eth_call", params, rpc_index).await.0?;
        BigUint::parse_bytes(&decimals_hex.as_bytes()[2..], 16)?.to_usize()
    }
    async fn scan_for_tokens(
        &self,
        _address: &str,
        _rpc_index: usize,
    ) -> SupportOption<Vec<Token>> {
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
        if !address.chars().all(|c| c.is_ascii_hexdigit()) {
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
