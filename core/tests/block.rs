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

extern crate acore;
extern crate aion_types;
extern crate types;
extern crate vms;
extern crate db;
#[macro_use]
extern crate log;

use std::sync::Arc;
use acore::block::{OpenBlock, LockedBlock};
use acore::engines::POWEquihashEngine;
use acore::error::Error;
use acore::header::Header;
use acore::factory::Factories;
use acore::state_db::StateDB;
use acore::state::State;
use acore::views::BlockView;
use acore::transaction::SignedTransaction;
use db::MemoryDBRepository;
use aion_types::Address;
use types::vms::LastHashes;

/// Enact the block given by `block_bytes` using `engine` on the database `db` with given `parent` block header
fn enact_bytes(
    block_bytes: &[u8],
    engine: &POWEquihashEngine,
    db: StateDB,
    parent: &Header,
    _grant_parent: Option<&Header>,
    last_hashes: Arc<LastHashes>,
    factories: Factories,
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

    {
        if log::max_log_level() >= log::LogLevel::Trace {
            let s = State::from_existing(
                db.boxed_clone(),
                parent.state_root().clone(),
                engine.machine().account_start_nonce(parent.number() + 1),
                factories.clone(),
                Arc::new(MemoryDBRepository::new()),
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
        None,
        last_hashes,
        Address::new(),
        (3141562.into(), 31415620.into()),
        vec![],
        Arc::new(MemoryDBRepository::new()),
    )?;

    b.populate_from(&header);
    b.push_transactions(&transactions)?;

    Ok(b.close_and_lock())
}

//#[test]
//fn open_block() {
//    let spec = Spec::new_test();
//    let genesis_header = spec.genesis_header();
//    let db = spec
//        .ensure_db_good(get_temp_state_db(), &Default::default())
//        .unwrap();
//    let last_hashes = Arc::new(vec![genesis_header.hash()]);
//    let b = OpenBlock::new(
//        &*spec.engine,
//        Default::default(),
//        db,
//        &genesis_header,
//        None,
//        last_hashes,
//        Address::zero(),
//        (3141562.into(), 31415620.into()),
//        vec![],
//        Arc::new(MemoryDBRepository::new()),
//    )
//        .unwrap();
//    let b = b.close_and_lock();
//    let _ = b.seal(&*spec.engine, vec![]);
//}
