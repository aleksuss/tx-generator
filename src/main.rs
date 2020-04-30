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

//! Util for hi-load streaming transactions into Exonum blockchain network.

#![warn(
    missing_debug_implementations,
    missing_docs,
    unsafe_code,
    bare_trait_objects
)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::module_name_repetitions)]

use atomic_counter::{AtomicCounter, RelaxedCounter};
use crossbeam::channel::{bounded, Sender, TryRecvError};
use exonum::merkledb::BinaryValue;
use exonum::messages::{AnyTx, Verified};
use generator::{CreateWalletGenerator, TransferGenerator, TransferGeneratorConfig};
use logger::init_custom_logger;
use reqwest::blocking::Client;
use serde_json::json;
use std::{
    ops::Deref,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime},
};
use structopt::StructOpt;

mod generator;
mod logger;

const TX_AMOUNT: usize = 10_000;
const CHANNEL_SIZE: usize = 500_000;

/// Generate hex encoded list of transactions.
#[derive(Debug, StructOpt)]
#[structopt(name = "tx-generator")]
struct Options {
    /// Service ID.
    #[structopt(short = "i", long = "service_id", help = "Service ID.")]
    service_id: u32,
    /// seed.
    #[structopt(short = "s", long = "seed", help = "Seed of random number generator.")]
    seed: u64,
    /// A transactions count.
    #[structopt(short = "c", long = "count", help = "Number of transactions")]
    count: usize,
    /// A transactions count.
    #[structopt(short = "a", long = "api", help = "Backend API")]
    api_hosts: Vec<String>,
    #[structopt(
        short = "t",
        long = "timeout",
        help = "A delay between sending transactions in microseconds"
    )]
    timeout: Option<u64>,
    #[structopt(subcommand)]
    transaction: Transaction,
}

impl Options {
    fn create_tx_generator(&self) -> Box<dyn Iterator<Item = Verified<AnyTx>>> {
        match self.transaction {
            Transaction::CreateWallet => {
                Box::new(CreateWalletGenerator::new(self.service_id, self.seed))
            }
            Transaction::Transfer {
                wallets_count,
                wallets_seed,
            } => Box::new(TransferGenerator::new(&TransferGeneratorConfig {
                service_id: self.service_id,
                seed: self.seed,
                wallets_count,
                wallets_seed,
            })),
        }
    }

    fn generator(&self, tx: &Sender<serde_json::Value>) {
        let tx_generator = self.create_tx_generator();

        for t in tx_generator.take(self.count) {
            let tx_body = json!({ "tx_body": hex::encode(t.to_bytes())});
            if let Err(e) = tx.send(tx_body) {
                log::error!("{}", e);
            }
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

fn post_transaction(
    client: &Client,
    url: &str,
    tx: &serde_json::Value,
    counter: &RelaxedCounter,
    timeout: Option<u64>,
    time: &Arc<Mutex<SystemTime>>,
) {
    let tx_count = counter.inc();
    if tx_count % TX_AMOUNT == 0 && tx_count > 0 {
        let mut time = time.lock().unwrap();
        let now = SystemTime::now();
        let delta = now.duration_since(*time).unwrap();
        *time = now;
        println!(
            "{} transactions were sent. Time: {} ms. RPS: {}",
            TX_AMOUNT,
            delta.as_millis(),
            (TX_AMOUNT * 1_000) as u128 / delta.as_millis()
        );
    }

    if let Some(timeout) = timeout {
        thread::sleep(Duration::from_micros(timeout));
    }

    log::info!("tx: {}", &tx);
    let _ = client
        .post(url)
        .json(&tx)
        .send()
        .map_err(|err| log::error!("{}", err))
        .and_then(|response| {
            log::info!("Response: {:?}", response);
            Ok(())
        });
}

fn main() {
    init_custom_logger().unwrap();
    let opts = Options::from_args();
    println!("Seed: {}. Transaction count: {}.", opts.seed, opts.count);

    let (tx, rx) = bounded::<serde_json::Value>(CHANNEL_SIZE);
    let hosts = opts.api_hosts.clone();
    let timeout = opts.timeout;

    let gen_handler = thread::spawn(move || {
        opts.generator(&tx);
    });

    let time = Arc::new(Mutex::new(SystemTime::now()));
    let counter = Arc::new(RelaxedCounter::new(0));
    let handlers = hosts.iter().map(|host| {
        let time_ref = time.clone();
        let counter_ref = counter.clone();
        let tx_url = format!("http://{}/api/explorer/v1/transactions", host);
        let client = Client::new();
        let tx_channel = rx.clone();
        thread::spawn(move || loop {
            match tx_channel.try_recv() {
                Ok(tx) => post_transaction(
                    &client,
                    &tx_url,
                    &tx,
                    counter_ref.deref(),
                    timeout,
                    &time_ref,
                ),
                Err(e) => match e {
                    TryRecvError::Empty => log::warn!("No messages"),
                    TryRecvError::Disconnected => break,
                },
            }
        })
    });

    let _ = gen_handler.join();
    for handler in handlers {
        let _ = handler.join();
    }
}
