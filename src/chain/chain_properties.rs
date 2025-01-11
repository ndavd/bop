use super::token::Token;
use reqwest::{header::HeaderMap, Url};
use std::fmt::Display;

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
