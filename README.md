# BP Node

Bitcoin blockchain indexing and notification node. It may be considered electrum
server replacement, which is faster, provides modern API (supporting wallet
descriptors and miniscript, LN-specific queries, client-side-validation tech
like RGB, modern RPC subscribe interfaces with ZMQ etc). In the future it is 
planned to upgrade the node into a fully-validating bitcoin node by using
`libbitcoinconsensus` library for validating blocks.

The node was originally designed and implemented by 
[Dr Maxim Orlovsky](https://github.com/dr-orlovsky) as a part of 
[LNP/BP Standards Association](https://github.com/LNP-BP) effort for 
building the foundation for LNP/BP layer 2 and 3 bitcoin application ecosystem.
It is based on other LNP/BP projects such as [BP Core Lib], [Descriptor Wallet],
[Storm Storage] and can be easily integrated with the rest of LNP/BP nodes like
[LNP Node], [RGB Node], [Storm Node].

## Node services

The node organized as a set of daemons or threads, interacting via ZMQ-based
API with each other and with RPC clients. It leverages microservice architecture
from [`microservices`] crate.

The node provides following set of services:
- `bpd`: main service providing clients with RPC request/reply API and managing
  life cycle of the rest of the node services (launching, terminating, brokering
  inter-service communications).
- `blockd`: service parsing and (in the future, using `libbitcoinconsensus`) 
  validating bitcoin blocks, storing them in the database (provided by `stored`
  service from [Storm Storage] library). It also connects to bitcoin core node
  to monitor the incoming new blocks and through `bpd` node clients when a new
  valid block has been parsed.
- `mempoold`: manages mempool transactions.
- `walletd`: service instantiated for each wallet client. It knows about wallet
  descriptor, monitors new mempool and mined transactions and notifies 
  subscribed client about their changes.
- `watchd`: watchtower service for lightning network and RGB.
- `signd`: service for signing bitcoin transactions and working with PSBTs. Used
  by wallets, lightning node etc.

Alongside these services, in the future it is planned to use lightning network
(Bifrost subnetwork) through [LNP Node] to propagate bitcoin blocks in encrypted
form to other BP Nodes (instead of using legacy bitcoin wire protocol).

## RPC and client tools

The repository also contains a BP Node RPC library (`bp_rpc` crate in 
[`rpc`](./rpc) directory) and command-line tool (`bp-cli` crate in 
[`cli`](./cli) directory) for querying/working with the node.

[`microservices`]: https://github.com/Internet2-WG/rust-microservices
[BP Core Lib]: https://github.com/BP-WG/bp-core
[Descriptor Wallet]: https://github.com/BP-WG/descriptor-wallet
[Storm Storage]: https://github.com/Storm-WG/storm-stored
[LNP Node]: https://github.com/LNP-WG/lnp-node
[RGB Node]: https://github.com/RGB-WG/rgb-node
[Storm Node]: https://github.com/Storm-WG/storm-node
