# BP Node

Bitcoin blockchain indexing and wallet notification node. It may be considered
electrum server replacement, which is faster, provides modern API (supporting
wallet descriptors, LN-specific queries, client-side-validation tech like RGB,
modern RPC subscribe interfaces).

The node was originally designed and implemented by 
[Dr Maxim Orlovsky](https://github.com/dr-orlovsky) as a part of 
[LNP/BP Standards Association](https://github.com/LNP-BP) effort for 
building the foundation for LNP/BP layer 2 and 3 bitcoin application ecosystem.
It is based on other LNP/BP projects such as [BP Core Lib], [BP Standard Lib],
[BP Wallet Lib] and can be easily integrated with the rest of LNP/BP nodes like
[LNP Node], [RGB Node], [Storm Node].

In the future it is planned to upgrade the node into a fully-validating bitcoin
node by using [`bitcoinkernel`] library for validating blocks.

## Node services

The node organized as a set of threads, interacting via crossbeam API with each
other -- and via [Strict RPC] with connecting clients. It leverages microservice
architecture with [I/O Reactor] (used instead of async), authentication with
self-sovereign identities ([SSI]) and end-to-end encryption from [Cyphernet]
crates.

The node provides following set of services:
- `bpd`: main service providing clients with RPC request/reply API and managing
  life cycle of the rest of the node services (launching, terminating, brokering
  inter-service communications).
- `blockd`: service parsing and (in the future, using `bitcoinkernel`) 
  validating bitcoin blocks, storing them in the database.
- `mempoold`: manages mempool transactions.
- `walletd`: service instantiated for each wallet client. It knows about wallet
  descriptor, monitors new mempool and mined transactions and notifies 
  subscribed client about their changes.
- `watchd`: watchtower service for lightning network and RGB.
- `listend`: interface listening to Bitcoin Core or an external indexing service
  (like Esplora), if one is used.

## RPC and client tools

The repository also contains a BP Node RPC library (`bp-rpc` crate in 
[`rpc`](./rpc) directory) and command-line tool (`bp-cli` crate in 
[`cli`](./client) directory) for querying/working with the node.

[`bitcoinkernel`]: https://github.com/bitcoin/bitcoin/issues/27587
[BP Core Lib]: https://github.com/BP-WG/bp-core
[BP Standard Lib]: https://github.com/BP-WG/bp-std
[BP Wallet Lib]: https://github.com/BP-WG/bp-wallet
[LNP Node]: https://github.com/LNP-WG/lnp-node
[RGB Node]: https://github.com/RGB-WG/rgb-node
[Storm Node]: https://github.com/Storm-WG/storm-node

[SSI]: https://github.com/LNP-BP/SSI
[Cyphernet]: https://github.com/cyphernet-labs
[Strict RPC]: https://github.com/strict-types/strict-rpc
[I/O Reactor]: https://github.com/rust-amplify/io-reactor
