# 📒📈 Book of Profits

<img alt="Crates.io Version" src="https://img.shields.io/crates/v/book-of-profits?style=flat-square">

`book-of-profits` is a multichain portfolio tracker in
[REPL](https://en.wikipedia.org/wiki/Read%E2%80%93eval%E2%80%93print_loop) form.

It is private - All data is locally encrypted.

<img src='https://raw.githubusercontent.com/ndavd/bop/main/.github/demo.gif' />

It features minimal and privacy preserving data fetching.\
External requests made by `book-of-profits` can be separated into two
categories:

- RPC calls: Used to query the blockchain
- Dexscreener API calls: Used to get the current price of tokens

If you're not content with the performance or privacy of the default RPC, you
can change it to one of your liking.

> [!NOTE]
> It is highly recommended to use custom RPCs, since the public ones often have
> severe rate limits.

See [features](https://github.com/ndavd/bop?tab=readme-ov-file#features).\
See
[supported blockchains](https://github.com/ndavd/bop?tab=readme-ov-file#supported-blockchains).\
See [faq](https://github.com/ndavd/bop?tab=readme-ov-file#faq).

## Installation

### Download the pre-built binaries

Pre-built binaries for Windows, Linux, MacOS can be found in the
[releases](https://github.com/ndavd/bop/releases) page.

### Install from crates.io

Make sure you have [cargo](https://doc.rust-lang.org/stable/cargo/) installed.

```
cargo install book-of-profits
```

### Install from source

```
git clone https://github.com/ndavd/bop
cd bop
cargo install --path .
```

### Uninstall

```
cargo uninstall book-of-profits
```

## Features

`✅`: Feature is supported on all chains\
`⚠️`: Feature is partially supported\
`❌`: Feature is planned but not supported yet

| Support                   | Feature                                                                              |
| ------------------------- | ------------------------------------------------------------------------------------ |
| `✅`                      | Password encryption                                                                  |
| `✅`                      | Change chain RPC                                                                     |
| `✅`                      | Enable or disable chain                                                              |
| `✅`                      | Add account to track and optionally set an alias                                     |
| `✅`                      | Manually add new token just by specifying chain and address                          |
| `✅`                      | Show global balance                                                                  |
| `✅`                      | Export raw configuration in plaintext                                                |
| `✅`                      | Display spinner when loading                                                         |
| `⚠️` Not supported in EVM | Scan for token holdings in account and automatically add them                        |
| `❌`                      | Fallback RPCs                                                                        |
| `❌`                      | Show balance by chain                                                                |
| `❌`                      | Show balance by account                                                              |
| `❌`                      | Automatically prune low liquidity tokens                                             |
| `❌`                      | Cache balances in order to display them in other views without refetching everything |
| `❌`                      | Show total balance of a single token                                                 |
| `❌`                      | Web client                                                                           |
| `❌`                      | Centralized exchanges support                                                        |

## Supported Blockchains

### Solana

- Solana

### Ton

- Ton

### EVM

- Ethereum
- Base
- BSC
- Arbitrum
- Avalanche
- Polygon
- zkSync
- Cronos
- Fantom
- Optimism
- Linea
- Mantle
- Metis
- Core
- Scroll
- IoTeX
- Celo
- PulseChain
- Polygon zkEVM
- Telos

## FAQ

#### Q: Where does it store the data?

All data is stored in the configuration file `.bop-data`, which is stored in
user's config directory:

- For Linux, that's `$XDG_CONFIG_HOME` or `$HOME/.config`
  - Example `/home/alice/.config/.bop-data`
- For MacOS, that's `$HOME/Library/Application Support`
  - Example `/Users/Alice/Library/Application Support/.bop-data`
- For Windows, that's `{FOLDERID_RoamingAppData}`
  - Example `C:\Users\Alice\AppData\Roaming\.bop-data`

> [!NOTE]
> Keep in mind that if you didn't set a password its contents are not encrypted.

#### Q: Why a REPL?

I wanted this to be primarly a terminal application and a REPL allows the
decrypted data to be loaded into memory once, enabling users to execute multiple
commands while only needing to input their password once, and also makes the
process of updating the data and re-encrypting it trivial.

Also because I'm a fan of [chisel](https://book.getfoundry.sh/reference/chisel).

#### Q: I'm not a terminal user. Is this getting a client?

Building a web client is a planned feature. It should not be hard to compile the
core components into WASM and make a client side application out of them.

## Contributing

Contributions are very welcome! Those being pull requests, issues or feature
requests.
