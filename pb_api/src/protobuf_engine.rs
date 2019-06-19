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

#![allow(dead_code)]
#![allow(unused_must_use)]
use zmq::{self, Socket, CurveKeyPair};
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use message::{Servs, Retcode};
use pb_api_util;
use protobuf::ProtobufEnum;
use api_process::{ApiProcess, to_rsp_msg, to_rsp_msg_with_result};
use aion_rpc::traits::Pb;
use std::net::SocketAddr;
use acore::transaction::local_transactions::TxIoMessage;
use io::IoService;
use std::io::Error;
use std::fs;
use dir::default_data_path;
use std::path::Path;
use super::LOG_TARGET;
static API_REQ_HEADER_LEN: usize = 4;
static API_VAR: u8 = 2;
static ZMQ_WK_TH: &str = "inproc://aionZmqWkTh";
static ZMQ_CB_TH: &str = "inproc://aionZmqCbTh";
static ZMQ_EV_TH: &str = "inproc://aionZmqEvTh";
static ZMQ_HB_TH: &str = "inproc://aionZmqHbTh";
static ZMQ_HWM: i32 = 100_000;
static SOCKET_RECV_TIMEOUT: i32 = 3000;
static SOCKET_ID_LEN: usize = 5;
static RET_CODE_TX_INCLUDED: i32 = 105;

struct CtxSocks {
    fe_socks: Socket,
    wk_socks: Socket,
    cb_socks: Socket,
    hb_socks: Socket,
}

impl CtxSocks {
    fn bind(
        ctx: &Arc<zmq::Context>,
        conf: WalletApiConfiguration,
        endpoint: &str,
    ) -> Result<Self, Error>
    {
        let fe_socks = ctx.socket(zmq::ROUTER)?;
        if conf.secure_connect_enabled {
            if let Ok((Some(curve_pub_key), Some(curve_sec_key))) =
                load_curve_key_pair(conf.zmq_key_path.clone())
            {
                fe_socks.set_zap_domain("global");
                fe_socks.set_curve_server(true);
                fe_socks.set_curve_publickey(&curve_pub_key);
                fe_socks.set_curve_secretkey(&curve_sec_key);
                info!(target: LOG_TARGET, "Secure connection enabled!");
            } else {
                info!(
                    target: LOG_TARGET,
                    "Can't find the key file for setup the connection"
                );
                if let Ok(keypair) = CurveKeyPair::new() {
                    fe_socks.set_zap_domain("global");
                    fe_socks.set_curve_server(true);
                    fe_socks.set_curve_publickey(&keypair.public_key);
                    fe_socks.set_curve_secretkey(&keypair.secret_key);
                    info!(target: LOG_TARGET, "generate a zmq keypair, saving it ...");
                    let public_key = {
                        let mut path = Path::new(&conf.zmq_key_path).to_path_buf();
                        path.push("zmqCurvePubkey");
                        path
                    };
                    let secret_key = {
                        let mut path = Path::new(&conf.zmq_key_path).to_path_buf();
                        path.push("zmqCurveSeckey");
                        path
                    };
                    fs::write(&public_key, &keypair.public_key)?;
                    fs::write(&secret_key, &keypair.secret_key)?;
                    info!(target: LOG_TARGET, "Secure connection enabled!");
                } else {
                    error!(
                        target: LOG_TARGET,
                        "Can't generate a keypair. Secure connection disabled!"
                    );
                }
            }
        } else {
            info!(target: LOG_TARGET, "Secure connection disabled!");
        }

        fe_socks.set_sndhwm(ZMQ_HWM);
        fe_socks.bind(endpoint)?;

        let wk_socks = ctx.socket(zmq::DEALER)?;
        wk_socks.bind(ZMQ_WK_TH)?;

        let cb_socks = ctx.socket(zmq::DEALER)?;
        cb_socks.bind(ZMQ_CB_TH)?;

        let hb_socks = ctx.socket(zmq::DEALER)?;
        hb_socks.bind(ZMQ_HB_TH)?;
        Ok(CtxSocks {
            fe_socks,
            wk_socks,
            cb_socks,
            hb_socks,
        })
    }
}

fn load_curve_key_pair(path: String) -> Result<(Option<String>, Option<String>), Error> {
    if !Path::new(&path).exists() {
        return Ok((None, None));
    }
    let mut curve_pub_key: Option<String> = None;
    let mut curve_sec_key: Option<String> = None;
    let mut nextload: String = "".to_owned();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.to_string_lossy().contains("zmqCurvePubkey") {
            curve_pub_key = Some(fs::read_to_string(path.clone())?);
            nextload = path
                .to_string_lossy()
                .replace("zmqCurvePubkey", "zmqCurveSeckey");
        } else if path.to_string_lossy().contains("zmqCurveSeckey") {
            curve_sec_key = Some(fs::read_to_string(path.clone())?);
            nextload = path
                .to_string_lossy()
                .replace("zmqCurveSeckey", "zmqCurvePubkey");
        } else if nextload.eq(&path.to_string_lossy()) {
            if nextload.contains("zmqCurveSeckey") {
                curve_sec_key = Some(fs::read_to_string(path.clone())?);
            } else {
                curve_pub_key = Some(fs::read_to_string(path.clone())?);
            }
            break;
        }
    }
    Ok((curve_pub_key, curve_sec_key))
}

#[derive(Debug, Clone, PartialEq)]
pub struct WalletApiConfiguration {
    pub enabled: bool,
    pub interface: String,
    pub port: u16,
    pub secure_connect_enabled: bool,
    pub zmq_key_path: String,
}

impl Default for WalletApiConfiguration {
    fn default() -> Self {
        WalletApiConfiguration {
            enabled: false,
            interface: "127.0.0.1".into(),
            port: 8547,
            secure_connect_enabled: false,
            zmq_key_path: {
                let base = default_data_path();
                let mut base = Path::new(&base).to_path_buf();
                base.push("zmq");
                base.to_string_lossy().into()
            },
        }
    }
}

/// initialize protobuf engine and start.
pub fn new_pb(
    conf: WalletApiConfiguration,
    client: Arc<Pb>,
    io_service: IoService<TxIoMessage>,
) -> Result<Option<PBEngine>, String>
{
    if !conf.enabled {
        Ok(None)
    } else {
        // validate url
        let url = format!("{}:{}", conf.interface, conf.port);
        let _addr: SocketAddr = url
            .parse()
            .map_err(|_| format!("Invalid Wallet api server listen host/port given: {}", url))?;

        let mut pb = PBEngine::new(client, io_service, url);
        match pb.run(conf) {
            Ok(()) => Ok(Some(pb)),
            Err(e) => Err(format!("wallet server start failed: {}", e)),
        }
    }
}

pub struct PBEngine {
    shutdown: Arc<AtomicBool>,
    apis: Arc<ApiProcess>,
    url: String,
    listeners: Option<Vec<thread::JoinHandle<()>>>,
}

impl PBEngine {
    fn new(client: Arc<Pb>, io_service: IoService<TxIoMessage>, url: String) -> PBEngine {
        let apis = Arc::new(ApiProcess::new(client, io_service));
        let shutdown = Arc::new(AtomicBool::new(true));
        let bind_url = format!("tcp://{}", url);
        info!(target: LOG_TARGET, "bind to address: {}", bind_url.as_str());
        PBEngine {
            shutdown,
            apis,
            url: bind_url,
            listeners: None,
        }
    }

    /// start threads to listen on each socket channel
    /// @return vector of listener threads.
    fn run(&mut self, conf: WalletApiConfiguration) -> Result<(), Error> {
        let ctx = Arc::new(zmq::Context::new());
        let socks = CtxSocks::bind(&ctx, conf, &self.url)?;
        let mut listeners = vec![];
        {
            // server listener thread
            let shutdown_0 = Arc::clone(&self.shutdown);
            let server_listener = thread::Builder::new()
                .name("pb_server".to_string())
                .spawn(move || {
                    PBEngine::listen_server(socks, shutdown_0);
                })
                .expect("start thread: pb_server failed");
            listeners.push(server_listener);
        }

        {
            // heart beat thread
            let arc_ctx_0 = Arc::clone(&ctx);
            let shutdown_0 = Arc::clone(&self.shutdown);
            let heartbeat_listener = thread::Builder::new()
                .name("pb_heartbeat".to_string())
                .spawn(move || {
                    PBEngine::listen_heartbeat(arc_ctx_0, shutdown_0);
                })
                .expect("start thread: pb_heartbeat failed");
            listeners.push(heartbeat_listener);
        }

        {
            // worker thread
            let arc_ctx_1 = Arc::clone(&ctx);
            let shutdown_1 = Arc::clone(&self.shutdown);
            let apis_1 = Arc::clone(&self.apis);
            let work_listener = thread::Builder::new()
                .name("pb_worker".to_string())
                .spawn(move || {
                    PBEngine::listen_worker(arc_ctx_1, shutdown_1, apis_1);
                })
                .expect("start thread: pb_worker failed");
            listeners.push(work_listener);
        }

        {
            // callback thread
            let arc_ctx_2 = Arc::clone(&ctx);
            let shutdown_2 = Arc::clone(&self.shutdown);
            let apis_2 = Arc::clone(&self.apis);
            let callback_listener = thread::Builder::new()
                .name("pb_callback".to_string())
                .spawn(move || {
                    PBEngine::listen_callback(arc_ctx_2, shutdown_2, apis_2);
                })
                .expect("start thread: pb_callback failed");
            listeners.push(callback_listener);
        }
        self.listeners = Some(listeners);
        Ok(())
    }

    /// init socket server and loop for data handling
    fn listen_server(socks: CtxSocks, shutdown: Arc<AtomicBool>) {
        let mut items = vec![
            socks.fe_socks.as_poll_item(zmq::POLLIN),
            socks.wk_socks.as_poll_item(zmq::POLLIN),
            socks.cb_socks.as_poll_item(zmq::POLLIN),
            socks.hb_socks.as_poll_item(zmq::POLLIN),
        ];

        while shutdown.load(Ordering::Relaxed) {
            // wait while there are either requests or replies to process.
            let rc = zmq::poll(&mut items, 3000).expect("zmq poll error!");
            if rc < 0 {
                continue;
            }
            // process a request
            if items[0].is_readable() {
                PBEngine::on_receive(&socks.fe_socks, &socks.wk_socks, &socks.hb_socks);
            }
            // process a reply
            if items[1].is_readable() {
                PBEngine::on_send(&socks.wk_socks, &socks.fe_socks);
            }
            // process a callback
            if items[2].is_readable() {
                PBEngine::on_send(&socks.cb_socks, &socks.fe_socks);
            }
            // heart beat reply
            if items[3].is_readable() {
                PBEngine::on_send(&socks.hb_socks, &socks.fe_socks);
            }
        }
    }

    /// init callback socket client and loop for data handling.
    fn listen_callback(
        arc_ctx: Arc<zmq::Context>,
        shutdown: Arc<AtomicBool>,
        apis: Arc<ApiProcess>,
    )
    {
        let sock = arc_ctx.socket(zmq::DEALER).expect("create socket failed");
        sock.connect(ZMQ_CB_TH);
        sock.set_rcvtimeo(SOCKET_RECV_TIMEOUT);
        sock.set_sndtimeo(SOCKET_RECV_TIMEOUT);
        while shutdown.load(Ordering::Relaxed) {
            let tps = apis.take_tx_status();
            if tps.is_empty() {
                continue;
            }

            let rsp_msg = if tps.to_tx_return_code() == RET_CODE_TX_INCLUDED {
                to_rsp_msg_with_result(
                    tps.msg_hash(),
                    tps.to_tx_return_code(),
                    tps.error(),
                    tps.tx_result(),
                )
            } else {
                to_rsp_msg(tps.msg_hash(), tps.to_tx_return_code(), tps.error())
            };

            debug!(target: LOG_TARGET, "callback listener send");
            debug!(target: LOG_TARGET, "socket_id: [{:?}]", tps.socket_id());
            debug!(target: LOG_TARGET, "msg_hash: [{:?}]", tps.msg_hash());
            debug!(
                target: LOG_TARGET,
                "tx_return_code: [{:?}]",
                tps.to_tx_return_code()
            );
            trace!(target: LOG_TARGET, "rsp_msg: [{:?}]", rsp_msg);

            let re = sock
                .send(tps.socket_id(), zmq::SNDMORE)
                .and_then(|_| sock.send(rsp_msg.as_slice(), zmq::DONTWAIT));
            if re.is_err() {
                error!(
                    target: LOG_TARGET,
                    "callback listener sock.send exception: {:?}",
                    re.err().unwrap()
                );
            }
        }
    }

    /// init worker client and loop for data receiving and sending.
    fn listen_worker(arc_ctx: Arc<zmq::Context>, shutdown: Arc<AtomicBool>, apis: Arc<ApiProcess>) {
        let sock = arc_ctx.socket(zmq::DEALER).expect("create socket failed");
        sock.connect(ZMQ_WK_TH);
        sock.set_rcvtimeo(SOCKET_RECV_TIMEOUT);
        sock.set_sndtimeo(SOCKET_RECV_TIMEOUT);
        while shutdown.load(Ordering::Relaxed) {
            let mut re = sock.recv_bytes(0);
            if re.is_ok() {
                let socket_id = re.unwrap();
                debug!(
                    target: LOG_TARGET,
                    "worker listener socketID: [{:?}]",
                    socket_id
                );
                if socket_id.len() == SOCKET_ID_LEN {
                    re = sock.recv_bytes(0);
                    if re.is_ok() {
                        let req_msg = re.unwrap();
                        trace!(
                            target: LOG_TARGET,
                            "worker listener req_msg: [{:?}]",
                            req_msg
                        );
                        let rsp_msg = apis.process(&req_msg, &socket_id);
                        trace!(
                            target: LOG_TARGET,
                            "worker listener rsp_msg: [{:?}]",
                            rsp_msg
                        );
                        let re = sock
                            .send(socket_id.as_slice(), zmq::SNDMORE)
                            .and_then(|_| sock.send(rsp_msg.as_slice(), zmq::DONTWAIT));
                        if re.is_err() {
                            error!(
                                target: LOG_TARGET,
                                "heartbeat listener sock.send failed: {:?}",
                                re.err().unwrap()
                            );
                        }
                    }
                }
            }
            //            } else {
            //                error!(
            //                    target: LOG_TARGET,
            //                    "worker listener recv bytes failed: {:?}",
            //                    re.err().unwrap()
            //                );
            //            }
        }
        info!(target: LOG_TARGET, "close worker listener sockets...");
    }

    /// init heart beat socket client and loop for data receiving and sending.
    fn listen_heartbeat(arc_ctx: Arc<zmq::Context>, shutdown: Arc<AtomicBool>) {
        let sock = arc_ctx.socket(zmq::DEALER).expect("create socket failed");
        sock.connect(ZMQ_HB_TH);
        sock.set_rcvtimeo(SOCKET_RECV_TIMEOUT);
        sock.set_sndtimeo(SOCKET_RECV_TIMEOUT);
        while shutdown.load(Ordering::Relaxed) {
            let mut re = sock.recv_bytes(0);
            if re.is_ok() {
                let socket_id = re.unwrap();
                debug!(
                    target: LOG_TARGET,
                    "heartbeat listener socketID: [{:?}]",
                    socket_id
                );
                if socket_id.len() == SOCKET_ID_LEN {
                    re = sock.recv_bytes(0);
                    if re.is_ok() {
                        let req_msg = re.unwrap();

                        trace!(
                            target: LOG_TARGET,
                            "heartbeat listener req_msg: [{:?}]",
                            req_msg
                        );
                        let rsp_msg = pb_api_util::to_return_header(
                            API_VAR,
                            Retcode::r_heartbeatReturn.value(),
                        );
                        trace!(
                            target: LOG_TARGET,
                            "heartbeat listener rsp_msg: [{:?}]",
                            rsp_msg
                        );
                        let re = sock
                            .send(socket_id.as_slice(), zmq::SNDMORE)
                            .and_then(|_| sock.send(rsp_msg.as_slice(), zmq::DONTWAIT));
                        if re.is_err() {
                            error!(
                                target: LOG_TARGET,
                                "heartbeat listener sock.send failed: {:?}",
                                re.err().unwrap()
                            );
                        }
                    }
                }
            }
        }
        info!(target: LOG_TARGET, "close heartbeat listener sockets...");
    }

    /// handle messages received on each socket channel.
    fn on_receive(
        receiver: &zmq::Socket,
        sender: &zmq::Socket,
        hb: &zmq::Socket,
    ) -> zmq::Result<()>
    {
        debug!(
            target: LOG_TARGET,
            "===============on receive==================="
        );
        let msg = receiver.recv_bytes(0)?;
        debug!(target: LOG_TARGET, "receive msg={:?}", msg);

        let has_more = receiver.get_rcvmore()?;
        if has_more {
            let msg_more = receiver.recv_bytes(0)?;
            debug!(target: LOG_TARGET, "receive more msg= {:?}", msg_more);

            if PBEngine::is_heartbeat(&msg_more) {
                debug!(
                    target: LOG_TARGET,
                    "is heartbeat, resend msg and msg more to hb socket."
                );
                hb.send(msg.as_slice(), zmq::SNDMORE)?;
                hb.send(msg_more.as_slice(), zmq::DONTWAIT)?;
            } else {
                debug!(
                    target: LOG_TARGET,
                    "is not heartbeart, resend msg and msg more to wk socket."
                );
                sender.send(msg.as_slice(), zmq::SNDMORE)?;
                sender.send(msg_more.as_slice(), zmq::DONTWAIT)?;
            }
        } else {
            debug!(
                target: LOG_TARGET,
                "is not heartbeat, resend msg to wk socket."
            );
            sender.send(msg.as_slice(), zmq::DONTWAIT)?;
        }

        Ok(())
    }

    /// handle messages sent to each socket channel.
    fn on_send(receiver: &zmq::Socket, sender: &zmq::Socket) -> zmq::Result<()> {
        debug!(
            target: LOG_TARGET,
            "===============on send==================="
        );
        let msg = receiver.recv_bytes(0)?;
        debug!(target: LOG_TARGET, "receive msg={:?}", msg);

        let has_more = receiver.get_rcvmore()?;
        if has_more {
            let msg_more = receiver.recv_bytes(0)?;
            debug!(target: LOG_TARGET, "receive more msg= {:?}", msg_more);

            sender.send(msg.as_slice(), zmq::SNDMORE)?;
            sender.send(msg_more.as_slice(), zmq::DONTWAIT)?;
            debug!(target: LOG_TARGET, "resend msg and msg more.");
        } else {
            sender.send(msg.as_slice(), zmq::DONTWAIT)?;
            debug!(target: LOG_TARGET, "resend msg.");
        }
        Ok(())
    }

    /// check if the given byte array is a heart beat payload.
    fn is_heartbeat(msg: &Vec<u8>) -> bool {
        if msg.len() != API_REQ_HEADER_LEN {
            return false;
        }
        if msg[0] < API_VAR {
            return false;
        }
        if msg[1] as i32 != Servs::s_hb.value() {
            return false;
        }
        true
    }
}

impl Drop for PBEngine {
    fn drop(&mut self) {
        self.shutdown.store(false, Ordering::Relaxed);
        self.apis.shut_down();
        if self.listeners.is_some() {
            info!(target: LOG_TARGET, "zmq proxy thread was interrupted.");
            for thread in self.listeners.take().expect("drop pb api server failed") {
                info!(
                    target: LOG_TARGET,
                    "Shutting down {} .... ",
                    thread.thread().name().unwrap_or("sub thread")
                );
                let _ = thread.join();
            }
            self.listeners = None;
            info!(target: LOG_TARGET, "Shutdown Zmq sockets... Done!");
        }
    }
}
