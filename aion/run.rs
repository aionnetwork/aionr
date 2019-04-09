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

use std::sync::{Arc, Weak};
use std::thread;
use std::time::{Duration, Instant};
use std::fs;
use std::io::{BufRead,BufReader};

use acore::account_provider::{AccountProvider, AccountProviderSettings};
use acore::client::{Client, DatabaseCompactionProfile, header_chain::HeaderChain, VMType};
use acore::miner::external::ExternalMiner;
use acore::miner::{Miner, MinerOptions, MinerService};
use acore::miner::{Stratum, StratumOptions};
use acore::service::ClientService;
use acore::transaction::local_transactions::TxIoMessage;
use acore::verification::queue::VerifierSettings;
use aion_rpc::{dispatch::DynamicGasPrice, impls::EthClient, informant};
use aion_version::version;
use ansi_term::Colour;
use cache::CacheConfig;
use ctrlc::CtrlC;
use dir::{DatabaseDirectories, Directories};
use fdlimit::raise_fd_limit;
use helpers::{passwords_from_files, to_client_config};
use dir::helpers::absolute;
use io::{IoChannel, IoService};
use logger::LogConfig;
use modules;
use num_cpus;
use params::{fatdb_switch_to_bool, AccountsConfig, MinerExtras, Pruning, SpecType, Switch};
use parking_lot::{Condvar, Mutex};
use pb::{new_pb, WalletApiConfiguration};
use rpc;
use rpc_apis;
use sync::p2p::{NetworkConfig, P2pMgr};
use sync::sync::{Params, SyncConfig};
use tokio;
use tokio::prelude::*;
use user_defaults::UserDefaults;
// Pops along with error messages when a password is missing or invalid.
const VERIFY_PASSWORD_HINT: &'static str = "Make sure valid password is present in files passed \
                                            using `--password` or in the configuration file.";

#[derive(Debug, PartialEq)]
pub struct RunCmd {
    pub cache_config: CacheConfig,
    pub dirs: Directories,
    pub spec: SpecType,
    pub pruning: Pruning,
    pub pruning_history: u64,
    pub pruning_memory: usize,
    /// Some if execution should be daemonized. Contains pid_file path.
    pub daemon: Option<String>,
    pub logger_config: LogConfig,
    pub miner_options: MinerOptions,
    pub dynamic_gas_price: Option<DynamicGasPrice>,
    pub ws_conf: rpc::WsConfiguration,
    pub http_conf: rpc::HttpConfiguration,
    pub ipc_conf: rpc::IpcConfiguration,
    pub wallet_api_conf: WalletApiConfiguration,
    pub net_conf: NetworkConfig,
    pub acc_conf: AccountsConfig,
    pub miner_extras: MinerExtras,
    pub fat_db: Switch,
    pub compaction: DatabaseCompactionProfile,
    pub wal: bool,
    pub vm_type: VMType,
    pub stratum: StratumOptions,
    pub check_seal: bool,
    pub verifier_settings: VerifierSettings,
    pub no_persistent_txqueue: bool,
}

// node info fetcher for the local store.
struct FullNodeInfo {
    miner: Option<Arc<Miner>>, // TODO: only TXQ needed, just use that after decoupling.
}

impl ::local_store::NodeInfo for FullNodeInfo {
    fn pending_transactions(&self) -> Vec<::acore::transaction::PendingTransaction> {
        let miner = match self.miner.as_ref() {
            Some(m) => m,
            None => return Vec::new(),
        };

        let local_txs = miner.local_transactions();
        miner
            .pending_transactions()
            .into_iter()
            .chain(miner.future_transactions())
            .filter(|tx| local_txs.contains_key(&tx.hash()))
            .collect()
    }
}

pub fn execute_impl(cmd: RunCmd) -> Result<(Weak<Client>), String> {
    // load spec
    let spec = cmd.spec.spec(&cmd.dirs.cache)?;

    // load genesis hash
    let genesis_hash = spec.genesis_header().hash();

    // database paths
    let db_dirs = cmd
        .dirs
        .database(genesis_hash.clone(), None, spec.data_dir.clone());

    // user defaults path
    let user_defaults_path = db_dirs.user_defaults_path();

    // load user defaults
    let mut user_defaults = UserDefaults::load(&user_defaults_path)?;

    // select pruning algorithm
    let algorithm = cmd.pruning.to_algorithm(&user_defaults);

    // check if fatdb is on
    let fat_db = fatdb_switch_to_bool(cmd.fat_db, &user_defaults, algorithm)?;

    // prepare client paths.
    let client_path = db_dirs.client_path(algorithm);

    // create dirs used by aion
    cmd.dirs.create_dirs()?;

    // run in daemon mode
    if let Some(pid_file) = cmd.daemon {
        daemonize(pid_file)?;
    }

    //print out running aion environment
    print_running_environment(&cmd.spec, &spec.data_dir, &cmd.dirs, &db_dirs);

    print_logo();

    let passwords = passwords_from_files(&cmd.acc_conf.password_files)?;

    // prepare account provider
    let account_provider = Arc::new(prepare_account_provider(
        &cmd.spec,
        &cmd.dirs,
        &spec.data_dir,
        cmd.acc_conf,
        &passwords,
    )?);

    let tx_status_service = IoService::<TxIoMessage>::start()
        .map_err(|e| format!("tx status server start failed : {}", e))?;
    let tx_status_channel = if cmd.wallet_api_conf.enabled {
        tx_status_service.channel()
    } else {
        IoChannel::disconnected()
    };
    // create miner
    let miner = Miner::new(
        cmd.miner_options,
        &spec,
        Some(account_provider.clone()),
        tx_status_channel,
    );
    miner.set_author(cmd.miner_extras.author);
    miner.set_gas_floor_target(cmd.miner_extras.gas_floor_target);
    miner.set_gas_ceil_target(cmd.miner_extras.gas_ceil_target);
    miner.set_extra_data(cmd.miner_extras.extra_data);
    // create client config
    let mut client_config = to_client_config(
        &cmd.cache_config,
        spec.name.to_lowercase(),
        fat_db,
        cmd.compaction,
        cmd.wal,
        cmd.vm_type,
        algorithm,
        cmd.pruning_history,
        cmd.pruning_memory,
        cmd.check_seal,
    );

    client_config.queue.verifier_settings = cmd.verifier_settings;

    // set up bootnodes
    let net_conf = cmd.net_conf;

    // create client service.
    let service = ClientService::start(
        client_config,
        &spec,
        &client_path,
        &cmd.dirs.ipc_path(),
        miner.clone(),
    )
    .map_err(|e| format!("Client service error: {:?}", e))?;

    info!(target: "run","Genesis hash: {:?}",genesis_hash);

    // display info about used pruning algorithm
    info!(
        target: "run",
        "State DB configuration: {}{}",
        Colour::White.bold().paint(algorithm.as_str()),
        match fat_db {
            true => Colour::White.bold().paint(" +Fat").to_string(),
            false => "".to_owned(),
        }
    );

    // display warning about using experimental journaldb algorithm
    if !algorithm.is_stable() {
        warn!(
            target: "run",
            "Your chosen strategy is {}! You can re-run with --pruning to change.",
            Colour::Red.bold().paint("unstable")
        );
    }

    if !cmd.wallet_api_conf.enabled {
        info!(target: "run", "Wallet API is disabled.");
    }

    let client = service.client();
    let header_chain = Arc::new(HeaderChain::new(&client_path, &spec).expect("Invalid db..."));

    // initialize the local node information store.
    let store = {
        let db = service.db();
        let node_info = FullNodeInfo {
            miner: match cmd.no_persistent_txqueue {
                true => None,
                false => Some(miner.clone()),
            },
        };

        let store = ::local_store::create(db, ::acore::db::COL_NODE_INFO, node_info);

        if cmd.no_persistent_txqueue {
            info!(target: "run","Running without a persistent transaction queue.");

            if let Err(e) = store.clear() {
                warn!(target: "run","Error clearing persistent transaction queue: {}", e);
            }
        }

        // re-queue pending transactions.
        match store.pending_transactions() {
            Ok(pending) => {
                let len = pending.len();
                if len > 0 {
                    info!(target: "run","Importing the local pending transactions ...");
                    for pending_tx in pending {
                        if let Err(e) = miner.import_own_transaction(&*client, pending_tx) {
                            warn!(target: "run","Error importing saved transaction: {}", e)
                        }
                    }
                    info!(target: "run","Import completed, total = {}", len);
                }
            }
            Err(e) => {
                warn!(target: "run","Error loading cached pending transactions from disk: {}", e)
            }
        }

        Arc::new(store)
    };

    // register it as an IO service to update periodically.
    service
        .register_io_handler(store)
        .map_err(|_| "Unable to register local store handler".to_owned())?;

    // create external miner
    let external_miner = Arc::new(ExternalMiner::default());

    // start stratum
    if cmd.stratum.enable {
        Stratum::register(&cmd.stratum, miner.clone(), Arc::downgrade(&client))
            .map_err(|e| format!("Stratum start error: {:?}", e))?;
    }

    // create sync object
    let sync_params = Params {
        config: SyncConfig::default(),
        client: client.clone(),
        network_config: net_conf,
        header_chain: header_chain,
    };

    let (sync_provider, network_manager, chain_notify) =
        modules::sync(sync_params).map_err(|e| format!("Sync error: {}", e))?;

    service.add_notify(chain_notify.clone());

    // drop the spec to free up genesis state.
    // drop(spec);

    // spin up rpc eventloop
    let runtime_rpc = tokio::runtime::Builder::new()
        .name_prefix("rpc-eventloop-")
        .build()
        .expect("runtime_rpc init failed");
    // set up dependencies for rpc servers
    let rpc_stats = Arc::new(informant::RpcStats::default());
    let account_store = Some(account_provider.clone());
    let pb_client = EthClient::new(
        &client.clone(),
        &sync_provider.clone(),
        &account_store,
        &miner.clone(),
        &external_miner.clone(),
        cmd.dynamic_gas_price.clone(),
    );

    // start pb server
    let pb_handles = Arc::new(pb_client);
    let pb_server = new_pb(cmd.wallet_api_conf, pb_handles, tx_status_service)?;
    let deps_for_rpc_apis = Arc::new(rpc_apis::FullDependencies {
        client: client.clone(),
        sync: sync_provider.clone(),
        account_store,
        miner: miner.clone(),
        external_miner: external_miner.clone(),
        dynamic_gas_price: cmd.dynamic_gas_price.clone(),
        executor: runtime_rpc.executor(),
    });

    let dependencies = rpc::Dependencies {
        apis: deps_for_rpc_apis.clone(),
        stats: rpc_stats.clone(),
    };

    let runtime_jsonrpc = {
        if cmd.http_conf.enabled && cmd.http_conf.processing_threads > num_cpus::get() {
            warn!(target: "run","jsonrpc processing threads is greater than num of cpus");
        }
        tokio::runtime::Builder::new()
            .core_threads(cmd.http_conf.processing_threads)
            .name_prefix("jsonrpc_eventloop-")
            .build()
            .map_err(|_| format!("can't spawn jsonrpc eventloop"))?
    };
    let executor_jsonrpc = runtime_jsonrpc.executor();
    // start rpc servers
    let ws_server = rpc::new_ws(cmd.ws_conf.clone(), &dependencies, executor_jsonrpc.clone())?;
    let ipc_server = rpc::new_ipc(cmd.ipc_conf, &dependencies, executor_jsonrpc.clone())?;
    let http_server = rpc::new_http(
        "HTTP JSON-RPC",
        "jsonrpc",
        cmd.http_conf.clone(),
        &dependencies,
        executor_jsonrpc.clone(),
    )?;

    // save user defaults
    user_defaults.is_first_launch = false;
    user_defaults.pruning = algorithm;
    user_defaults.fat_db = fat_db;
    user_defaults.save(&user_defaults_path)?;

    // start miner module
    // let runtime_miner = tokio::runtime::Builder::new()
    //     .core_threads(1)
    //     .name_prefix("seal-block-loop #")
    //     .build()
    //     .expect("seal block runtime loop init failed");
    // let executor_miner = runtime_miner.executor();
    // let close = run_miner(executor_miner.clone(), client.clone());

    // enable Sync module
    network_manager.start_network();
    if let Some(config_path) = cmd.dirs.config.clone() {
        fill_back_local_node(config_path);
    }

    // Create a weak reference to the client so that we can wait on shutdown until it is dropped
    let weak_client = Arc::downgrade(&client);

    // Handle exit
    wait_for_exit();

    // let _ = close.send(());

    info!(target: "run","Finishing work, please wait...");

    ws_server.expect("Invalid WS server instance!").close();
    http_server.expect("Invalid HTTP server instance!").close();
    ipc_server.expect("Invalid IPC server instance!").close();

    if let Some(config_path) = cmd.dirs.config {
        //        if !net_conf.sync_from_boot_nodes_only {
        fill_back_boot_nodes(config_path);
        //        }
    }
    network_manager.stop_network();

    // close/drop this stuff as soon as exit detected.
    drop((sync_provider, network_manager, chain_notify, pb_server));

    thread::sleep(Duration::from_secs(5));

    runtime_rpc
        .shutdown_now()
        .wait()
        .expect("Failed to shutdown rpc runtime instance!");
    runtime_jsonrpc
        .shutdown_now()
        .wait()
        .expect("Failed to shutdown jsonrpc runtime instance!");

    info!(target: "run","Shutdown.");

    Ok(weak_client)
}

pub fn execute(cmd: RunCmd) -> Result<(), String> {
    // increase max number of open files
    raise_fd_limit();

    fn wait<T>(res: Result<Weak<T>, String>) -> Result<(), String> {
        res.map(|weak_client| {
            wait_for_drop(weak_client);
        })
    }
    wait(execute_impl(cmd))
}

#[cfg(not(windows))]
fn daemonize(pid_file: String) -> Result<(), String> {
    extern crate daemonize;

    daemonize::Daemonize::new()
        .pid_file(pid_file)
        .chown_pid_file(true)
        .start()
        .map(|_| ())
        .map_err(|e| format!("Couldn't daemonize; {}", e))
}

#[cfg(windows)]
fn daemonize(_pid_file: String) -> Result<(), String> {
    Err("daemon is no supported on windows".into())
}

fn print_running_environment(
    spec: &SpecType,
    spec_data_dir: &String,
    dirs: &Directories,
    db_dirs: &DatabaseDirectories,
)
{
    if let Some(config) = &dirs.config {
        info!(
            target: "run",
            "Config path {}",
            Colour::White
                .bold()
                .paint(config)
        );
    } else {
        info!(target: "run", "Start without config.");
    }
    match spec {
        SpecType::Foundation => {
            info!(target: "run", "Load built-in Mainnet Genesis Spec.");
        }
        SpecType::Custom(ref filename) => {
            info!(
                target: "run",
                "Genesis spec path {}",
            Colour::White
                .bold()
                .paint(absolute(filename.to_string()))
            );
        }
    }
    info!(
        target: "run",
        "Keys path {}",
        Colour::White
            .bold()
            .paint(dirs.keys_path(spec_data_dir).to_string_lossy().into_owned())
    );
    info!(
        target: "run",
        "DB path {}",
        Colour::White
            .bold()
            .paint(db_dirs.db_root_path().to_string_lossy().into_owned())
    );
}

fn print_logo() {
    info!(
        target: "run",
        "{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
        Colour::Blue
            .bold()
            .paint("\n             _____    ____    _   _ \n"),
        Colour::Blue.bold().paint("     /\\     |_   _|  / __ "),
        Colour::Green.bold().paint("\\"),
        Colour::Blue.bold().paint("  | \\ | |\n"),
        Colour::Blue
            .bold()
            .paint("    /  \\      | |   | |  | | |  \\| |\n"),
        Colour::Blue.bold().paint("   / /\\ \\     | |   "),
        Colour::Green.bold().paint("|"),
        Colour::Blue.bold().paint(" |  | | | . ` |\n"),
        Colour::Blue.bold().paint("  / ____ \\   _| |_  "),
        Colour::Green.bold().paint("|"),
        Colour::Blue.bold().paint(" |__| | | |\\  |\n"),
        Colour::Blue.bold().paint(" /_/    \\_\\ |_____|  "),
        Colour::Green.bold().paint("\\"),
        Colour::Blue.bold().paint("____/  |_| \\_|\n\n")
    );
    info!(target: "run","Starting {}", Colour::White.bold().paint(version()));
}

fn prepare_account_provider(
    spec: &SpecType,
    dirs: &Directories,
    data_dir: &str,
    cfg: AccountsConfig,
    passwords: &[String],
) -> Result<AccountProvider, String>
{
    use acore::keychain::accounts_dir::RootDiskDirectory;
    use acore::keychain::EthStore;

    let path = dirs.keys_path(data_dir);
    let dir = Box::new(
        RootDiskDirectory::create(&path)
            .map_err(|e| format!("Could not open keys directory: {}", e))?,
    );
    let account_settings = AccountProviderSettings {
        unlock_keep_secret: cfg.enable_fast_unlock,
        blacklisted_accounts: vec![
            // blacklist accounts for development. since we change account address to 32 bytes,
            // so just append zero to keep it work.
            "00a329c0648769a73afac7f9381e08fb43dbea72000000000000000000000000".into(),
        ],
    };

    let ethstore = EthStore::open_with_iterations(dir, cfg.iterations)
        .map_err(|e| format!("Could not open keys directory: {}", e))?;
    if cfg.refresh_time > 0 {
        ethstore.set_refresh_time(::std::time::Duration::from_secs(cfg.refresh_time));
    }
    let account_provider = AccountProvider::new(Box::new(ethstore), account_settings);

    for a in cfg.unlocked_accounts {
        // Check if the account exists
        if !account_provider.has_account(a).unwrap_or(false) {
            return Err(format!(
                "Account {} not found for the current chain. {}",
                a,
                build_create_account_hint(spec, &dirs.keys)
            ));
        }

        // Check if any passwords have been read from the password file(s)
        if passwords.is_empty() {
            return Err(format!(
                "No password found to unlock account {}. {}",
                a, VERIFY_PASSWORD_HINT
            ));
        }

        if !passwords.iter().any(|p| {
            account_provider
                .unlock_account_permanently(a, (*p).clone())
                .is_ok()
        }) {
            return Err(format!(
                "No valid password to unlock account {}. {}",
                a, VERIFY_PASSWORD_HINT
            ));
        }
    }

    Ok(account_provider)
}

// Construct an error `String` with an adaptive hint on how to create an account.
fn build_create_account_hint(spec: &SpecType, keys: &str) -> String {
    format!(
        "You can create an account via RPC, UI or `aion account new --chain {} --keys-path {}`.",
        spec, keys
    )
}

fn fill_back_local_node(path: String) {
    let local_node_info = P2pMgr::get_local_node().get_node_string();
    let file = fs::File::open(&path).expect("Cannot open config file");
    let reader = BufReader::new(file);
    let mut no_change = true;
    let mut ret: String = reader
        .lines()
        .filter_map(|l| l.ok())
        .map(|config| {
            let config_ = config.clone().to_owned();
            let option: Vec<&str> = config_.split("=").collect();
            if option[0].trim() == "local_node" && option[1]
                .find("00000000-0000-0000-0000-000000000000")
                .is_some()
            {
                no_change = false;
                format!("local_node = {:?}", local_node_info)
            } else {
                config.trim().into()
            }
        })
        .collect::<Vec<String>>()
        .join("\n");
    if ret.find("\nlocal_node").is_none() {
        if let Some(index) = ret.find("[network]\n") {
            ret.insert_str(index + 10, &format!("local_node = {:?}\n", local_node_info));
        } else {
            ret.insert_str(
                0,
                &format!("[network]\nlocal_node = {:?}\n\n", local_node_info),
            );
        }
    } else if no_change {
        return;
    }
    let _ = fs::write(&path, ret).expect("Rewrite failed");
    info!(target: "run","Local node fill back!");
}

fn fill_back_boot_nodes(path: String) {
    let boot_nodes: Vec<String> = P2pMgr::get_top8_node_hashes()
        .iter()
        .filter_map(|hash| P2pMgr::get_node(*hash))
        .map(|x| x.get_node_string())
        .collect();

    let file = fs::File::open(&path).expect("Cannot open config file");
    let reader = BufReader::new(file);
    let mut ret: String = reader
        .lines()
        .filter_map(|l| l.ok())
        .map(|s| s.trim().into())
        .collect::<Vec<String>>()
        .join("\n");
    if let Some(index) = ret.find("\nboot_nodes") {
        let ret_clone = ret.clone();
        let (left, rest) = ret_clone.split_at(index);
        let (_, right) = rest.split_at(rest.find("]").unwrap() + 1);
        ret = format!("{}\nboot_nodes = {:?}{}", left, boot_nodes, right);
    } else {
        if let Some(index) = ret.find("[network]\n") {
            ret.insert_str(index + 10, &format!("boot_nodes = {:?}\n", boot_nodes));
        } else {
            ret.insert_str(0, &format!("[network]\nboot_nodes = {:?}\n\n", boot_nodes));
        }
    }
    let _ = fs::write(&path, ret).expect("Rewrite failed");
    info!(target: "run","Boot nodes fill back!");
}

fn wait_for_exit() {
    let exit = Arc::new((Mutex::new(false), Condvar::new()));

    // TODO： Perfect end, ensure resource release
    // Handle possible exits
    let e = exit.clone();
    CtrlC::set_handler(move || {
        e.1.notify_all();
    });

    // Wait for signal
    let mut l = exit.0.lock();
    let _ = exit.1.wait(&mut l);
}

fn wait_for_drop<T>(w: Weak<T>) {
    let sleep_duration = Duration::from_secs(1);
    let warn_timeout = Duration::from_secs(60);
    let max_timeout = Duration::from_secs(300);

    let instant = Instant::now();
    let mut warned = false;

    while instant.elapsed() < max_timeout {
        if w.upgrade().is_none() {
            return;
        }

        if !warned && instant.elapsed() > warn_timeout {
            warned = true;
            warn!(target: "run","Shutdown is taking longer than expected.");
        }

        thread::sleep(sleep_duration);
    }

    warn!(target: "run","Shutdown timeout reached, exiting uncleanly.");
}
