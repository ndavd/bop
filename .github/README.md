# 📒📈 Book of Profits

`book-of-profits` is a multichain portfolio tracker in
[REPL](https://en.wikipedia.org/wiki/Read%E2%80%93eval%E2%80%93print_loop) form.

It is private - All data is locally encrypted.

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

See [features](https://github.com/ndavd/bop/#features).\
See
[supported blockchains](https://github.com/ndavd/bop/#supported-blockchains).

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
| `⚠️` Not supported in EVM | Scan for token holdings in account and automatically add them                        |
| `❌`                      | Show balance by chain                                                                |
| `❌`                      | Show balance by account                                                              |
| `❌`                      | Automatically prune low liquidity tokens                                             |
| `❌`                      | Cache balances in order to display them in other views without refetching everything |
| `❌`                      | Display spinner when loading                                                         |
| `❌`                      | Show total balance of a single token                                                 |

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

## Contributing

Contributions are very welcome! Those being pull requests, issues or feature
requests.
