// Copyright 2019 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use exonum::messages::to_hex_string;
use generator::{CreateWalletGenerator, TransferGenerator, TransferGeneratorConfig};
use lockfree::channel::spmc::{create, Sender};
use log::{error, info};
use logger::init_custom_logger;
use serde_json::json;
use std::thread;
use structopt::StructOpt;

mod generator;
mod logger;

/// Generate hex encoded list of transactions.
#[derive(Debug, StructOpt)]
#[structopt(name = "tx-generator")]
struct Options {
    /// seed.
    #[structopt(short = "s", long = "seed", help = "Seed of random number generator.")]
    seed: u64,
    /// A transactions count.
    #[structopt(short = "c", long = "count", help = "Number of transactions")]
    count: usize,
    /// A transactions count.
    #[structopt(short = "a", long = "api", help = "Backend API")]
    api_hosts: Vec<String>,
    #[structopt(subcommand)]
    transaction: Transaction,
}

impl Options {
    pub fn generator(&self, tx: &mut Sender<serde_json::Value>) {
        let gen = match self.transaction {
            Transaction::CreateWallet => {
                let gen = CreateWalletGenerator::new(self.seed)
                    .map(|tx| json!({ "tx_body": to_hex_string(&tx) }));
                Box::new(gen) as Box<dyn Iterator<Item = serde_json::Value>>
            }
            Transaction::Transfer {
                wallets_count,
                wallets_seed,
            } => {
                let gen = TransferGenerator::new(TransferGeneratorConfig {
                    seed: self.seed,
                    wallets_count,
                    wallets_seed,
                })
                .map(|tx| json!({ "tx_body": to_hex_string(&tx) }));
                Box::new(gen) as Box<dyn Iterator<Item = serde_json::Value>>
            }
        };
        for t in gen.take(self.count as usize) {
            let _ = tx.send(t).expect("Couldn't send message");
        }
    }
}

#[derive(Debug, StructOpt)]
enum Transaction {
    /// Generate create wallet transactions
    #[structopt(name = "create-wallets")]
    CreateWallet,
    /// Generate transfer transactions
    #[structopt(name = "transfer")]
    Transfer {
        #[structopt(long = "wallets-count", help = "Number of wallets")]
        wallets_count: usize,
        #[structopt(long = "wallets-seed", help = "Wallets seed")]
        wallets_seed: u64,
    },
}

fn post_transaction(client: &reqwest::Client, url: &str, tx: serde_json::Value) {
    info!("tx: {}", &tx);
    let _ = client
        .post(url)
        .json(&tx)
        .send()
        .map_err(|err| error!("{}", err))
        .and_then(|response| {
            info!("Response: {:?}", response);
            Ok(())
        });
}

fn main() {
    init_custom_logger().unwrap();
    let opts = Options::from_args();
    println!("Seed: {}. Transaction count: {}.", opts.seed, opts.count);

    let (mut tx, rx) = create::<serde_json::Value>();
    let hosts = opts.api_hosts.clone();

    let gen_handler = thread::spawn(move || {
        opts.generator(&mut tx);
    });

    let mut handlers = Vec::new();

    for host in hosts {
        let tx_url = format!("http://{}/api/explorer/v1/transactions", host);
        let client = reqwest::Client::new();
        let tx_channel = rx.clone();
        handlers.push(thread::spawn(move || {
            while let Ok(tx) = tx_channel.recv() {
                post_transaction(&client, &tx_url, tx);
            }
        }));
    }

    let _ = gen_handler.join();
    for handler in handlers {
        let _ = handler.join();
    }
}
