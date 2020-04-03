# bpd: Bitcoin protocol daemon written in Rust

`bpd` replaces Bitcoin Core in parts of it's outdated JSON API and
different indexing servers (like Electrum) for an efficient and extended
queries agains bitcoin blockchain (see the drawing below).

The daemon is made with focuse on:
* non-blocking/async IO and APIs
* ZMQ APIs for the clients
* efficient indexing with
  [LNPBP-5 standard](https://github.com/LNP-BP/lnpbps/blob/master/lnpbp-0005.md)
* native miniscript support
* arbitrary complex queries agains bitcoin transactions and their parts
* layer 2/3 protocols in mind
* native support for the new rust 
  [Lightning network node](https://github.com/LNP-BP/lnpbps) `lnpd`

The repository also contains a tool for building initial blockchain index.

NB: This is (not yet) a full validating node!

![Software architecture](doc/architecture.jpeg)
