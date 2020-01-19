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

use log;
use std::sync::Arc;
use crate::block::{OpenBlock, LockedBlock, SealedBlock, Drain};
use crate::engine::Engine;
use crate::types::error::Error;
use crate::header::Header;
use crate::factory::Factories;
use crate::db::StateDB;
use crate::state::State;
use crate::views::BlockView;
use crate::transaction::SignedTransaction;
use kvdb::MockDbRepository;
use aion_types::Address;
use vms::LastHashes;
use crate::tests::common::helpers::get_temp_state_db;
use crate::tests::common::TestBlockChainClient;
use crate::spec::Spec;
use crate::client::BlockChainClient;

/// Enact the block given by `block_bytes` using `engine` on the database `db` with given `parent` block header
fn enact_bytes(
    block_bytes: &[u8],
    engine: &dyn Engine,
    db: StateDB,
    parent: &Header,
    grand_parent: Option<&Header>,
    great_grand_parent: Option<&Header>,
    last_hashes: Arc<LastHashes>,
    factories: Factories,
    client: &dyn BlockChainClient,
) -> Result<LockedBlock, Error>
{
    let block = BlockView::new(block_bytes);
    let header = block.header();
    let transactions: Result<Vec<_>, Error> = block
        .transactions()
        .into_iter()
        .map(SignedTransaction::new)
        .map(|r| r.map_err(Into::into))
        .collect();
    let transactions = transactions?;
    let seal_type = header.seal_type().clone();

    {
        if log::max_log_level() >= log::LogLevel::Trace {
            let s = State::from_existing(
                db.boxed_clone(),
                parent.state_root().clone(),
                engine.account_start_nonce(parent.number() + 1),
                factories.clone(),
                Arc::new(MockDbRepository::init(vec![])),
            )?;
            trace!(target: "enact", "num={}, root={}, author={}, author_balance={}\n",
                   header.number(), s.root(), header.author(), s.balance(&header.author())?);
        }
    }

    let mut b = OpenBlock::new(
        engine,
        factories,
        db,
        parent,
        seal_type.unwrap_or_default(),
        grand_parent,
        great_grand_parent,
        last_hashes,
        Address::new(),
        (3141562.into(), 31415620.into()),
        vec![],
        Arc::new(MockDbRepository::init(vec![])),
        None,
        client,
    )?;

    b.populate_from(&header);
    b.push_transactions(&transactions)?;

    Ok(b.close_and_lock())
}

/// Enact the block given by `block_bytes` using `engine` on the database `db` with given `parent` block header. Seal the block afterwards
fn enact_and_seal(
    block_bytes: &[u8],
    engine: &dyn Engine,
    db: StateDB,
    parent: &Header,
    grand_parent: Option<&Header>,
    great_grand_parent: Option<&Header>,
    last_hashes: Arc<LastHashes>,
    factories: Factories,
    client: &dyn BlockChainClient,
) -> Result<SealedBlock, Error>
{
    let header = BlockView::new(block_bytes).header_view();
    Ok(enact_bytes(
        block_bytes,
        engine,
        db,
        parent,
        grand_parent,
        great_grand_parent,
        last_hashes,
        factories,
        client,
    )?
    .seal(engine, header.seal())?)
}

#[test]
fn open_block() {
    let spec = Spec::new_test();
    let genesis_header = spec.genesis_header();
    let client = TestBlockChainClient::new_with_spec(spec.clone());
    let db = spec
        .ensure_db_good(get_temp_state_db(), &Default::default())
        .unwrap();
    let last_hashes = Arc::new(vec![genesis_header.hash()]);
    let b = OpenBlock::new(
        &*spec.engine,
        Default::default(),
        db,
        &genesis_header,
        Default::default(),
        None,
        None,
        last_hashes,
        Address::zero(),
        (3141562.into(), 31415620.into()),
        vec![],
        Arc::new(MockDbRepository::init(vec![])),
        None,
        &client,
    )
    .unwrap();
    let b = b.close_and_lock();
    let res = b.seal(&*spec.engine, vec![]);
    assert!(res.is_ok());
}

#[test]
fn enact_block() {
    use crate::spec::*;
    let spec = Spec::new_test();
    let engine = &*spec.engine;
    let genesis_header = spec.genesis_header();
    let client = TestBlockChainClient::new_with_spec(spec.clone());

    let db = spec
        .ensure_db_good(get_temp_state_db(), &Default::default())
        .unwrap();
    let last_hashes = Arc::new(vec![genesis_header.hash()]);
    let b = OpenBlock::new(
        engine,
        Default::default(),
        db,
        &genesis_header,
        Default::default(),
        None,
        None,
        last_hashes.clone(),
        Address::zero(),
        (3141562.into(), 31415620.into()),
        vec![],
        Arc::new(MockDbRepository::init(vec![])),
        None,
        &client,
    )
    .unwrap()
    .close_and_lock()
    .seal(engine, vec![])
    .unwrap();
    let orig_bytes = b.rlp_bytes();
    let orig_db = b.drain();

    let db = spec
        .ensure_db_good(get_temp_state_db(), &Default::default())
        .unwrap();
    let e = enact_and_seal(
        &orig_bytes,
        engine,
        db,
        &genesis_header,
        None,
        None,
        last_hashes,
        Default::default(),
        &client,
    )
    .unwrap();

    assert_eq!(e.rlp_bytes(), orig_bytes);

    let db = e.drain();
    assert_eq!(orig_db.journal_db().keys(), db.journal_db().keys());
    assert!(
        orig_db
            .journal_db()
            .keys()
            .iter()
            .filter(|k| orig_db.journal_db().get(k.0) != db.journal_db().get(k.0))
            .next()
            .is_none()
    );
}
