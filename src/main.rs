#![feature(try_trait_v2)]

mod chain;
mod dexscreener;
mod repl;
mod utils;

use repl::Repl;

#[tokio::main]
async fn main() {
    if let Some(arg) = std::env::args().nth(1) {
        if arg.as_str() == "--version" {
            return println!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        }
    }
    if let Err(err) = Repl::default().run().await {
        eprintln!("Error: {err}");
    }
}
