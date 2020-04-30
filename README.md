# Transactions generator

![Rust](https://github.com/aleksuss/tx-generator/workflows/Rust/badge.svg?branch=master)

This utility used to emulate hi-load stream of transactions to
Exonum Blockchain network with `cryptocurrency-advanced` service inside.

### Usage example:

#### Install:
```bash
cargo install --git https://github.com/aleksuss/tx-generator.git
```

#### Create wallets
```bash
tx-generator --service_id 1024 --api node_hostname1:port1 --api node_hostnameN:portN --count 10000 --seed 1 --timeout 10 create-wallets
```

- `--servie_id` - service identifier.
- `--api` - IP address or hostname and port of the node. This parameter is repeatable.
- `--count` - number of transactions.
- `--seed` - seed for key pairs generating.
- `--timeout` is optional parameter. It sets a timeout between sending transactions
in microseconds.

#### Transfer funds between created wallets

```bash
tx-generator --service_id 1024 --api node_hostname1:port1 --api node_hostnameN:portN --count 20000 --seed 1000 transfer --wallets-count 10000 --wallets-seed 1
```

- `--wallets-count` should be equal to `--count` from `create_wallets` stage.
- `--wallets-seed` should be equal to `--seed` from `create_wallets` stage.

You should pass node's hostnames which should receive transactions.

