# Transaction generator

This utility used to emulate hi-load stream of transactions to
Exonum Blockchain network with `cryptocurrency-advanced` service inside.

### Usage example:

#### Install:
```bash
cargo install --git https://github.com/aleksuss/tx-generator.git
```

#### Create wallets
```bash
tx-generator --api node_hostname1:port1 --api node_hostnameN:portN --count 10000 --seed 1 --timeout 10 create-wallets
```

#### Transfer funds
```bash
tx-generator --api node_hostname1:port1 --api node_hostnameN:portN --count 20000 --seed 1000 transfer --wallets-count 10000 --wallets-seed 1
```
`--wallets-count` should be equal to `--count` and `--wallets-seed` to
`--seed` respectively from `create-wallets` subcommand.

`--timeout` is optional parameter. It sets a timeout between sending
transactions in microseconds.

You should pass node's hostnames which should receive transactions.

