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
#![allow(
    // Next `cast_*` lints don't give alternatives.
    clippy::cast_possible_wrap, clippy::cast_possible_truncation, clippy::cast_sign_loss,
    // Next lints produce too much noise/false positives.
    clippy::module_name_repetitions, clippy::similar_names, clippy::must_use_candidate,
    clippy::pub_enum_variant_names,
    // '... may panic' lints.
    clippy::indexing_slicing,
    // Too much work to fix.
    clippy::missing_errors_doc, clippy::missing_const_for_fn
)]

use atomic_counter::{AtomicCounter, RelaxedCounter};
use crossbeam::channel::{bounded, Sender, TryRecvError};
use exonum::merkledb::BinaryValue;
use generator::{CreateWalletGenerator, TransferGenerator, TransferGeneratorConfig};
use hex::encode;
use log::{error, info, warn};
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
    fn generator(&self, tx: &Sender<serde_json::Value>) {
        let gen = match self.transaction {
            Transaction::CreateWallet => {
                let gen = CreateWalletGenerator::new(self.service_id, self.seed)
                    .map(|tx| json!({ "tx_body": encode(tx.to_bytes()) }));
                Box::new(gen) as Box<dyn Iterator<Item = serde_json::Value>>
            }
            Transaction::Transfer {
                wallets_count,
                wallets_seed,
            } => {
                let gen = TransferGenerator::new(&TransferGeneratorConfig {
                    service_id: self.service_id,
                    seed: self.seed,
                    wallets_count,
                    wallets_seed,
                })
                .map(|tx| json!({ "tx_body": encode(tx.to_bytes()) }));
                Box::new(gen) as Box<dyn Iterator<Item = serde_json::Value>>
            }
        };
        for t in gen.take(self.count as usize) {
            if let Err(e) = tx.send(t) {
                error!("{}", e);
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

    let (tx, rx) = bounded::<serde_json::Value>(CHANNEL_SIZE);
    let hosts = opts.api_hosts.clone();
    let timeout = opts.timeout;

    let gen_handler = thread::spawn(move || {
        opts.generator(&tx);
    });

    let time = Arc::new(Mutex::new(SystemTime::now()));
    let counter = Arc::new(RelaxedCounter::new(0));
    let mut handlers = Vec::new();

    for host in hosts {
        let time_ref = time.clone();
        let counter_ref = counter.clone();
        let tx_url = format!("http://{}/api/explorer/v1/transactions", host);
        let client = Client::new();
        let tx_channel = rx.clone();
        handlers.push(thread::spawn(move || loop {
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
                    TryRecvError::Empty => warn!("No messages"),
                    TryRecvError::Disconnected => break,
                },
            }
        }));
    }

    let _ = gen_handler.join();
    for handler in handlers {
        let _ = handler.join();
    }
}
