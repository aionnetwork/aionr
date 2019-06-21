/*******************************************************************************
 * Copyright (c) 2015-2018 Parity Technologies (UK) Ltd.
 * Copyright (c) 2018-2019 Aion foundation.
 *
 *     This file is part of the aion network project.
 *
 *     The aion network project is free software: you can redistribute it
 *     and/or modify it under the terms of the GNU General Public License
 *     as published by the Free Software Foundation, either version 3 of
 *     the License, or any later version.
 *
 *     The aion network project is distributed in the hope that it will
 *     be useful, but WITHOUT ANY WARRANTY; without even the implied
 *     warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
 *     See the GNU General Public License for more details.
 *
 *     You should have received a copy of the GNU General Public License
 *     along with the aion network project source files.
 *     If not, see <https://www.gnu.org/licenses/>.
 *
 ******************************************************************************/

#![warn(unused_extern_crates)]

extern crate aion_types;
extern crate acore;
extern crate rustc_hex;

use acore::transaction::{Transaction, SignedTransaction, Action, PendingTransaction};
use acore::miner::{Miner, MinerService};
use aion_types::U256;
use acore::keychain;
use rustc_hex::FromHex;
use acore::transaction::transaction_queue::PrioritizationStrategy;
use acore::transaction::DEFAULT_TRANSACTION_TYPE;
use acore::client::BlockChainClient;
//use acore::tests::helpers::generate_dummy_client;

fn transaction() -> SignedTransaction {
    let keypair = keychain::ethkey::generate_keypair();
    Transaction {
        action: Action::Create,
        value: U256::zero(),
        data: "3331600055".from_hex().unwrap(),
        gas: U256::from(300_000),
        gas_price: default_gas_price(),
        nonce: U256::zero(),
        transaction_type: DEFAULT_TRANSACTION_TYPE,
        nonce_bytes: Vec::new(),
        gas_price_bytes: Vec::new(),
        gas_bytes: Vec::new(),
        value_bytes: Vec::new(),
    }
    .sign(keypair.secret(), None)
}

fn default_gas_price() -> U256 { 0u64.into() }

//#[test]
//fn internal_seals_without_work() {
//    let spec = Spec::new_instant();
//    let mut miner = Miner::with_spec(&spec);
//    miner.set_minimal_gas_price(0.into());
//
//    let client = generate_dummy_client(2);
//
//    assert_eq!(
//        miner
//            .import_external_transactions(&*client, vec![transaction().into()])
//            .pop()
//            .unwrap()
//            .unwrap(),
//        TransactionImportResult::Current
//    );
//
//    miner.update_sealing(&*client);
//    client.flush_queue();
//    assert!(miner.pending_block(0).is_none());
//    assert_eq!(client.chain_info().best_block_number, 3 as u64);
//
//    assert_eq!(
//        miner
//            .import_own_transaction(
//                &*client,
//                PendingTransaction::new(transaction().into(), None)
//            )
//            .unwrap(),
//        TransactionImportResult::Current
//    );
//
//    miner.update_sealing(&*client);
//    client.flush_queue();
//    assert!(miner.pending_block(0).is_none());
//    assert_eq!(client.chain_info().best_block_number, 4 as u64);
//}
