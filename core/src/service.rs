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

//! Creates and registers client and network services.

use std::time::{Instant, Duration};
use std::path::Path;
use std::sync::Arc;

use futures::sync::oneshot;
use tokio::runtime::TaskExecutor;
use tokio::timer::Interval;
use tokio::prelude::{Future, Stream};
use ansi_term::Colour;
use bytes::Bytes;
use client::{ChainNotify, Client, ClientConfig};
use db;
use error::*;
use io::*;
use kvdb::KeyValueDB;
use kvdb::{DatabaseConfig, RepositoryConfig, DbRepository, DBTransaction, Error as DbError};
use miner::Miner;
use spec::Spec;
use stop_guard::StopGuard;
use aion_types::{H256, U256};
use rlp::*;

/// Message type for external and internal events
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ClientIoMessage {
    /// Best Block Hash in chain has been changed
    NewChainHead,
    /// An external block is verified and ready to be imported
    BlockVerified,
    /// New transaction RLPs are ready to be imported
    NewTransactions(Vec<Bytes>, usize),
    /// New consensus message received.
    NewMessage(Bytes),
}

/// Run the miner
pub fn run_miner(executor: TaskExecutor, client: Arc<Client>) -> oneshot::Sender<()> {
    let (close, shutdown_signal) = oneshot::channel();
    // let seal_block_task = Interval::new(Instant::now(), client.prepare_block_interval())
    let seal_block_task = Interval::new(Instant::now(), Duration::from_secs(5))
        .for_each(move |_| {
            let client: Arc<Client> = client.clone();
            client.miner().try_prepare_block(&*client, false);
            Ok(())
        })
        .map_err(|e| panic!("interval err: {:?}", e))
        .select(shutdown_signal.map_err(|_| {}))
        .map(|_| ())
        .map_err(|_| ());
    executor.spawn(seal_block_task);
    close
}

/// Run the transaction pool
pub fn run_transaction_pool(executor: TaskExecutor, client: Arc<Client>) -> oneshot::Sender<()> {
    let (close, shutdown_signal) = oneshot::channel();
    let update_transaction_pool_task = Interval::new(Instant::now(), Duration::from_secs(1))
        .for_each(move |_| {
            let client: Arc<Client> = client.clone();
            client.miner().update_transaction_pool(&*client, false);
            Ok(())
        })
        .map_err(|e| panic!("interval err: {:?}", e))
        .select(shutdown_signal.map_err(|_| {}))
        .map(|_| ())
        .map_err(|_| ());
    executor.spawn(update_transaction_pool_task);
    close
}

/// Client service setup. Creates and registers client and network services with the IO subsystem.
pub struct ClientService {
    io_service: Arc<IoService<ClientIoMessage>>,
    client: Arc<Client>,
    database: Arc<DbRepository>,
    _stop_guard: StopGuard,
}

impl ClientService {
    /// Start the `ClientService`.
    pub fn start(
        config: ClientConfig,
        spec: &Spec,
        client_path: &Path,
        _ipc_path: &Path,
        miner: Arc<Miner>,
    ) -> Result<ClientService, Error>
    {
        let io_service = IoService::<ClientIoMessage>::start()?;

        info!(
            target:"run",
            "Configured for {} using {} engine",
            Colour::White.bold().paint(spec.name.clone()),
            Colour::Yellow.bold().paint(spec.engine.name())
        );

        let mut db_config = DatabaseConfig::default();
        db_config.wal = config.db_wal;
        db_config.block_cache_size = config.db_cache_size.unwrap_or(1024) as u64;
        let mut db_configs = Vec::new();
        for db_name in db::DB_NAMES.to_vec() {
            let db_path = client_path.join(db_name);
            db_config.compact_options = config.db_compaction.compaction_profile(&db_path);
            db_configs.push(RepositoryConfig {
                db_name: db_name.into(),
                db_config: db_config.clone(),
                db_path: db_path.to_string_lossy().into(),
            });
        }
        let dbs = DbRepository::init(db_configs)?;
        let dbs = Arc::new(dbs);

        // correct dbs
        ClientService::correct_db(dbs.clone())
            .map_err(|_e| Error::Database(DbError::Other(format!("db is not correct"))))?;

        let client = Client::new(config, &spec, dbs.clone(), miner, io_service.channel())?;

        let client_io = Arc::new(ClientIoHandler {
            client: client.clone(),
        });
        io_service.register_handler(client_io)?;

        let stop_guard = StopGuard::new();

        Ok(ClientService {
            io_service: Arc::new(io_service),
            client,
            database: dbs,
            _stop_guard: stop_guard,
        })
    }

    /// Get general IO interface
    pub fn register_io_handler(
        &self,
        handler: Arc<IoHandler<ClientIoMessage> + Send>,
    ) -> Result<(), IoError>
    {
        self.io_service.register_handler(handler)
    }

    /// Get client interface
    pub fn client(&self) -> Arc<Client> { self.client.clone() }

    /// Get network service component
    pub fn io(&self) -> Arc<IoService<ClientIoMessage>> { self.io_service.clone() }

    /// Set the actor to be notified on certain chain events
    pub fn add_notify(&self, notify: Arc<ChainNotify>) { self.client.add_notify(notify); }

    /// Get a handle to the database.
    pub fn db(&self) -> Arc<KeyValueDB> { self.database.clone() }

    /// check db if correct
    fn correct_db(dbs: Arc<KeyValueDB>) -> Result<(), String> {
        use db::Readable;
        use blockchain::BlockDetails;
        // get best block hash
        let best_block_hash = dbs.get(db::COL_EXTRA, b"best").expect("EXTRA db not found");
        match best_block_hash {
            None => {
                // new db , nothing to do ;
                return Ok(());
            }
            Some(best) => {
                let best_block_hash = H256::from_slice(&best);
                let best_block_detail: BlockDetails = dbs
                    .read(db::COL_EXTRA, &best_block_hash)
                    .expect("db crashed");
                let best_block_number = best_block_detail.number;
                let mut loop_end = if best_block_number > 10 {
                    10
                } else {
                    best_block_number as i64
                };
                let mut cur_blk_hash = best_block_hash;
                loop {
                    // check header and bodies whether exist in db
                    let header = dbs
                        .get(db::COL_HEADERS, &cur_blk_hash)
                        .expect("HEADERS db not found");
                    let bodies = dbs
                        .get(db::COL_BODIES, &cur_blk_hash)
                        .expect("BODIES db not found");
                    if header.is_none() || bodies.is_none() {
                        break;
                    }
                    loop_end -= 1;
                    if loop_end < 0 {
                        break;
                    }
                    // check it's parent
                    let cur_blk_detail: Option<BlockDetails> =
                        dbs.read(db::COL_EXTRA, &cur_blk_hash);
                    if cur_blk_detail.is_none() {
                        return Err(format!("db crashed"));
                    }
                    let cur_blk_detail = cur_blk_detail.expect("db crashed");
                    cur_blk_hash = cur_blk_detail.parent;
                }
                if loop_end > 0 {
                    // reset db
                    let mut batch = DBTransaction::new();
                    let cur_blk_detail: BlockDetails =
                        dbs.read(db::COL_EXTRA, &cur_blk_hash).expect("db crashed");
                    let parent = cur_blk_detail.parent;
                    let parent_header_bytes = dbs
                        .get(db::COL_HEADERS, &parent)
                        .expect("HEADERS db not found")
                        .expect("db crashed")
                        .to_vec();
                    let parent_header = ::encoded::Header::new(parent_header_bytes).decode();
                    let parnet_number = parent_header.number();
                    batch.put(db::COL_EXTRA, b"best", &parent);
                    let new_parent_block_detail = BlockDetails {
                        number: parnet_number,
                        total_difficulty: U256::from(*parent_header.difficulty()),
                        parent: H256::from(*parent_header.parent_hash()),
                        children: vec![],
                    };
                    // reset state db
                    let latest_era_key = [b'l', b'a', b's', b't', 0, 0, 0, 0, 0, 0, 0, 0];
                    batch.put(db::COL_STATE, &latest_era_key, &encode(&parnet_number));
                    use db::Writable;
                    batch.write(db::COL_EXTRA, &parent, &new_parent_block_detail);
                    let _ = dbs.write(batch);
                }
                return Ok(());
            }
        }
    }
}

/// IO interface for the Client handler
struct ClientIoHandler {
    client: Arc<Client>,
}

const CLIENT_TICK_TIMER: TimerToken = 0;

const CLIENT_TICK_MS: u64 = 5000;

impl IoHandler<ClientIoMessage> for ClientIoHandler {
    fn initialize(&self, io: &IoContext<ClientIoMessage>) {
        io.register_timer(CLIENT_TICK_TIMER, CLIENT_TICK_MS)
            .expect("Error registering client timer");
    }

    fn timeout(&self, _io: &IoContext<ClientIoMessage>, timer: TimerToken) {
        match timer {
            CLIENT_TICK_TIMER => self.client.tick(),
            _ => warn!(target: "io","IO service triggered unregistered timer '{}'", timer),
        }
    }

    fn message(&self, _io: &IoContext<ClientIoMessage>, net_message: &ClientIoMessage) {
        match *net_message {
            ClientIoMessage::BlockVerified => {
                self.client.import_verified_blocks();
            }
            ClientIoMessage::NewChainHead => {
                debug!(target: "block", "ClientIoMessage::NewChainHead");
                let client: Arc<Client> = self.client.clone();
                client.miner().update_transaction_pool(&*client, true);
                client.miner().try_prepare_block(&*client, true);
            }
            _ => {} // ignore other messages
        }
    }
}
