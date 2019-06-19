/*******************************************************************************
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
//
//#![warn(unused_extern_crates)]
//
//extern crate acore;
//extern crate db;
//extern crate sync;
//extern crate acore_io;
//
//use std::sync::Arc;
//use acore::client::{BlockChainClient, Client, ClientConfig};
//use acore::miner::Miner;
//use acore::spec::Spec;
//use acore::db::DB_NAMES;
//use acore_io::IoChannel;
//use db::KeyValueDB;
//use db::MockDbRepository;
//use sync::sync::storage::SyncStorage;
//
//
//
//pub fn new_spec() -> Spec {
//    load(
//        &"$HOME/.aion/cache_".into(),
//        include_bytes!("../../resources/mainnet.json"),
//    )
//}
//
//fn new_db() -> Arc<KeyValueDB> {
//    let mut db_configs = Vec::new();
//    for db_name in DB_NAMES.to_vec() {
//        db_configs.push(db_name.into());
//    }
//    Arc::new(MockDbRepository::init(db_configs))
//}
//
//pub fn init_sync_storage() {
//    let spec = new_spec();
//    let client = get_client(&spec);
//    SyncStorage::init(client.clone() as Arc<BlockChainClient>);
//}
//
