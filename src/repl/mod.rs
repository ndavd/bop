mod data_file;
mod default;

use std::{collections::HashMap, fmt::Display, str::FromStr};

use age::secrecy::{ExposeSecret, SecretString};
use data_file::{data_file_exists, read_data_file, write_data_file};
use futures::{stream, StreamExt};
use itertools::Itertools;
use num_bigint::BigUint;
use reqwest::{header::HeaderMap, Url};
use rustyline::{error::ReadlineError, DefaultEditor};
use serde::{Deserialize, Serialize};

use crate::{
    chain::{
        chain_type::{ChainType, CHAIN_TYPES},
        token::Token,
        Chain, ChainOps,
    },
    dexscreener,
    utils::{
        float::ExtendFloat, retry::handle_retry_indexed, spinner::Spinner, table::Table,
        text::StylizedText,
    },
};

static BOOK_OF_PROFITS: &str = "Book of Profits";

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct ReplConfig {
    /// Vec of chain-type, account address and optional alias
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    accounts: Vec<(ChainType, String, Option<String>)>,
    /// Vec of chain-id and token
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tokens: Vec<(String, Token)>,
    /// Map of chain-id to custom rpc
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    rpcs: HashMap<String, String>,
    /// Map of chain-id to enabled
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    chains_enabled: HashMap<String, bool>,
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
    spinner: Spinner,
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
    fn is_chain_enabled(&self, chain: &Chain) -> bool {
        let chain_id = chain.properties.get_id();
        match self.config.chains_enabled.get(&chain_id) {
            Some(x) => *x,
            None => true,
        }
    }
    fn enabled_chains(&self) -> impl Iterator<Item = &Chain> {
        self.chains.iter().filter(|c| self.is_chain_enabled(c))
    }
    fn enabled_chains_of_type<'a>(
        &'a self,
        chain_type: &'a ChainType,
    ) -> impl Iterator<Item = &'a Chain> + 'a {
        self.chains
            .iter()
            .filter(move |c| c.chain_type == *chain_type && self.is_chain_enabled(c))
    }
    fn chains_of_type<'a>(
        &'a self,
        chain_type: &'a ChainType,
    ) -> impl Iterator<Item = &'a Chain> + 'a {
        self.chains
            .iter()
            .filter(move |c| c.chain_type == *chain_type)
    }
    fn accounts_of_type<'a>(
        &'a self,
        chain_type: &'a ChainType,
    ) -> impl Iterator<Item = &'a (ChainType, String, Option<String>)> + 'a {
        self.config
            .accounts
            .iter()
            .filter(move |(c_type, _, _)| c_type == chain_type)
    }
    fn tokens_of_chain<'a>(
        &'a self,
        chain: &'a Chain,
    ) -> impl Iterator<Item = &'a (String, Token)> {
        let chain_id = chain.properties.get_id();
        self.config
            .tokens
            .iter()
            .filter(move |(c_id, _)| *c_id == chain_id)
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
    fn find_account_address(&self, account: &str) -> Result<(&ChainType, &String), String> {
        match self
            .config
            .accounts
            .iter()
            .find_map(|(chain_type, address, alias)| {
                (account == address || alias.clone().is_some_and(|alias| alias == account))
                    .then(|| (chain_type, address))
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
    fn format_account(address: &String, alias: &Option<String>) -> String {
        if alias.is_some() {
            return alias.clone().unwrap();
        }
        Self::format_address(address)
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
                                self.is_chain_enabled(c).to_string(),
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
                    if self.is_chain_enabled(chain) {
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
                        let new_state = !self.is_chain_enabled(chain);
                        self.config
                            .chains_enabled
                            .insert(chain.properties.get_id(), new_state);
                        self.store_config_to_data_file()?;
                        println!(
                            "{chain_name} chain set to {}",
                            if new_state { "enabled" } else { "disabled" }
                        );
                        Ok(())
                    }
                    "toggle-all" => {
                        let chain_type = ChainType::from_str(arg)?;
                        let new_state = !self
                            .chains_of_type(&chain_type)
                            .all(|c| self.is_chain_enabled(c));
                        self.config.chains_enabled.extend(
                            self.chains_of_type(&chain_type)
                                .map(|c| (c.properties.get_id(), new_state))
                                .collect::<Vec<_>>(),
                        );
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
                    let mut rows = self
                        .accounts_of_type(chain_type)
                        .map(|(_, address, alias)| {
                            Vec::from([
                                Repl::format_address(address),
                                address.to_string(),
                                alias.clone().unwrap_or("-".to_string()),
                            ])
                        })
                        .collect::<Vec<_>>();
                    if rows.len() == 0 {
                        continue;
                    }
                    rows.insert(
                        0,
                        Vec::from([
                            "Short address".to_string(),
                            "Full address".to_string(),
                            "Alias".to_string(),
                        ]),
                    );
                    let mut t = Table::from(rows);
                    t.title = format!("{} accounts", chain_type.label());
                    println!("{t}");
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
                    .iter()
                    .position(|a| a.0 == *chain_type && a.1 == *address)
                    .unwrap();
                self.config.accounts.remove(index);
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
                self.config.accounts.push((chain_type, address, alias));
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
                for chain in &self.chains {
                    let mut tokens = self
                        .tokens_of_chain(chain)
                        .map(|(_, t)| {
                            Vec::from([t.symbol.clone(), t.address.clone(), t.decimals.to_string()])
                        })
                        .collect::<Vec<_>>();
                    if tokens.len() == 0 {
                        continue;
                    }
                    tokens.insert(
                        0,
                        Vec::from([
                            "Symbol".to_string(),
                            "Address".to_string(),
                            "Decimals".to_string(),
                        ]),
                    );
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
                        let token = match Token::new(&token_address, &chain).await {
                            Some(x) => x,
                            None => return Err("Could not fetch token info".to_string()),
                        };
                        if self
                            .tokens_of_chain(chain)
                            .find(|(_, t)| t.address == token.address)
                            .is_some()
                        {
                            return Err("Token already added".to_string());
                        }
                        self.config.tokens.push((chain_id.to_string(), token));
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
                        match self
                            .config
                            .tokens
                            .iter()
                            .position(|(c_id, t)| c_id == chain_id && t.address == token_address)
                        {
                            Some(x) => self.config.tokens.remove(x),
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
                        if *chain_type != chain.chain_type {
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
                        let new_tokens = tokens_found
                            .into_iter()
                            .filter_map(|t| {
                                self.tokens_of_chain(chain)
                                    .find(|(_, ct)| ct.address == t.address)
                                    .is_none()
                                    .then(|| (chain_id.to_string(), t))
                            })
                            .collect::<Vec<_>>();
                        let new_tokens_len = new_tokens.len();
                        self.config.tokens.extend(new_tokens);
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
                // TODO: Remove the partition part by making use of the `SupportOption`
                // and stream everything together

                // Partition between the accounts that support `get_holdings_balance` and those
                // that do not
                let (accounts_supported, accounts_not_supported): (Vec<_>, Vec<_>) = self
                    .config
                    .accounts
                    .iter()
                    .flat_map(|(chain_type, address, alias)| {
                        self.enabled_chains_of_type(&chain_type)
                            .map(move |chain| (chain, address, alias))
                    })
                    .partition(|(chain, _, _)| chain.chain_type == ChainType::Ton);

                let accounts_not_supported = accounts_not_supported
                    .iter()
                    .flat_map(|(chain, address, alias)| {
                        self.tokens_of_chain(&chain)
                            .map(move |(_, token)| (chain, token.clone(), address, alias))
                    })
                    .collect::<Vec<_>>();

                let accounts_natives = self
                    .enabled_chains()
                    .flat_map(|chain| {
                        self.accounts_of_type(&chain.chain_type)
                            .map(move |(_, address, alias)| (chain, address, alias))
                    })
                    .collect::<Vec<_>>();

                let total_balances = accounts_supported.len()
                    + accounts_not_supported.len()
                    + accounts_natives.len();

                self.spinner.set_total(total_balances);
                self.spinner.start(Some("Querying balances..."));

                let mut balances: Vec<ReplBalanceEntry> = Vec::new();

                let results_natives = stream::iter(accounts_natives.iter().enumerate())
                    .map(async |(i, (chain, address, _))| {
                        let task = || chain.get_native_token_balance(address);
                        let result = handle_retry_indexed(i, task).await;
                        self.spinner.inc_progress();
                        result
                    })
                    .buffer_unordered(20)
                    .collect::<Vec<_>>()
                    .await;

                let results_not_supported = stream::iter(accounts_not_supported.iter().enumerate())
                    .map(async |(i, (chain, token, address, _))| {
                        let task = || chain.get_token_balance(token, address);
                        let result = handle_retry_indexed(i, task).await;
                        self.spinner.inc_progress();
                        result
                    })
                    .buffer_unordered(20)
                    .collect::<Vec<_>>()
                    .await;

                let results_supported = stream::iter(accounts_supported.iter().enumerate())
                    .map(async |(i, (chain, address, _))| {
                        let task = async || {
                            (
                                chain
                                    .get_holdings_balance(address)
                                    .await
                                    .to_result()
                                    .unwrap(),
                                None,
                            )
                        };
                        let result = handle_retry_indexed(i, task).await;
                        self.spinner.inc_progress();
                        result
                    })
                    .buffer_unordered(20)
                    .collect::<Vec<_>>()
                    .await;

                self.spinner.stop();

                balances.extend(results_natives.iter().filter_map(|(i, balance)| {
                    let (chain, address, alias) = &accounts_natives[*i];
                    let account_label = Repl::format_account(address, alias);
                    (*balance != BigUint::ZERO).then(|| ReplBalanceEntry {
                        account: account_label.clone(),
                        chain: chain.properties.name.clone(),
                        token: chain.properties.native_token.clone(),
                        balance_native: balance.clone(),
                        balance_usd: 0.0,
                    })
                }));

                balances.extend(results_not_supported.iter().filter_map(|(i, balance)| {
                    let (chain, token, address, alias) = &accounts_not_supported[*i];
                    let account_label = Repl::format_account(address, alias);
                    (*balance != BigUint::ZERO).then(|| ReplBalanceEntry {
                        account: account_label.clone(),
                        chain: chain.properties.name.clone(),
                        token: token.clone(),
                        balance_native: balance.clone(),
                        balance_usd: 0.0,
                    })
                }));

                balances.extend(results_supported.iter().flat_map(|(i, account_holdings)| {
                    let (chain, address, alias) = &accounts_supported[*i];
                    let account_label = Repl::format_account(address, alias);
                    let mut tokens_of_chain = self.tokens_of_chain(chain);
                    account_holdings
                        .iter()
                        .filter_map(move |(token_address, balance)| {
                            let (_, token) = tokens_of_chain
                                .find(|(_, t)| t.address == *token_address)
                                .unwrap();
                            (*balance != BigUint::ZERO).then(|| ReplBalanceEntry {
                                account: account_label.clone(),
                                chain: chain.properties.name.clone(),
                                token: token.clone(),
                                balance_native: balance.clone(),
                                balance_usd: 0.0,
                            })
                        })
                }));

                let tokens_to_fetch_price = balances
                    .iter()
                    .map(|b| b.token.address.as_str())
                    .unique()
                    .collect::<Vec<_>>();

                self.spinner.set_total(tokens_to_fetch_price.len());
                self.spinner.start(Some("Fetching token prices..."));

                let pairs = match dexscreener::pairs::get_pairs_with_progress(
                    tokens_to_fetch_price,
                    Some(|| {
                        self.spinner.inc_progress();
                    }),
                )
                .await
                {
                    Some(x) => x,
                    None => return Err(format!("Could not fetch tokens price")),
                }
                .iter()
                .filter_map(|p| {
                    let price: f64 = p.price_usd.clone()?.parse().ok()?;
                    Some((p.base_token.address.clone(), price))
                })
                .collect::<Vec<_>>();

                self.spinner.stop();

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
                    .filter(|balance| balance.balance_usd >= 0.01)
                    .collect::<Vec<_>>();
                let mut rows = relevant_balances
                    .iter()
                    .map(|balance| {
                        Vec::from([
                            balance.account.clone(),
                            balance.chain.clone(),
                            balance.token.symbol.clone(),
                            balance.token.format(&balance.balance_native).to_string(),
                            balance.balance_usd.round_to_fixed_string(2),
                        ])
                    })
                    .collect::<Vec<_>>();
                rows.insert(
                    0,
                    Vec::from([
                        "Account".to_string(),
                        "Chain".to_string(),
                        "Token".to_string(),
                        "Balance".to_string(),
                        "Balance (USD)".to_string(),
                    ]),
                );
                let mut t = Table::from(rows);
                t.title = "Balances".to_string();
                println!("{t}");
                println!(
                    "Holdings: {}\nBalance: {} USD",
                    relevant_balances.len(),
                    relevant_balances
                        .iter()
                        .fold(0.0, |sum, b| sum + b.balance_usd)
                        .round_to_fixed_string(2),
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
    fn read_config_from_data_file(&mut self, keep_trying: bool) -> Result<ReplConfig, String> {
        let data = read_data_file()?;
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
        let _ = self.chains.iter_mut().filter_map(|c| {
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
        }).collect::<Vec<_>>();
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
        write_data_file(contents.as_slice())?;
        self.sync_rpcs();
        Ok(())
    }
    fn startup_config(&mut self) -> Result<(), String> {
        if !data_file_exists()? {
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
