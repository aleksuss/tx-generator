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

use byteorder::{ByteOrder, LittleEndian};
use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;

use exonum::crypto::{gen_keypair_from_seed, PublicKey, SecretKey, Seed};
use exonum::messages::{AnyTx, Verified};
use exonum::runtime::rust::Transaction;
use exonum_cryptocurrency_advanced::transactions::{CreateWallet, Transfer};

#[derive(Clone)]
pub struct KeypairGenerator {
    seed: u64,
}

impl KeypairGenerator {
    pub fn new(seed: u64) -> Self {
        KeypairGenerator { seed }
    }
}

impl Iterator for KeypairGenerator {
    type Item = (PublicKey, SecretKey);

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = [0u8; 32];
        LittleEndian::write_u64(&mut buf, self.seed);
        self.seed = self.seed.overflowing_add(1).0;
        Some(gen_keypair_from_seed(&Seed::new(buf)))
    }
}

pub struct CreateWalletGenerator {
    service_id: u32,
    generator: KeypairGenerator,
}

impl CreateWalletGenerator {
    pub fn new(service_id: u32, seed: u64) -> Self {
        Self {
            service_id,
            generator: KeypairGenerator::new(seed),
        }
    }
}

impl Iterator for CreateWalletGenerator {
    type Item = Verified<AnyTx>;

    fn next(&mut self) -> Option<Self::Item> {
        let (pk, sk) = self.generator.next().unwrap();
        Some(
            CreateWallet {
                name: pk.to_string(),
            }
            .sign(self.service_id, pk, &sk),
        )
    }
}

pub struct TransferGeneratorConfig {
    pub service_id: u32,
    pub seed: u64,
    pub wallets_seed: u64,
    pub wallets_count: usize,
}

#[derive(Clone)]
pub struct TransferGenerator {
    service_id: u32,
    seed: u64,
    wallets_count: usize,
    rand: XorShiftRng,
}

impl TransferGenerator {
    pub fn new(conf: TransferGeneratorConfig) -> Self {
        assert!(conf.wallets_count > 1);

        let mut buf = [0; 16];
        LittleEndian::write_u64(&mut buf, conf.seed);
        let rand = XorShiftRng::from_seed(buf);

        Self {
            service_id: conf.service_id,
            seed: conf.wallets_seed,
            wallets_count: conf.wallets_count,
            rand,
        }
    }

    fn gen_keypair(&self, offset: u64) -> (PublicKey, SecretKey) {
        let mut buf = [0u8; 32];
        LittleEndian::write_u64(&mut buf, self.seed + offset);
        gen_keypair_from_seed(&Seed::new(buf))
    }

    pub fn random_owner(&mut self) -> usize {
        self.rand.gen_range(0, self.wallets_count)
    }
}

impl Iterator for TransferGenerator {
    type Item = Verified<AnyTx>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let from = self.random_owner();
            let to = self.random_owner();

            if from == to {
                continue;
            }

            let (pk, sk) = self.gen_keypair(from as u64);
            let to = self.gen_keypair(to as u64).0;
            let seed = self.rand.gen();
            let amount = self.rand.gen_range(1, 10);
            return Some(Transfer { to, amount, seed }.sign(self.service_id, pk, &sk));
        }
    }
}

#[test]
fn test_wallets_generator() {
    let service_id = 1024;
    let wallets_seed = 1000;
    let wallets_count = 10_000;

    let owners = CreateWalletGenerator::new(service_id, wallets_seed)
        .take(wallets_count)
        .map(|tx| tx.author())
        .collect::<Vec<_>>();

    let gen = TransferGenerator::new(TransferGeneratorConfig {
        service_id,
        seed: 2000,
        wallets_seed,
        wallets_count,
    });

    gen.take(wallets_count)
        .for_each(|x| assert!(owners.contains(&x.author())));
}

#[test]
fn test_wallets_generator2() {
    let service_id = 1024;
    let wallets_seed = 1000;
    let wallets_count = 100_000;

    let mut owners = CreateWalletGenerator::new(service_id, wallets_seed)
        .map(|tx| tx.author())
        .take(wallets_count)
        .collect::<Vec<_>>();

    let gen = TransferGenerator::new(TransferGeneratorConfig {
        service_id,
        seed: 2000,
        wallets_seed,
        wallets_count,
    });
    assert_eq!(
        gen.map(|x| x.author())
            .take(wallets_count)
            .collect::<Vec<_>>()
            .sort(),
        owners.sort()
    );
}

#[test]
fn test_transfer_generator() {
    use std::collections::HashSet;

    let service_id = 1024;
    let wallets_count = 12500;
    let wallets_seed = wallets_count as u64;
    let txs_count = 25000;
    let seed = txs_count as u64;

    let wallet_gen = CreateWalletGenerator::new(service_id, wallets_seed);

    let transfer_gen = TransferGenerator::new(TransferGeneratorConfig {
        service_id,
        seed,
        wallets_count,
        wallets_seed,
    });

    let wallets = wallet_gen
        .map(|x| x.author())
        .take(wallets_count)
        .collect::<HashSet<_>>();

    let txs = transfer_gen
        .map(|x| x.serialize())
        .take(txs_count)
        .collect::<HashSet<_>>();

    assert_eq!(wallets.len(), wallets_count);
    assert_eq!(txs.len(), txs_count);
}
