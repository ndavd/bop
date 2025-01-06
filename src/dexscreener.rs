#![allow(dead_code)]

use std::str::FromStr;

use futures::future::join_all;
use reqwest::{Client, Url};
use serde::Deserialize;

pub const DEXSCREENER_API_URL: &str = "https://api.dexscreener.com";

#[derive(Deserialize, Debug, Clone)]
pub struct Token {
    pub address: String,
    pub name: String,
    pub symbol: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Pair {
    pub chain_id: String,
    pub dex_id: String,
    pub url: String,
    pub pair_address: String,
    pub base_token: Token,
    pub quote_token: Token,
    pub price_native: String,
    pub price_usd: Option<String>,
}

#[derive(Deserialize, Debug)]
struct GetPairsResponse {
    pairs: Vec<Pair>,
}

async fn get_pairs_request(url: Url) -> Option<Vec<Pair>> {
    Some(
        Client::new()
            .get(url)
            .send()
            .await
            .ok()?
            .json::<GetPairsResponse>()
            .await
            .ok()?
            .pairs,
    )
}

pub async fn get_pairs(addresses: Vec<String>) -> Option<Vec<Pair>> {
    let requests = addresses.chunks(25).map(|a| {
        let url = Url::from_str(
            format!("{DEXSCREENER_API_URL}/latest/dex/tokens/{}", a.join(",")).as_str(),
        )
        .unwrap();
        get_pairs_request(url)
    });
    let results = join_all(requests).await;
    let pairs = results
        .iter()
        .filter_map(|r| r.clone())
        .flatten()
        .collect::<Vec<_>>();
    if pairs.len() == 0 {
        return None;
    }
    Some(pairs)
}
