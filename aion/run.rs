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

use acore::account_provider::{AccountProvider, AccountProviderSettings};
use acore::client::{Client, DatabaseCompactionProfile, VMType, ChainNotify};
use acore::miner::external::ExternalMiner;
use acore::miner::{Miner, MinerOptions, MinerService};
use acore::service::{ClientService, run_miner,
run_staker, run_transaction_pool};
use acore::verification::queue::VerifierSettings;
use acore::sync::Sync;
use aion_rpc::{dispatch::DynamicGasPrice, informant};
use aion_version::version;
use ansi_term::Colour;
use cache::CacheConfig;
use ctrlc::CtrlC;
use dir::{DatabaseDirectories, Directories};
use fdlimit::raise_fd_limit;
use helpers::{passwords_from_files, to_client_config};
use dir::helpers::absolute;
use io::IoChannel;
use logger::LogConfig;
use tokio;
use tokio::prelude::*;
use num_cpus;
use params::{fatdb_switch_to_bool, AccountsConfig, StakeConfig, MinerExtras, Pruning, SpecType, Switch};
use parking_lot::{Condvar, Mutex};
use rpc;
use rpc_apis;

// chris
use p2p::Config;
use p2p::Mgr;

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
    pub net_conf: Config,
    pub acc_conf: AccountsConfig,
    pub stake_conf: StakeConfig,
    pub miner_extras: MinerExtras,
    pub fat_db: Switch,
    pub compaction: DatabaseCompactionProfile,
    pub wal: bool,
    pub vm_type: VMType,
    pub verifier_settings: VerifierSettings,
    pub no_persistent_txqueue: bool,
}

pub fn execute_impl(cmd: RunCmd) -> Result<(Weak<Client>), String> {
    // load spec
    let spec = cmd.spec.spec()?;

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

    print_logo();

    print_running_environment(&cmd.spec, &spec.data_dir, &cmd.dirs, &db_dirs);

    let passwords = passwords_from_files(&cmd.acc_conf.password_files)?;

    // prepare account provider
    let account_provider = Arc::new(prepare_account_provider(
        &cmd.spec,
        &cmd.dirs,
        &spec.data_dir,
        cmd.acc_conf,
        &passwords,
    )?);

    let tx_status_channel = IoChannel::disconnected();

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
    );

    client_config.queue.verifier_settings = cmd.verifier_settings;
    client_config.stake_contract = cmd.stake_conf.contract;

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

    info!(target: "run","     genesis: {:?}",genesis_hash);

    // display info about used pruning algorithm
    info!(
        target: "run",
        "    state db: {}{}",
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

    let client = service.client();

    // drop the spec to free up genesis state.
    drop(spec);

    // create external miner
    let external_miner = Arc::new(ExternalMiner::default());

    // log apis
    info!(target: "run", "        apis: rpc-http({}) rpc-ws({}) rpc-ipc({})",
          if cmd.http_conf.enabled { "y" } else { "n" },
          if cmd.ws_conf.enabled { "y" } else { "n" },
          if cmd.ipc_conf.enabled { "y" } else { "n" },
    );

    // chris
    // start p2p
    let p2p_mgr = Mgr::new(net_conf.clone());
    p2p_mgr.run();

    // start sync
    let sync = Sync::new(client.clone(), net_conf);
    // let chain_notify = sync.clone() as Arc<ChainNotify>;
    // let chain_notify = Arc::new(sync.clone()) as Arc<ChainNotify>;
    // service.add_notify(chain_notify.clone());
    // chris
    //sync.start_network();

    // start rpc server
    let runtime_rpc = tokio::runtime::Builder::new()
        .name_prefix("rpc-")
        .build()
        .expect("runtime_rpc init failed");
    let rpc_stats = Arc::new(informant::RpcStats::default());
    let account_store = Some(account_provider.clone());

    // chris
    let deps_for_rpc_apis = Arc::new(rpc_apis::FullDependencies {
        client: client.clone(),
        //sync: sync.clone()),
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
        if cmd.http_conf.processing_threads > num_cpus::get() {
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
    let ipc_server = rpc::new_ipc(
        cmd.ipc_conf.clone(),
        &dependencies,
        executor_jsonrpc.clone(),
    )?;
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
    let runtime_transaction_pool = tokio::runtime::Builder::new()
        .core_threads(1)
        .name_prefix("transaction-pool-loop #")
        .build()
        .expect("seal block runtime loop init failed");
    let executor_transaction_pool = runtime_transaction_pool.executor();
    let close_transaction_pool =
        run_transaction_pool(executor_transaction_pool.clone(), client.clone());

    // start miner module
    let runtime_miner = tokio::runtime::Builder::new()
        .core_threads(1)
        .name_prefix("seal-block-loop #")
        .build()
        .expect("seal block runtime loop init failed");
    let executor_miner = runtime_miner.executor();
    let close_miner = run_miner(executor_miner.clone(), client.clone());

    // start internal staker module
    let runtime_staker = tokio::runtime::Builder::new()
        .core_threads(1)
        .name_prefix("seal-block-loop #")
        .build()
        .expect("internal staker runtime loop init failed");
    let executor_staker = runtime_staker.executor();
    let close_staker = run_staker(executor_staker.clone(), client.clone());

    // Create a weak reference to the client so that we can wait on shutdown until it is dropped
    let weak_client = Arc::downgrade(&client);

    // Handle exit
    wait_for_exit();

    info!(target: "run","Finishing work, please wait...");

    // close pool
    let _ = close_transaction_pool.send(());
    let _ = close_miner.send(());
    let _ = close_staker.send(());

    // close rpc
    if ws_server.is_some() {
        ws_server.unwrap().close();
    }
    if http_server.is_some() {
        http_server.unwrap().close();
    }
    if ipc_server.is_some() {
        ipc_server.unwrap().close();
    }

    // chris
    // close p2p
    p2p_mgr.shutdown();
    // sync.stop_network();

    // close/drop this stuff as soon as exit detected.
    // drop((sync, chain_notify));
    drop(sync);


    thread::sleep(Duration::from_secs(5));

    runtime_rpc
        .shutdown_now()
        .wait()
        .expect("Failed to shutdown rpc runtime instance!");
    runtime_jsonrpc
        .shutdown_now()
        .wait()
        .expect("Failed to shutdown json rpc runtime instance!");
    runtime_transaction_pool
        .shutdown_now()
        .wait()
        .expect("Failed to shutdown transaction pool runtime instance!");
    runtime_miner
        .shutdown_now()
        .wait()
        .expect("Failed to shutdown miner runtime instance!");
    runtime_staker
        .shutdown_now()
        .wait()
        .expect("Failed to shutdown internal staker runtime instance!");

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
            " config path: {}",
            Colour::White
                .bold()
                .paint(config)
        );
    } else {
        info!(target: "run", "Start without config.");
    }
    match spec {
        SpecType::Default => {
            info!(target: "run", "Load built-in Mainnet Genesis Spec.");
        }
        SpecType::Custom(ref filename) => {
            info!(
                target: "run",
                "genesis path: {}",
            Colour::White
                .bold()
                .paint(absolute(filename.to_string()))
            );
        }
    }
    info!(
        target: "run",
        "   keys path: {}",
        Colour::White
            .bold()
            .paint(dirs.keys_path(spec_data_dir).to_string_lossy().into_owned())
    );
    info!(
        target: "run",
        "     db path: {}",
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
    info!(target: "run","       build: {}", Colour::White.bold().paint(version()));
}

fn prepare_account_provider(
    spec: &SpecType,
    dirs: &Directories,
    data_dir: &str,
    cfg: AccountsConfig,
    passwords: &[String],
) -> Result<AccountProvider, String>
{
    use keychain::accounts_dir::RootDiskDirectory;
    use keychain::EthStore;

    let path = dirs.keys_path(data_dir);
    let dir = Box::new(
        RootDiskDirectory::create(&path)
            .map_err(|e| format!("Could not open keys directory: {}", e))?,
    );
    let account_settings = AccountProviderSettings {
        unlock_keep_secret: cfg.enable_fast_signing,
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
        if !account_provider.has_account(&a).unwrap_or(false) {
            return Err(format!(
                "Account {} not found for the current chain. {}",
                &a,
                build_create_account_hint(spec, &dirs.keys)
            ));
        }

        // Check if any passwords have been read from the password file(s)
        if passwords.is_empty() {
            return Err(format!(
                "No password found to unlock account {}. {}",
                &a, VERIFY_PASSWORD_HINT
            ));
        }

        if !passwords.iter().any(|p| {
            account_provider
                .unlock_account_permanently(&a, (*p).clone())
                .is_ok()
        }) {
            return Err(format!(
                "No valid password to unlock account {}. {}",
                &a, VERIFY_PASSWORD_HINT
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
