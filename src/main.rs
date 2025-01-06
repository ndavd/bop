#![feature(try_trait_v2)]

mod chain;
mod dexscreener;
mod repl;
mod utils;

use repl::Repl;

#[tokio::main]
async fn main() {
    if let Err(err) = Repl::default().run().await {
        eprintln!("Error: {}", err);
    }
}
