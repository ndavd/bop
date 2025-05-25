#![allow(dead_code)]

use std::{str::FromStr, sync::Arc};

use futures::{stream, StreamExt};
use reqwest::{Client, Url};
use serde::Deserialize;

use crate::utils::retry::handle_retry;

#[derive(Deserialize, Debug, Clone)]
pub struct Token {
    pub address: String,
    pub name: String,
    pub symbol: String,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct PairLiquidity {
    pub usd: Option<f64>,
    pub base: f64,
    pub quote: f64,
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
    pub liquidity: Option<PairLiquidity>,
}

#[derive(Deserialize, Debug)]
struct GetPairsResponse {
    pairs: Option<Vec<Pair>>,
}

async fn get_pairs_request(url: Url) -> Option<Vec<Pair>> {
    let response = Client::new().get(url.clone()).send().await.ok()?;
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
            let url = Url::from_str(
                format!("https://api.dexscreener.com/latest/dex/tokens/{t}").as_str(),
            )
            .unwrap();
            let task = async |_rpc_index| (get_pairs_request(url.clone()).await, None);
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
                .max_by(|pair_a, pair_b| {
                    let liq_a = pair_a
                        .liquidity
                        .clone()
                        .unwrap_or_default()
                        .usd
                        .unwrap_or_default();
                    let liq_b = pair_b
                        .liquidity
                        .clone()
                        .unwrap_or_default()
                        .usd
                        .unwrap_or_default();
                    liq_a.total_cmp(&liq_b)
                })
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
