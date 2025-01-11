use std::{collections::HashMap, fmt::Display, path::PathBuf, str::FromStr};

use age::secrecy::{ExposeSecret, SecretString};
use futures::{stream, StreamExt};
use itertools::Itertools;
use num_bigint::BigUint;
use reqwest::{header::HeaderMap, Url};
use rustyline::{error::ReadlineError, DefaultEditor};
use serde::{Deserialize, Serialize};

use crate::{
    chain::{Chain, ChainOps, ChainType, Token},
    dexscreener,
    utils::{retry::handle_retry_indexed, table::Table, text::StylizedText},
};

static DATA_FILE: &str = ".bop-data";
static BOOK_OF_PROFITS: &str = "Book of Profits";

static CHAIN_TYPES: &[ChainType; 3] = &[ChainType::Evm, ChainType::Solana, ChainType::Ton];

// TODO: These hashmaps aren't really needed, just use vectors of tuples
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct ReplConfig {
    /// Map of chain-type to account address and optional alias
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    accounts: HashMap<ChainType, Vec<(String, Option<String>)>>,
    /// Map of chain-id to custom rpc
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    rpcs: HashMap<String, String>,
    /// Map of chain-id to enabled
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    chains_enabled: HashMap<String, bool>,
    /// Map of chain-id to tokens
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    tokens: HashMap<String, Vec<Token>>,
}

impl Display for ReplConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match serde_json::to_string(self) {
                Ok(x) => x,
                _ => "ERR".to_string(),
            }
        )
    }
}

pub struct Repl {
    chains: Vec<Chain>,
    config: ReplConfig,
    secret: Option<SecretString>,
}

#[derive(Debug, Clone)]
struct ReplBalanceEntry {
    chain: String,
    account: String,
    token: Token,
    balance_native: BigUint,
    balance_usd: f64,
}

impl Repl {
    fn is_chain_enabled(&self, chain_name: &str) -> bool {
        match self.config.chains_enabled.get(chain_name) {
            Some(x) => *x,
            None => true,
        }
    }
    fn enabled_chains(&self) -> impl Iterator<Item = &Chain> {
        self.chains
            .iter()
            .filter(|c| self.is_chain_enabled(&c.properties.get_id()))
    }
    fn enabled_chains_of_type<'a>(
        &'a self,
        chain_type: &'a ChainType,
    ) -> impl Iterator<Item = &'a Chain> + 'a {
        self.chains.iter().filter(move |c| {
            c.chain_type == *chain_type && self.is_chain_enabled(&c.properties.get_id())
        })
    }
    fn chains_of_type<'a>(
        &'a self,
        chain_type: &'a ChainType,
    ) -> impl Iterator<Item = &'a Chain> + 'a {
        self.chains
            .iter()
            .filter(move |c| c.chain_type == *chain_type)
    }
    fn find_chain(&self, chain_name: &str) -> Result<&Chain, String> {
        match self
            .chains
            .iter()
            .find(|c| c.properties.get_id() == chain_name)
        {
            Some(x) => Ok(x),
            None => Err(format!(
                "There is no available chain with name {:?}",
                chain_name
            )),
        }
    }
    fn find_account_address(&self, account: &str) -> Result<(ChainType, String), String> {
        match self
            .config
            .accounts
            .iter()
            .flat_map(|(c, a)| {
                a.iter()
                    .map(|a| (c, a.0.clone(), a.1.clone()))
                    .collect::<Vec<_>>()
            })
            .find_map(|a| {
                (a.1 == account || a.2.is_some() && a.2.unwrap() == account)
                    .then(|| (a.0.clone(), a.1))
            }) {
            Some(x) => Ok(x),
            _ => Err(format!("Found no account corresponding to {:?}", account)),
        }
    }
    fn format_address(a: &str) -> String {
        let first = &a[..if a.starts_with("0x") { 7 } else { 5 }].to_string();
        let last = &a[a.len() - 5..].to_string();
        format!("{first}..{last}")
    }
    fn format_account(a: &(String, Option<String>)) -> String {
        if a.1.is_some() {
            return a.1.clone().unwrap();
        }
        Self::format_address(&a.0)
    }
    fn get_unknown_option_err(s: &str) -> Result<(), String> {
        Err(format!("Unknown option: {s:?}"))
    }
    fn get_unknown_option_expecting_err(s: &str) -> Result<(), String> {
        Err(format!("Unknown option, expecting: {s:?}"))
    }
    fn get_unknown_option_expecting_or_err(s: &[&str]) -> Result<(), String> {
        Err(format!("Unknown option, expecting: {s:?}"))
    }
    fn get_bad_argument_count_err() -> Result<(), String> {
        Err("Bad argument count".to_string())
    }
    fn display_help() {
        let help = r###"
chain - Display available chain-types and chains
    chain [chain] - Show chain information
    chain set [chain] [url] - Modify chain RPC url
    chain rm [chain] - Remove custom chain RPC url
    chain toggle [chain] - Toggle chain
    chain toggle-all [chain-type] - Toggle all chains of chain-type
account - Display accounts
    account add [chain-type] [address] [alias?] - Add new address to track, optionally pass an alias
    account rm [account] - Remove account
token - Display tokens
    token add [chain] [address] - Add new token
    token rm [chain] [address] - Remove token
    token scan [chain] [account] - Automatically scan account and add tokens
balance - Display global balance
config - Export BoP config in plain text
    config password - Change password
"###
        .trim()
        .lines()
        .map(|line| {
            let (command, description) = match line.split_once(" - ") {
                Some(x) => x,
                None => return line.to_string(),
            };
            format!("{} - {description}", command.to_colored())
        })
        .collect::<Vec<_>>()
        .join("\n");
        println!("{}\n{help}", "Commands".to_title());
    }
    fn handle_config(&mut self, command_parts: &[&str]) -> Result<(), String> {
        match command_parts.len() {
            0 => {
                match self.read_config_from_data_file(false) {
                    Ok(x) => println!("{x}"),
                    Err(x) => println!("{x}"),
                };
                Ok(())
            }
            _ => {
                if command_parts[0] == "password" {
                    self.create_password()?;
                    self.store_config_to_data_file()?;
                    println!("Password altered successfully");
                    return Ok(());
                }
                Self::get_unknown_option_err(command_parts[0])
            }
        }
    }
    fn handle_chain(&mut self, command_parts: &[&str]) -> Result<(), String> {
        match command_parts.len() {
            0 => {
                let available_chain_types = format!(
                    "{} currently supports the following chain-types: {}, {}, {}",
                    BOOK_OF_PROFITS.to_colored(),
                    ChainType::Evm.to_string().as_str().to_colored(),
                    ChainType::Solana.to_string().as_str().to_colored(),
                    ChainType::Ton.to_string().as_str().to_colored()
                );
                println!("{available_chain_types}\n");
                let note = r###"
To call a command involving a chain, use its internal ID to refer to it (check
tables bellow). All chains are enabled by default.

NOTE: Chains of type Ton make use of the TON API (https://tonapi.io) instead of
regular RPC endpoints. So instead of being able to set the RPC url for Ton, you
can use the same command to set an authentication token for the API.
                    "###
                .trim();
                println!("{note}\n");
                let table_titles =
                    Vec::from(["ID".to_string(), "Name".to_string(), "Enabled".to_string()]);
                for chain_type in CHAIN_TYPES {
                    let mut chains_of_type = self
                        .chains_of_type(chain_type)
                        .map(|c| {
                            Vec::from([
                                c.properties.get_id(),
                                c.properties.name.clone(),
                                self.is_chain_enabled(&c.properties.get_id()).to_string(),
                            ])
                        })
                        .collect::<Vec<_>>();
                    chains_of_type.insert(0, table_titles.clone());
                    let mut t = Table::from(chains_of_type);
                    t.title = format!("{} chains", chain_type.label());
                    println!("{t}");
                }
                Ok(())
            }
            1 => {
                let chain = self.find_chain(command_parts[0])?;
                println!(
                    "{} - {}",
                    chain.properties,
                    if self.is_chain_enabled(&chain.properties.get_id()) {
                        "ENABLED".to_colored()
                    } else {
                        "DISABLED".to_string()
                    }
                );
                println!("{}", chain.properties.rpc_url);
                Ok(())
            }
            2 => {
                let sub_command = command_parts[0];
                let arg = command_parts[1];
                match sub_command {
                    "rm" => {
                        let chain_name = self.find_chain(arg)?.properties.name.clone();
                        self.config.rpcs.remove_entry(arg);
                        self.store_config_to_data_file()?;
                        println!("{} chain set back to default state", chain_name);
                        Ok(())
                    }
                    "toggle" => {
                        let chain = self.find_chain(arg)?;
                        let chain_name = chain.properties.name.clone();
                        let chain_id = chain.properties.get_id();
                        let new_state = !self.is_chain_enabled(&chain_id);
                        self.config
                            .chains_enabled
                            .insert(chain_id.clone(), new_state);
                        self.store_config_to_data_file()?;
                        println!(
                            "{chain_name} chain set to {}",
                            if new_state { "enabled" } else { "disabled" }
                        );
                        Ok(())
                    }
                    "toggle-all" => {
                        let chain_type = ChainType::from_str(arg)?;
                        let chain_ids_of_type = self
                            .chains_of_type(&chain_type)
                            .map(|c| c.properties.get_id())
                            .collect::<Vec<_>>();
                        let new_state =
                            !chain_ids_of_type.iter().all(|id| self.is_chain_enabled(id));
                        for chain_id in chain_ids_of_type {
                            self.config.chains_enabled.insert(chain_id, new_state);
                        }
                        self.store_config_to_data_file()?;
                        println!(
                            "All chains of type {} set to {}",
                            chain_type.label(),
                            if new_state { "enabled" } else { "disabled" }
                        );
                        Ok(())
                    }
                    _ => Self::get_unknown_option_expecting_or_err(&["rm", "toggle", "toggle-all"]),
                }
            }
            3 => {
                let sub_command = command_parts[0];
                if sub_command != "set" {
                    return Repl::get_unknown_option_expecting_err("set");
                }
                let chain_id = command_parts[1];
                let arg = command_parts[2];
                let chain = self.find_chain(chain_id)?;
                if chain.chain_type != ChainType::Ton && Url::from_str(arg).is_err() {
                    return Err(format!("{:?} is not a valid url", arg));
                }
                self.config
                    .rpcs
                    .insert(chain_id.to_string(), arg.to_string());
                self.store_config_to_data_file()?;
                Ok(())
            }
            _ => Self::get_bad_argument_count_err(),
        }
    }
    fn handle_account(&mut self, command_parts: &[&str]) -> Result<(), String> {
        match command_parts.len() {
            0 => {
                let note = r###"
To call a command involving an account, you can use either its full address or 
alias, if set.
                    "###
                .trim();
                println!("{note}\n");
                if self.config.accounts.is_empty() {
                    println!("You have no accounts");
                }
                for chain_type in CHAIN_TYPES {
                    if let Some(accounts) = self.config.accounts.get(chain_type) {
                        let table_titles = Vec::from([
                            "Short address".to_string(),
                            "Full address".to_string(),
                            "Alias".to_string(),
                        ]);
                        let mut rows = accounts
                            .iter()
                            .map(|(address, alias)| {
                                Vec::from([
                                    Repl::format_address(address),
                                    address.to_string(),
                                    alias.clone().unwrap_or("-".to_string()),
                                ])
                            })
                            .collect::<Vec<_>>();
                        rows.insert(0, table_titles);
                        let mut t = Table::from(rows);
                        t.title = format!("{} accounts", chain_type.label());
                        println!("{t}");
                    }
                }
                Ok(())
            }
            2 => {
                let sub_command = command_parts[0];
                let arg = command_parts[1];
                if sub_command != "rm" {
                    return Repl::get_unknown_option_expecting_err("rm");
                }
                let (chain_type, address) = self.find_account_address(arg)?;
                let index = self
                    .config
                    .accounts
                    .get(&chain_type)
                    .unwrap()
                    .iter()
                    .position(|a| a.0 == address)
                    .unwrap();
                self.config
                    .accounts
                    .get_mut(&chain_type)
                    .unwrap()
                    .remove(index);
                self.store_config_to_data_file()?;
                Ok(())
            }
            3 | 4 => {
                let sub_command = command_parts[0];
                if sub_command != "add" {
                    return Repl::get_unknown_option_expecting_err("add");
                }
                let chain_type = ChainType::from_str(command_parts[1])?;
                let addr = command_parts[2];
                let address = match self
                    .chains_of_type(&chain_type)
                    .next()
                    .unwrap()
                    .parse_wallet_address(addr)
                {
                    Some(x) => x,
                    None => {
                        return Err(format!(
                            "{addr} is not a valid {} address",
                            chain_type.label()
                        ))
                    }
                };
                let alias = (command_parts.len() == 4).then(|| command_parts[3].to_string());
                let new_acc = (address, alias);
                if let Some(accounts) = self.config.accounts.get_mut(&chain_type) {
                    accounts.push(new_acc);
                } else {
                    self.config.accounts.insert(chain_type, vec![new_acc]);
                }
                self.store_config_to_data_file()?;
                Ok(())
            }
            _ => Self::get_bad_argument_count_err(),
        }
    }
    async fn handle_token(&mut self, command_parts: &[&str]) -> Result<(), String> {
        match command_parts.len() {
            0 => {
                if self.config.tokens.is_empty() {
                    println!("You have no tokens");
                }
                for (chain_id, chain_tokens) in &self.config.tokens {
                    let chain = self.find_chain(&chain_id)?;
                    let table_titles = Vec::from([
                        "Symbol".to_string(),
                        "Address".to_string(),
                        "Decimals".to_string(),
                    ]);
                    let mut tokens = chain_tokens
                        .iter()
                        .map(|t| {
                            Vec::from([t.symbol.clone(), t.address.clone(), t.decimals.to_string()])
                        })
                        .collect::<Vec<_>>();
                    tokens.insert(0, table_titles);
                    let mut t = Table::from(tokens);
                    t.title = format!("{} tokens", chain.properties.name);
                    println!("{t}");
                }
                Ok(())
            }
            3 => {
                let sub_command = command_parts[0];
                let chain_id = command_parts[1];
                let chain = self.find_chain(chain_id)?;
                let addr = command_parts[2];
                match sub_command {
                    "add" => {
                        let token_address = match chain.parse_token_address(addr) {
                            Some(x) => x,
                            None => {
                                return Err(format!(
                                    "{addr} is not a valid {} token address",
                                    chain.properties.name
                                ))
                            }
                        };
                        let token = match Token::new(token_address, &chain).await {
                            Some(x) => x,
                            None => return Err("Could not fetch token info".to_string()),
                        };
                        if self
                            .config
                            .tokens
                            .get(chain_id)
                            .unwrap_or(&vec![])
                            .iter()
                            .find(|t| t.address == token.address)
                            .is_some()
                        {
                            return Err("Token already added".to_string());
                        }
                        if let Some(tokens) = self.config.tokens.get_mut(chain_id) {
                            tokens.push(token);
                        } else {
                            self.config.tokens.insert(chain_id.to_string(), vec![token]);
                        }
                        self.store_config_to_data_file()
                    }
                    "rm" => {
                        let token_address = match chain.parse_token_address(addr) {
                            Some(x) => x,
                            None => {
                                return Err(format!(
                                    "{addr} is not a valid {} token address",
                                    chain.properties.name
                                ))
                            }
                        };
                        let chain_tokens = self.config.tokens.get_mut(chain_id).unwrap();
                        match chain_tokens.iter().position(|t| t.address == token_address) {
                            Some(x) => chain_tokens.remove(x),
                            None => {
                                return Err(format!(
                                    "Could not find token with address {:?}",
                                    token_address
                                ))
                            }
                        };
                        self.store_config_to_data_file()
                    }
                    "scan" => {
                        let (chain_type, account_address) = self.find_account_address(addr)?;
                        if chain_type != chain.chain_type {
                            return Err(format!(
                                "Account does not belong to the {} chain-type",
                                chain.chain_type.label(),
                            ));
                        }
                        let tokens_found =
                            match chain.scan_for_tokens(account_address).await.to_result()? {
                                Some(x) => x,
                                None => return Err("Could not fetch account holdings".to_string()),
                            };
                        let current_tokens = self
                            .config
                            .tokens
                            .entry(chain_id.to_string())
                            .or_insert(Vec::new());
                        let new_tokens = tokens_found
                            .into_iter()
                            .filter(|t| {
                                current_tokens
                                    .iter()
                                    .find(|c| c.address == t.address)
                                    .is_none()
                            })
                            .collect::<Vec<_>>();
                        let new_tokens_len = new_tokens.len();
                        self.config
                            .tokens
                            .get_mut(chain_id)
                            .unwrap()
                            .extend(new_tokens);
                        self.store_config_to_data_file()?;
                        if new_tokens_len == 0 {
                            println!("Found no new tokens");
                        } else {
                            println!("{} new tokens added", new_tokens_len);
                        }
                        Ok(())
                    }
                    _ => Repl::get_unknown_option_expecting_or_err(&["add", "rm", "scan"]),
                }
            }
            _ => Repl::get_bad_argument_count_err(),
        }
    }
    async fn handle_balance(&mut self, command_parts: &[&str]) -> Result<(), String> {
        match command_parts.len() {
            0 => {
                // TODO: Refactor; this is a mess, but it works
                let accounts = self
                    .config
                    .accounts
                    .iter()
                    .flat_map(|(chain_type, accounts)| {
                        accounts.iter().map(|a| (chain_type, a)).collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                let (accounts_supported, accounts_not_supported): (Vec<_>, Vec<_>) = accounts
                    .into_iter()
                    .flat_map(|(chain_type, account)| {
                        self.enabled_chains_of_type(chain_type)
                            .map(|chain| (chain.clone(), account.clone()))
                            .collect::<Vec<_>>()
                    })
                    .partition(|x| x.0.chain_type == ChainType::Ton);

                let accounts_not_supported = accounts_not_supported
                    .iter()
                    .flat_map(|(chain, account)| {
                        self.config
                            .tokens
                            .get(&chain.properties.get_id())
                            .unwrap_or(&Vec::new())
                            .iter()
                            .map(|token| (chain.clone(), token.clone(), account.clone()))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();

                let accounts_natives = self
                    .enabled_chains()
                    .flat_map(|chain| {
                        self.config
                            .accounts
                            .get(&chain.chain_type)
                            .unwrap_or(&Vec::new())
                            .to_owned()
                            .iter()
                            .map(|account| (chain.clone(), account.clone()))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();

                println!(
                    "Querying {} balances...",
                    accounts_supported.len()
                        + accounts_not_supported.len()
                        + accounts_natives.len()
                );

                let mut balances: Vec<ReplBalanceEntry> = Vec::new();

                let results_natives = stream::iter(accounts_natives.iter().enumerate())
                    .map(async |(i, (chain, account))| {
                        let task = || chain.get_native_token_balance(account.0.clone());
                        handle_retry_indexed(i, task).await
                    })
                    .buffer_unordered(20)
                    .collect::<Vec<_>>()
                    .await;

                let results_not_supported = stream::iter(accounts_not_supported.iter().enumerate())
                    .map(async |(i, (chain, token, account))| {
                        let task = || chain.get_token_balance(token, account.0.clone());
                        handle_retry_indexed(i, task).await
                    })
                    .buffer_unordered(20)
                    .collect::<Vec<_>>()
                    .await;

                let results_supported = stream::iter(accounts_supported.iter().enumerate())
                    .map(async |(i, (chain, account))| {
                        let task = async || {
                            (
                                chain
                                    .get_holdings_balance(account.0.clone())
                                    .await
                                    .to_result()
                                    .unwrap(),
                                None,
                            )
                        };
                        handle_retry_indexed(i, task).await
                    })
                    .buffer_unordered(20)
                    .collect::<Vec<_>>()
                    .await;

                for (i, balance) in results_natives {
                    let (chain, account) = &accounts_natives[i];
                    let account_label = Repl::format_account(&account);
                    if balance != BigUint::ZERO {
                        balances.push(ReplBalanceEntry {
                            account: account_label.clone(),
                            chain: chain.properties.name.clone(),
                            token: chain.properties.native_token.clone(),
                            balance_native: balance,
                            balance_usd: 0.0,
                        });
                    }
                }
                for (i, balance) in results_not_supported {
                    let (chain, token, account) = &accounts_not_supported[i];
                    let account_label = Repl::format_account(&account);
                    if balance != BigUint::ZERO {
                        balances.push(ReplBalanceEntry {
                            account: account_label.clone(),
                            chain: chain.properties.name.clone(),
                            token: token.clone().clone(),
                            balance_native: balance,
                            balance_usd: 0.0,
                        });
                    }
                }
                for (i, account_holdings) in results_supported {
                    let (chain, account) = &accounts_supported[i];
                    let account_label = Repl::format_account(&account);
                    let tokens_of_chain = self
                        .config
                        .tokens
                        .get(&chain.properties.get_id())
                        .unwrap_or(&Vec::new())
                        .to_owned();
                    for (token_address, balance) in account_holdings {
                        let token = tokens_of_chain
                            .iter()
                            .find(|t| t.address == token_address)
                            .unwrap();
                        if balance != BigUint::ZERO {
                            balances.push(ReplBalanceEntry {
                                account: account_label.clone(),
                                chain: chain.properties.name.clone(),
                                token: token.clone(),
                                balance_native: balance,
                                balance_usd: 0.0,
                            });
                        }
                    }
                }

                let tokens_to_fetch_price = balances
                    .iter()
                    .map(|b| b.token.address.clone())
                    .unique()
                    .collect::<Vec<_>>();
                println!("Fetching {} token prices...", tokens_to_fetch_price.len());
                let pairs = match dexscreener::get_pairs(tokens_to_fetch_price).await {
                    Some(x) => x,
                    None => return Err(format!("Could not fetch tokens price")),
                }
                .iter()
                .filter_map(|p| {
                    let price: f64 = p.price_usd.clone()?.parse().ok()?;
                    Some((p.base_token.address.clone(), price))
                })
                .collect::<Vec<_>>();

                for i in 0..balances.len() {
                    let balance = &mut balances[i];
                    if let Some((_, price)) =
                        pairs.iter().find(|pair| pair.0 == balance.token.address)
                    {
                        balance.balance_usd = price * balance.token.format(&balance.balance_native);
                    }
                }
                balances.sort_by(|a, b| b.balance_usd.total_cmp(&a.balance_usd));
                let relevant_balances = balances
                    .iter()
                    .filter(|balance| balance.balance_usd >= 0.1)
                    .collect::<Vec<_>>();
                let table_titles = Vec::from([
                    "Account".to_string(),
                    "Chain".to_string(),
                    "Token".to_string(),
                    "Balance".to_string(),
                    "Balance (USD)".to_string(),
                ]);
                let mut rows = relevant_balances
                    .iter()
                    .map(|balance| {
                        Vec::from([
                            balance.account.clone(),
                            balance.chain.clone(),
                            balance.token.symbol.clone(),
                            balance.token.format(&balance.balance_native).to_string(),
                            balance.balance_usd.to_string(),
                        ])
                    })
                    .collect::<Vec<_>>();
                rows.insert(0, table_titles);
                let mut t = Table::from(rows);
                t.title = "Balances".to_string();
                println!("{t}");
                println!(
                    "Holdings: {}\nBalance: {} USD",
                    relevant_balances.len(),
                    relevant_balances
                        .iter()
                        .fold(0.0, |sum, b| sum + b.balance_usd),
                );
                Ok(())
            }
            _ => Repl::get_bad_argument_count_err(),
        }
    }
    async fn handle_command(&mut self, command: &str) {
        if command.trim() == "" {
            return;
        }
        let command = command.split_whitespace().collect::<Vec<_>>();
        let command_parts = &command[1..];
        if let Err(x) = match command[0] {
            "balance" => self.handle_balance(command_parts).await,
            "token" => self.handle_token(command_parts).await,
            "chain" => self.handle_chain(command_parts),
            "account" => self.handle_account(command_parts),
            "config" => self.handle_config(command_parts),
            "help" | "?" => Ok(Self::display_help()),
            "exit" | "quit" => std::process::exit(0),
            x => Err(format!("Unknown command: {x:?}")),
        } {
            eprintln!("{x}");
        }
    }
    fn create_password(&mut self) -> Result<(), String> {
        let secret = match age::cli_common::read_secret(
            "Create password (leave empty if you don't require encryption)",
            "Password",
            Some("Confirm password"),
        ) {
            Ok(x) => Some(x),
            Err(pinentry::Error::Cancelled) => None,
            _ => return Err("Could not create password".to_string()),
        };
        self.secret = if secret.is_none() || secret.clone().unwrap().expose_secret().is_empty() {
            None
        } else {
            secret
        };
        Ok(())
    }
    fn read_password(&mut self) -> Result<(), String> {
        let pass = match age::cli_common::read_secret("Enter password", "Password", None) {
            Ok(x) => x,
            _ => return Err("Could not read password".to_string()),
        };
        self.secret = Some(pass);
        Ok(())
    }
    fn get_data_file_path() -> Result<PathBuf, String> {
        let home = match dirs::config_dir() {
            Some(x) => x,
            None => return Err("Could not find config directory".to_string()),
        };
        Ok(home.join(DATA_FILE))
    }
    fn data_file_exists() -> Result<bool, String> {
        Ok(std::fs::metadata(Repl::get_data_file_path()?).is_ok())
    }
    fn read_data_file() -> Result<Vec<u8>, String> {
        match std::fs::read(Repl::get_data_file_path()?) {
            Ok(x) => Ok(x),
            _ => return Err("Could not read data file".to_string()),
        }
    }
    fn write_data_file(contents: &[u8]) -> Result<(), String> {
        match std::fs::write(Repl::get_data_file_path()?, contents) {
            Ok(_) => Ok(()),
            _ => Err("Could not write data file".to_string()),
        }
    }
    fn read_config_from_data_file(&mut self, keep_trying: bool) -> Result<ReplConfig, String> {
        let data = Self::read_data_file()?;
        if age::Decryptor::new(data.as_slice()).is_ok() {
            let mut contents: Option<Vec<u8>> = None;
            while contents.is_none() {
                self.read_password()?;
                let identity = age::scrypt::Identity::new(self.secret.clone().unwrap());
                contents = match age::decrypt(&identity, data.as_slice()) {
                    Ok(x) => Some(x),
                    _ => {
                        let err = "Bad password, try again".to_string();
                        if keep_trying {
                            eprintln!("{err}");
                        } else {
                            return Err(err);
                        }
                        continue;
                    }
                };
            }
            match serde_json::from_slice::<ReplConfig>(contents.unwrap().as_slice()) {
                Ok(x) => Ok(x),
                _ => Err("Bad decrypted config".to_string()),
            }
        } else {
            match serde_json::from_slice::<ReplConfig>(data.as_slice()) {
                Ok(x) => Ok(x),
                _ => Err("Bad config".to_string()),
            }
        }
    }
    fn sync_rpcs(&mut self) {
        let default_chains = Self::default().chains;
        self.chains.iter_mut().find_map(|c| {
            let id = c.properties.get_id();
            if let Some(rpc) = self.config.rpcs.get(&id) {
                if c.chain_type == ChainType::Ton {
                    let mut headers = HeaderMap::new();
                    headers.insert("Authorization", format!("Bearer {rpc}").parse().unwrap());
                    c.properties.rpc_headers = headers;
                } else {
                    c.properties.rpc_url = Url::from_str(rpc).unwrap();
                }
                return Some(c);
            }
            let default_properties = &default_chains
                .iter()
                .find(|d| d.properties.get_id() == id)
                .unwrap()
                .properties;
            if c.chain_type != ChainType::Ton {
                if default_properties.rpc_url.to_string() != c.properties.rpc_url.to_string() {
                    c.properties.rpc_url = default_properties.rpc_url.clone();
                    return Some(c);
                }
                return None;
            }
            if c.properties.rpc_headers.get("Authorization").is_some() {
                c.properties.rpc_headers = HeaderMap::new();
                return Some(c);
            }
            None
        });
    }
    fn store_config_to_data_file(&mut self) -> Result<(), String> {
        let mut contents = serde_json::to_vec(&self.config).unwrap();
        if self.secret.is_some() {
            let recipient = age::scrypt::Recipient::new(self.secret.clone().unwrap());
            let encrypted_contents = match age::encrypt(&recipient, contents.as_slice()) {
                Ok(x) => x,
                _ => return Err("Could not encrypt config".to_string()),
            };
            contents = encrypted_contents;
        };
        Repl::write_data_file(contents.as_slice())?;
        self.sync_rpcs();
        Ok(())
    }
    fn startup_config(&mut self) -> Result<(), String> {
        if !Self::data_file_exists()? {
            self.create_password()?;
            return self.store_config_to_data_file();
        }
        self.config = self.read_config_from_data_file(true)?;
        self.sync_rpcs();
        Ok(())
    }
    pub async fn run(&mut self) -> Result<(), String> {
        self.startup_config()?;
        let mut rl = DefaultEditor::new().unwrap();
        let mut last_command: Option<String> = None;
        let mut interrupted = false;
        println!(
            "Welcome to the {}! Enter ? for available commands.",
            BOOK_OF_PROFITS.to_colored()
        );
        loop {
            match rl.readline("> ".to_colored().as_str()) {
                Ok(line) => {
                    if interrupted {
                        interrupted = false;
                    }
                    let mut command = line.trim();
                    if command == "!!" {
                        // Execute previous command like in a posix shell
                        command = last_command.as_deref().unwrap_or(command);
                    } else {
                        last_command = Some(line.clone());
                    }
                    self.handle_command(command).await;
                    rl.add_history_entry(command).unwrap();
                }
                Err(ReadlineError::Interrupted) => {
                    if !interrupted {
                        interrupted = true;
                        println!("(Press ^C again to exit)");
                    } else {
                        std::process::exit(0);
                    }
                }
                Err(ReadlineError::Eof) => {
                    std::process::exit(0);
                }
                _ => {}
            }
        }
    }
}

impl Default for Repl {
    fn default() -> Self {
        let sol = Vec::from([Chain::new(
            ChainType::Solana,
            "https://api.mainnet-beta.solana.com",
            "Solana",
            "SOL",
            "So11111111111111111111111111111111111111112",
            9,
        )]);
        let ton = Vec::from([Chain::new(
            ChainType::Ton,
            "https://tonapi.io/v2",
            "Ton",
            "TON",
            "0x582d872A1B094FC48F5DE31D3B73F2D9bE47def1",
            9,
        )]);
        let evm = Vec::from([
            Chain::new(
                ChainType::Evm,
                "https://eth.llamarpc.com",
                "Ethereum",
                "ETH",
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://base.llamarpc.com",
                "Base",
                "ETH",
                "0x4200000000000000000000000000000000000006",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://binance.llamarpc.com",
                "BSC",
                "BNB",
                "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://arbitrum.llamarpc.com",
                "Arbitrum",
                "ETH",
                "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://avalanche.drpc.org",
                "Avalanche",
                "AVAX",
                "0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://polygon.llamarpc.com",
                "Polygon",
                "POL",
                "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://mainnet.era.zksync.io",
                "zkSync",
                "ETH",
                "0x5AEa5775959fBC2557Cc8789bC1bf90A239D9a91",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://cronos-evm-rpc.publicnode.com",
                "Cronos",
                "CRO",
                "0x5C7F8A570d578ED84E63fdFA7b1eE72dEae1AE23",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://fantom.drpc.org",
                "Fantom",
                "FTM",
                "0x21be370D5312f44cB42ce377BC9b8a0cEF1A4C83",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://mainnet.optimism.io",
                "Optimism",
                "ETH",
                "0x4200000000000000000000000000000000000006",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://linea.drpc.org",
                "Linea",
                "ETH",
                "0xe5D7C2a44FfDDf6b295A15c148167daaAf5Cf34f",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://rpc.mantle.xyz",
                "Mantle",
                "MNT",
                "0x201EBa5CC46D216Ce6DC03F6a759e8E766e956aE",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://metis.drpc.org",
                "Metis",
                "METIS",
                "0x75cb093E4D61d2A2e65D8e0BBb01DE8d89b53481",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://core.drpc.org",
                "Core",
                "CORE",
                "0x40375C92d9FAf44d2f9db9Bd9ba41a3317a2404f",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://rpc.scroll.io",
                "Scroll",
                "ETH",
                "0x5300000000000000000000000000000000000004",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://rpc.ankr.com/iotex",
                "IoTeX",
                "IOTX",
                "0xA00744882684C3e4747faEFD68D283eA44099D03",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://forno.celo.org",
                "Celo",
                "CELO",
                "0x471EcE3750Da237f93B8E339c536989b8978a438",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://rpc.pulsechain.com",
                "PulseChain",
                "PLS",
                "0xA1077a294dDE1B09bB078844df40758a5D0f9a27",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://polygon-zkevm.drpc.org",
                "Polygon zkEVM",
                "ETH",
                "0x4F9A0e7FD2Bf6067db6994CF12E4495Df938E6e9",
                18,
            ),
            Chain::new(
                ChainType::Evm,
                "https://rpc.telos.net",
                "Telos",
                "TLOS",
                "0xB4B01216a5Bc8F1C8A33CD990A1239030E60C905",
                18,
            ),
        ]);
        Repl {
            chains: Vec::from([ton, sol, evm]).into_iter().flatten().collect(),
            config: ReplConfig::default(),
            secret: None,
        }
    }
}
