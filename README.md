# Bitcoin Transaction Services

This repository holds the source code for building the daemons and tools that parse 
bitcoin blockchain blocks into a compact database based on LNPBP-5 standard and 
provides a query API for data that can't be provided by either Bitcoin Core and 
Electrum Server (like getting transaction spending particular output, querying 
transactions by their script/miniscript code etc).

This project contains the following components:

* `txparserd`: daemon that feeds with blocks from Bitcoin database, parses them
  and stores transaction & block information in indexed database
* `txqueryd`: daemon that provides API to external clients to query transaction
  indexes
* `txlib`: library with database models and schemata used by the above daemons
* `fsparser`: a tool that parses the content of Bitcoin Core `blocks` directory
  and feeds it to the `txparserd` for initial database population with the data
* `zmqnotifier`: a service that can run together with Bitcoin Core instance
  monitoring new blocks via ZeroMQ interface and feeds them to the `txparserd`
  for maintaining the database updated
  
![Software architecture](doc/architecture.jpeg)
