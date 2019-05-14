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

//! Client-side stratum job dispatcher and mining notifier handler

use std::sync::{Arc, Weak};
use std::net::{SocketAddr, AddrParseError};
use std::fmt;

use block::IsBlock;
use client::Client;
use aion_types::{H256, clean_0x};
use acore_stratum::{
    JobDispatcher, PushWorkHandler, Stratum as StratumService, Error as StratumServiceError,
};
use miner::{self, Miner, MinerService};
use dir::helpers::replace_home_and_local;
use dir::{default_data_path, default_local_path, CHAINS_PATH};

use bytes::Bytes;
use rustc_hex::FromHex;

/// Trait for notifying about new mining work
pub trait NotifyWork: Send + Sync {
    /// Fired when new mining job available
    fn notify_work(&self, pow_hash: H256, target: H256);
}

/// Configures stratum server options.
#[derive(Debug, PartialEq, Clone)]
pub struct Options {
    /// Enable to use stratum
    pub enable: bool,
    /// Working directory
    pub io_path: String,
    /// Network address
    pub listen_addr: String,
    /// Port
    pub port: u16,
    /// Secret for peers
    pub secret: Option<H256>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            enable: true,
            io_path: replace_home_and_local(
                &default_data_path(),
                &default_local_path(),
                CHAINS_PATH,
            ),
            listen_addr: "127.0.0.1".to_string(),
            port: 8008,
            secret: None,
        }
    }
}

struct SubmitPayload {
    nonce: H256,
    pow_hash: H256,
    solution: Bytes,
}

impl SubmitPayload {
    fn from_args(payload: Vec<String>) -> Result<Self, PayloadError> {
        trace!(target: "stratum", "payload = {:?}", payload);
        if payload.len() != 3 {
            return Err(PayloadError::ArgumentsAmountUnexpected(payload.len()));
        }

        let nonce = match clean_0x(&payload[0]).parse::<H256>() {
            Ok(nonce) => nonce,
            Err(e) => {
                warn!(target: "stratum", "submit_work ({}): invalid nonce ({:?})", &payload[0], e);
                return Err(PayloadError::InvalidNonce(payload[0].clone()));
            }
        };

        let pow_hash = match clean_0x(&payload[1]).parse::<H256>() {
            Ok(pow_hash) => pow_hash,
            Err(e) => {
                warn!(target: "stratum", "submit_work ({}): invalid hash ({:?})", &payload[1], e);
                return Err(PayloadError::InvalidPowHash(payload[1].clone()));
            }
        };

        let solution = FromHex::from_hex(clean_0x(&payload[2]))
            .map_err(|_| PayloadError::InvalidSolution(payload[2].clone()))?;

        Ok(SubmitPayload {
            nonce: nonce,
            pow_hash: pow_hash,
            solution: solution,
        })
    }
}

#[derive(Debug)]
enum PayloadError {
    ArgumentsAmountUnexpected(usize),
    InvalidNonce(String),
    InvalidPowHash(String),
    InvalidSolution(String),
}

impl fmt::Display for PayloadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { fmt::Debug::fmt(&self, f) }
}

/// Job dispatcher for stratum service
pub struct StratumJobDispatcher {
    client: Weak<Client>,
    miner: Weak<Miner>,
}

impl JobDispatcher for StratumJobDispatcher {
    fn initial(&self) -> Option<String> {
        // initial payload may contain additional data, not in this case
        //        self.job()
        // aion miner ignored:
        // 1. session id, 2. extranonce
        Some(r#"["0", ""]"#.to_string())
    }

    fn job(&self) -> Option<String> {
        self.with_core(|client, miner| {
            miner.map_sealing_work(&*client, |b| {
                let pow_hash = b.block().header().mine_hash();
                let target = b.block().header().boundary();

                self.payload(pow_hash, target)
            })
        })
    }

    fn submit(&self, payload: Vec<String>) -> Result<(), StratumServiceError> {
        // TODO: consider different struct for different engine.
        let payload = SubmitPayload::from_args(payload)
            .map_err(|e| StratumServiceError::Dispatch(e.to_string()))?;

        trace!(
            target: "stratum",
            "submit_work: Decoded: nonce={}, pow_hash={}, solution={:?}",
            payload.nonce,
            payload.pow_hash,
            payload.solution,
        );

        self.with_core_result(|client, miner| {
            //            let seal = vec![encode(&payload.mix_hash).into_vec(), encode(&payload.nonce).into_vec()];
            let seal = vec![payload.nonce.to_vec(), payload.solution.to_vec()];
            match miner.submit_seal(&*client, payload.pow_hash, seal) {
                Ok(_) => Ok(()),
                Err(e) => {
                    warn!(target: "stratum", "submit_seal error: {:?}", e);
                    Err(StratumServiceError::Dispatch(e.to_string()))
                }
            }
        })
    }
}

impl StratumJobDispatcher {
    /// New stratum job dispatcher given the miner and client
    fn new(miner: Weak<Miner>, client: Weak<Client>) -> StratumJobDispatcher {
        StratumJobDispatcher {
            client: client,
            miner: miner,
        }
    }

    /// Serializes payload for stratum service
    fn payload(&self, pow_hash: H256, target: H256) -> String {
        // in order to insert incremental job id, return
        // param 1 - clean. non-clean job will be ignored by miner.
        // param 2 - target
        // param 3 - header hash
        format!(r#"true, "{:x}", "{:x}""#, target, pow_hash)
    }

    fn with_core<F, R>(&self, f: F) -> Option<R>
    where F: Fn(Arc<Client>, Arc<Miner>) -> Option<R> {
        self.client
            .upgrade()
            .and_then(|client| self.miner.upgrade().and_then(|miner| (f)(client, miner)))
    }

    fn with_core_result<F>(&self, f: F) -> Result<(), StratumServiceError>
    where F: Fn(Arc<Client>, Arc<Miner>) -> Result<(), StratumServiceError> {
        match (self.client.upgrade(), self.miner.upgrade()) {
            (Some(client), Some(miner)) => f(client, miner),
            _ => Ok(()),
        }
    }
}

/// Wrapper for dedicated stratum service
pub struct Stratum {
    dispatcher: Arc<StratumJobDispatcher>,
    service: Arc<StratumService>,
}

#[derive(Debug)]
/// Stratum error
pub enum Error {
    /// IPC sockets error
    Service(StratumServiceError),
    /// Invalid network address
    Address(AddrParseError),
}

impl From<StratumServiceError> for Error {
    fn from(service_err: StratumServiceError) -> Error { Error::Service(service_err) }
}

impl From<AddrParseError> for Error {
    fn from(err: AddrParseError) -> Error { Error::Address(err) }
}

impl NotifyWork for Stratum {
    fn notify_work(&self, pow_hash: H256, target: H256) {
        trace!(target: "stratum", "Notify work");

        self.service
            .push_work_all(self.dispatcher.payload(pow_hash, target))
            .unwrap_or_else(|e| warn!(target: "stratum", "Error while pushing work: {:?}", e));
    }
}

impl Stratum {
    /// New stratum job dispatcher, given the miner, client and dedicated stratum service
    pub fn start(
        options: &Options,
        miner: Weak<Miner>,
        client: Weak<Client>,
    ) -> Result<Stratum, Error>
    {
        use std::net::IpAddr;

        let dispatcher = Arc::new(StratumJobDispatcher::new(miner, client));

        let stratum_svc = StratumService::start(
            &SocketAddr::new(options.listen_addr.parse::<IpAddr>()?, options.port),
            dispatcher.clone(),
            options.secret.clone(),
        )?;

        Ok(Stratum {
            dispatcher: dispatcher,
            service: stratum_svc,
        })
    }

    /// Start STRATUM job dispatcher and register it in the miner
    pub fn register(cfg: &Options, miner: Arc<Miner>, client: Weak<Client>) -> Result<(), Error> {
        let stratum = miner::Stratum::start(cfg, Arc::downgrade(&miner.clone()), client)?;
        miner.push_notifier(Box::new(stratum) as Box<NotifyWork>);
        Ok(())
    }
}
