#![allow(dead_code)]

use std::{str::FromStr, sync::Arc};

use futures::{stream, StreamExt};
use reqwest::{Client, StatusCode, Url};
use serde::Deserialize;

use crate::utils::retry::handle_retry;

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
    pub market_cap: Option<u64>,
}

#[derive(Deserialize, Debug)]
struct GetPairsResponse {
    pairs: Option<Vec<Pair>>,
}

async fn get_pairs_request(url: Url) -> Option<Vec<Pair>> {
    let response = Client::new().get(url.clone()).send().await.ok()?;
    let status = response.status();
    if status != StatusCode::OK {
        eprintln!("PRICES {status}");
    }
    response
        .json::<GetPairsResponse>()
        .await
        .ok()?
        .pairs
        .or(Some(Vec::new()))
}

pub async fn _get_pairs<F>(tokens: Vec<&str>, progress_handler: Option<F>) -> Option<Vec<Pair>>
where
    F: Fn(),
{
    let progress_handler = Arc::new(progress_handler);
    let pairs = stream::iter(tokens.clone())
        .map(async |t| {
            let url =
                Url::from_str(format!("{DEXSCREENER_API_URL}/latest/dex/tokens/{}", t).as_str())
                    .unwrap();
            let task = async || (get_pairs_request(url.clone()).await, None);
            let result = handle_retry(task).await;
            if let Some(handler) = progress_handler.as_ref() {
                handler();
            }
            result
        })
        .buffer_unordered(20)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let p = tokens
        .iter()
        .filter_map(|token| {
            pairs
                .iter()
                .filter(|pair| pair.base_token.address == *token)
                .max_by_key(|pair| pair.market_cap.unwrap_or(0))
                .cloned()
        })
        .collect::<Vec<_>>();
    Some(p)
}

pub async fn get_pairs_with_progress<F>(
    tokens: Vec<&str>,
    progress_handler: Option<F>,
) -> Option<Vec<Pair>>
where
    F: Fn(),
{
    _get_pairs(tokens, progress_handler).await
}

pub async fn get_pairs(tokens: Vec<&str>) -> Option<Vec<Pair>> {
    _get_pairs::<fn()>(tokens, None).await
}
