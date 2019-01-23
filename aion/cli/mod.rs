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

#[macro_use]
mod usage;
mod group;

usage! {
    {
        // CLI subcommands
        // Subcommands must start with cmd_ and have '_' in place of '-'
        // Sub-subcommands must start with the name of the subcommand
        // Arguments must start with arg_
        // Flags must start with flag_

        CMD cmd_daemon
        {
            "Use Aion as a daemon",

            ARG arg_daemon_pid_file: (Option<String>) = None,
            "<PID-FILE>",
            "Path to the pid file",
        }

        CMD cmd_account
        {
            "Manage accounts",

            CMD cmd_account_new {
                "Create a new account",
            }

            CMD cmd_account_list {
                "List existing accounts",
            }

            CMD cmd_account_import
            {
                "Import account",

                ARG arg_account_import_path : (Option<Vec<String>>) = None,
                "<PATH>...",
                "Path to the accounts",
            }

            CMD cmd_account_import_by_key
            {
                "Import account by private key",
                ARG arg_account_private_key: (Option<String>) = None,
                "<key>",
                "account private key",
            }

            CMD cmd_account_export_to_key
            {
                "Export account to private key",
                ARG arg_account_address: (Option<String>) = None,
                "<address>",
                "account address",
            }
        }

        CMD cmd_import
        {
            "Import blockchain",

            ARG arg_import_format: (Option<String>) = None,
            "--format=[FORMAT]",
            "Import in a given format. FORMAT must be either 'hex' or 'binary'. (default: auto)",

            ARG arg_import_file: (Option<String>) = None,
            "<FILE>",
            "Path to the file to import from",
        }

        CMD cmd_export
        {
            "Export blockchain",

            ARG arg_export_blocks_format: (Option<String>) = None,
            "--format=[FORMAT]",
            "Export in a given format. FORMAT must be either 'hex' or 'binary'. (default: binary)",

            ARG arg_export_blocks_from: (String) = "1",
            "--from=[BLOCK]",
            "Export from block BLOCK, which may be an index or hash.",

            ARG arg_export_blocks_to: (String) = "latest",
            "--to=[BLOCK]",
            "Export to (including) block BLOCK, which may be an index, hash or latest.",

            ARG arg_export_blocks_file: (Option<String>) = None,
            "[FILE]",
            "Path to the exported file",
        }

        CMD cmd_revert
        {
            "Revert blockchain",

            ARG arg_revert_blocks_to: (String) = "0",
            "--to=[BLOCK]",
            "Revert Database to (including) block BLOCK, which may be an index, hash.",
        }

        CMD cmd_db
        {
            "Manage the database representing the state of the blockchain on this system",

            CMD cmd_db_kill {
                "Clean the database",
            }
        }
    }
    {
        // Global flags and arguments
        ["Operating Options"]
            ARG arg_chain: (String) = "mainnet", or |c: &Config| c.aion.as_ref()?.chain.clone(),
            "--chain=[CHAIN]",
            "Specify the blockchain type. CHAIN may be a JSON chain specification file.",

            ARG arg_keys_path: (Option<String>) = None, or |c: &Config| c.aion.as_ref()?.keys_path.clone(),
            "--keys-path=[PATH]",
            "Specify the path for JSON key files to be found",

            ARG arg_base_path: (Option<String>) = None, or |c: &Config| c.aion.as_ref()?.base_path.clone(),
            "-d, --base-path=[PATH]",
            "Specify the base data storage path.",

            ARG arg_db_path: (Option<String>) = None, or |c: &Config| c.aion.as_ref()?.db_path.clone(),
            "--db-path=[PATH]",
            "Specify the database directory path",

        ["Miscellaneous Options"]
            FLAG flag_full_help: (bool) = false, or |_| None,
            "--full-help",
            "Show full help information.",

            FLAG flag_version: (bool) = false, or |_| None,
            "-v, --version",
            "Show information about version.",

            FLAG flag_no_config: (bool) = false, or |_| None,
            "--no-config",
            "Don't load a configuration file.",

            FLAG flag_default_config: (bool) = false, or |_| None,
            "--default-config",
            "Print DEFAULT configuration to $HOME/.aion/default_config.toml.",

            FLAG flag_no_seal_check: (bool) = false, or |_| None,
            "--no-seal-check",
            "Skip block seal check. Used to make import and export blocks faster, if checking seal is not necessary.",

            ARG arg_config: (String) = "$HOME/.aion/config.toml", or |_| None,
            "-c, --config=[CONFIG]",
            "Specify a configuration. CONFIG may be a configuration file .",

        ["Account Options"]
            FLAG flag_fast_signing: (bool) = false, or |c: &Config| c.account.as_ref()?.fast_signing.clone(),
            "--fast-signing",
            "Use drastically faster signing mode for permanently unlocked accounts. This setting causes raw secrets of these accounts to be stored unprotected in memory, so use with care.",

            ARG arg_keys_iterations: (u32) = 10240u32, or |c: &Config| c.account.as_ref()?.keys_iterations.clone(),
            "--keys-iterations=[NUM]",
            "Specify the number of iterations to use when deriving key from the password (bigger is more secure)",

            ARG arg_refresh_time: (u64) = 5u64, or |c: &Config| c.account.as_ref()?.refresh_time.clone(),
            "--accounts-refresh=[TIME]",
            "Specify the cache time of accounts read from disk. If you manage thousands of accounts set this to 0 to disable refresh.",

            ARG arg_unlock: (Vec<String>) = Vec::new(), or |c: &Config| c.account.as_ref()?.unlock.clone(),
            "--unlock=[ACCOUNTS]...",
            "Unlock ACCOUNTS for the duration of the execution. ACCOUNTS is a comma-delimited list of addresses.",

            ARG arg_password: (Vec<String>) = Vec::new(), or |c: &Config| c.account.as_ref()?.password.clone(),
            "--password=[FILE]...",
            "Provide a list of files containing passwords for unlocking accounts. Leading and trailing whitespace is trimmed.",

        ["Network Options"]
            FLAG flag_sync_from_boot_nodes_only: (bool) = false, or |c: &Config| c.network.as_ref()?.sync_from_boot_nodes_only.clone(),
            "--sync-boot-nodes-only",
            "Indicates if only sync from bootnodes.",

            ARG arg_max_peers: (u32) = 64u32, or |c: &Config| c.network.as_ref()?.max_peers.clone(),
            "--max-peers=[NUM]",
            "Allow up to NUM peers.",

            ARG arg_net_id: (u32) = 256u32, or |c: &Config| c.network.as_ref()?.net_id.clone(),
            "--net-id=[INDEX]",
            "Override the network identifier from the chain we are on.",

            ARG arg_local_node: (String) = "p2p://00000000-0000-0000-0000-000000000000@0.0.0.0:30303", or |c: &Config| c.network.as_ref()?.local_node.clone(),
            "--local-node=[NODE]",
            "Override the local node. NODE should be a p2p node.",

            ARG arg_boot_nodes: (Vec<String>) = vec!["p2p://c33d1066-8c7e-496c-9c4e-c89318280274@13.92.155.115:30303".into(), "p2p://c33d2207-729a-4584-86f1-e19ab97cf9ce@51.144.42.220:30303".into(), "p2p://c33d302f-216b-47d4-ac44-5d8181b56e7e@52.231.187.227:30303".into(), "p2p://c33d4c07-6a29-4ca6-8b06-b2781ba7f9bf@191.232.164.119:30303".into(), "p2p://c33d5a94-20d8-49d9-97d6-284f88da5c21@13.89.244.125:30303".into(), "p2p://741b979e-6a06-493a-a1f2-693cafd37083@66.207.217.190:30303".into()]
, or |c: &Config| c.network.as_ref()?.boot_nodes.clone(),
            "--boot-nodes=[NODES]...",
            "Override the boot nodes from our chain. NODES should be p2p nodes.",

            ARG arg_ip_black_list: (Vec<String>) = Vec::new(), or |c: &Config| c.network.as_ref()?.ip_black_list.clone(),
            "--black_ip_list=[IPs]",
            "IP list whose connecting requests are to be rejected.",

        ["Rpc Options"]
            ARG arg_rpc_processing_threads: (Option<usize>) = None, or |c: &Config| c.rpc.as_ref()?.processing_threads,
            "--rpc--processing-threads=[NUM]",
            "Turn on additional processing threads for JSON-RPC servers (for all severs http, websocket and ipc). Setting this to a non-zero value allows parallel execution of cpu-heavy queries.",

        ["Http Options"]
            FLAG flag_no_http: (bool) = false, or |c: &Config| c.http.as_ref()?.disable.clone(),
            "--no-http",
            "Disable the HTTP API server.",

            ARG arg_http_port: (u16) = 8545u16, or |c: &Config| c.http.as_ref()?.port.clone(),
            "--http-port=[PORT]",
            "Specify the port portion of the HTTP API server.",

            ARG arg_http_interface: (String)  = "local", or |c: &Config| c.http.as_ref()?.interface.clone(),
            "--http-interface=[IP]",
            "Specify the hostname portion of the HTTP API server, IP should be an interface's IP address, or all (all interfaces) or local.",

            ARG arg_http_apis: (Vec<String>) = vec!["all".into(),"-pubsub".into()], or |c: &Config| c.http.as_ref()?.apis.clone(),
            "--http-apis=[APIS]...",
            "Specify the APIs available through the HTTP interface. APIS is a comma-delimited list of API name. Possible name are all, web3, eth, stratum, net, personal, rpc. You can also disable a specific API by putting '-' in the front: all,-personal.NOTE that rpc doesn’t support pubsub",

            ARG arg_http_hosts: (Vec<String>) = vec!["none".into()], or |c: &Config| c.http.as_ref()?.hosts.clone(),
            "--http-hosts=[HOSTS]...",
            "List of allowed Host header values. This option will validate the Host header sent by the browser, it is additional security against some attack vectors. Special options: \"all\", \"none\",.",

            ARG arg_http_cors: (Vec<String>) = vec!["none".into()], or |c: &Config| c.http.as_ref()?.cors.clone(),
            "--http-cors=[URL]...",
            "Specify CORS header for HTTP JSON-RPC API responses. Special options: \"all\", \"none\".",

            ARG arg_http_server_threads: (Option<usize>) = None, or |c: &Config| c.http.as_ref()?.server_threads,
            "--http-server-threads=[NUM]",
            "Enables multiple threads handling incoming connections for HTTP JSON-RPC server.",

        ["WebSockets Options"]
            FLAG flag_no_ws: (bool) = false, or |c: &Config| c.websockets.as_ref()?.disable.clone(),
            "--no-ws",
            "Disable the WebSockets server.",

            ARG arg_ws_port: (u16) = 8546u16, or |c: &Config| c.websockets.as_ref()?.port.clone(),
            "--ws-port=[PORT]",
            "Specify the port portion of the WebSockets server.",

            ARG arg_ws_interface: (String)  = "local", or |c: &Config| c.websockets.as_ref()?.interface.clone(),
            "--ws-interface=[IP]",
            "Specify the hostname portion of the WebSockets server, IP should be an interface's IP address, or all (all interfaces) or local.",

            ARG arg_ws_apis: (Vec<String>) = vec!["all".into(),"-pubsub".into()], or |c: &Config| c.websockets.as_ref()?.apis.clone(),
            "--ws-apis=[APIS]...",
            "Specify the APIs available through the WebSockets interface. APIS is a comma-delimited list of API name. Possible name are web3, eth, stratum, net, personal, rpc, pubsub.",

            ARG arg_ws_origins: (Vec<String>) = vec!["none".into()], or |c: &Config| c.websockets.as_ref()?.origins.clone(),
            "--ws-origins=[URL]...",
            "Specify Origin header values allowed to connect. Special options: \"all\", \"none\".",

            ARG arg_ws_hosts: (Vec<String>) = vec!["none".into()], or |c: &Config| c.websockets.as_ref()?.hosts.clone(),
            "--ws-hosts=[HOSTS]...",
            "List of allowed Host header values. This option will validate the Host header sent by the browser, it is additional security against some attack vectors. Special options: \"all\", \"none\".",

            ARG arg_ws_max_connections: (usize) = 100usize, or |c: &Config| c.websockets.as_ref()?.max_connections.clone(),
            "--ws-max-connections=[CONN]",
            "Maximum number of allowed concurrent WebSockets JSON-RPC connections.",

        ["IPC Options"]
            FLAG flag_no_ipc: (bool) = false, or |c: &Config| c.ipc.as_ref()?.disable.clone(),
            "--no-ipc",
            "Disable JSON-RPC over IPC service.",

            ARG arg_ipc_path: (String) = if cfg!(windows) { r"\\.\pipe\jsonrpc.ipc" } else { "$BASE/jsonrpc.ipc" }, or |c: &Config| c.ipc.as_ref()?.path.clone(),
            "--ipc-path=[PATH]",
            "Specify custom path for JSON-RPC over IPC service.",

            ARG arg_ipc_apis: (Vec<String>) = vec!["all".into(),"-pubsub".into()], or |c: &Config| c.ipc.as_ref()?.apis.clone(),
            "--ipc-apis=[APIS]...",
            "Specify custom API set available via JSON-RPC over IPC. Possible name are web3, eth, stratum, net, personal, rpc, pubsub.",

        ["Wallet Options"]
            FLAG flag_enable_wallet: (bool) = false, or |c: &Config| c.wallet.as_ref()?.disable.clone().map(|a| !a),
            "--enable-wallet",
            "Enable Wallet API",

            FLAG flag_secure_connect: (bool) = false, or |c: &Config| c.wallet.as_ref()?.secure_connect.clone(),
            "--secure-connect",
            "Run wallet server for secure connect",

            ARG arg_wallet_interface: (String) = "local", or |c: &Config| c.wallet.as_ref()?.interface.clone(),
            "--wallet-interface=[IP]",
            "Specify the hostname portion of the Wallet API server, IP should be an interface's IP address, or all (all interfaces) or local.",

            ARG arg_wallet_port: (u16) = 8547u16, or |c: &Config| c.wallet.as_ref()?.port.clone(),
            "--wallet-port=[PORT]",
            "Specify the port portion of the Wallet API server.",

            ARG arg_zmq_key_path: (Option<String>) = None, or |c: &Config| c.wallet.as_ref()?.zmq_key_path.clone(),
            "--zmq-key-path=[PATH]",
            "Specify zmq key path for wallet server secure connect ",

        ["Stratum Options"]
            FLAG flag_no_stratum: (bool) = false, or |c: &Config| c.stratum.as_ref()?.disable.clone(),
            "--no-stratum",
            "Run Stratum server for miner push notification.",

            ARG arg_stratum_interface: (String) = "local", or |c: &Config| c.stratum.as_ref()?.interface.clone(),
            "--stratum-interface=[IP]",
            "Interface address for Stratum server.",

            ARG arg_stratum_port: (u16) = 8008u16, or |c: &Config| c.stratum.as_ref()?.port.clone(),
            "--stratum-port=[PORT]",
            "Port for Stratum server to listen on.",

            ARG arg_stratum_secret: (Option<String>) = None, or |c: &Config| c.stratum.as_ref()?.secret.clone(),
            "--stratum-secret=[STRING]",
            "Secret for authorizing Stratum server for peers.",

        ["Sealing/Mining Options"]
            FLAG flag_force_sealing: (bool) = false, or |c: &Config| c.mining.as_ref()?.force_sealing.clone(),
            "--force-sealing",
            "Force the node to author new blocks as if it were always sealing/mining.",

            FLAG flag_remove_solved: (bool) = false, or |c: &Config| c.mining.as_ref()?.remove_solved.clone(),
            "--remove-solved",
            "Remove solved blocks from the work package queue instead of cloning them. This gives a slightly faster import speed, but means that extra solutions submitted for the same work package will go unused.",

            FLAG flag_infinite_pending_block: (bool) = false, or |c: &Config| c.mining.as_ref()?.infinite_pending_block.clone(),
            "--infinite-pending-block",
            "Pending block will be created with maximal possible gas limit and will execute all transactions in the queue. Note that such block is invalid and should never be attempted to be mined.",

            FLAG flag_dynamic_gas_price: (bool) = false , or |c: &Config| c.mining.as_ref()?.dynamic_gas_price.clone(),
            "--dynamic-gas-price",
            "use dynamic gas price which adjust with --gas-price-percentile, --max-blk-traverse, --blk-price-window",

            ARG arg_reseal_on_txs: (String) = "own", or |c: &Config| c.mining.as_ref()?.reseal_on_txs.clone(),
            "--reseal-on-txs=[SET]",
            "Specify which transactions should force the node to reseal a block. SET is one of: none - never reseal on new transactions; own - reseal only on a new local transaction; ext - reseal only on a new external transaction; all - reseal on all new transactions.",

            ARG arg_reseal_min_period: (u64) = 4000u64, or |c: &Config| c.mining.as_ref()?.reseal_min_period.clone(),
            "--reseal-min-period=[MS]",
            "Specify the minimum time between reseals from incoming transactions. MS is time measured in milliseconds.",

            ARG arg_reseal_max_period: (u64) = 120000u64, or |c: &Config| c.mining.as_ref()?.reseal_max_period.clone(),
            "--reseal-max-period=[MS]",
            "Specify the maximum time since last block to enable force-sealing. MS is time measured in milliseconds.",

            ARG arg_work_queue_size: (usize) = 20usize, or |c: &Config| c.mining.as_ref()?.work_queue_size.clone(),
            "--work-queue-size=[ITEMS]",
            "Specify the number of historical work packages which are kept cached lest a solution is found for them later. High values take more memory but result in fewer unusable solutions.",

            ARG arg_relay_set: (String) = "cheap", or |c: &Config| c.mining.as_ref()?.relay_set.clone(),
            "--relay-set=[SET]",
            "Set of transactions to relay. SET may be: cheap - Relay any transacticon in the queue (this may include invalid transactions); strict - Relay only executed transactions (this guarantees we don't relay invalid transactions, but means we relay nothing if not mining); lenient - Same as strict when mining, and cheap when not.",

            ARG arg_gas_floor_target: (String) = "15000000", or |c: &Config| c.mining.as_ref()?.gas_floor_target.clone(),
            "--gas-floor-target=[GAS]",
            "Amount of gas per block to target when sealing a new block.",

            ARG arg_gas_cap: (String) = "20000000", or |c: &Config| c.mining.as_ref()?.gas_cap.clone(),
            "--gas-cap=[GAS]",
            "A cap on how large we will raise the gas limit per block due to transaction volume.",

            ARG arg_tx_queue_mem_limit: (u32) = 2u32, or |c: &Config| c.mining.as_ref()?.tx_queue_mem_limit.clone(),
            "--tx-queue-mem-limit=[MB]",
            "Maximum amount of memory that can be used by the transaction queue. Setting this parameter to 0 disables limiting.",

            ARG arg_tx_queue_strategy: (String) = "gas_price", or |c: &Config| c.mining.as_ref()?.tx_queue_strategy.clone(),
            "--tx-queue-strategy=[S]",
            "Prioritization strategy used to order transactions in the queue. S may be: gas - Prioritize txs with low gas limit; gas_price - Prioritize txs with high gas price; gas_factor - Prioritize txs using gas price and gas limit ratio.",

            ARG arg_tx_queue_ban_count: (u16) = 1u16, or |c: &Config| c.mining.as_ref()?.tx_queue_ban_count.clone(),
            "--tx-queue-ban-count=[C]",
            "Number of times maximal time for execution (--tx-time-limit) can be exceeded before banning sender/recipient/code.",

            ARG arg_tx_queue_ban_time: (u64) = 180u64, or |c: &Config| c.mining.as_ref()?.tx_queue_ban_time.clone(),
            "--tx-queue-ban-time=[SEC]",
            "Banning time (in seconds) for offenders of specified execution time limit. Also number of offending actions have to reach the threshold within that time.",

            ARG arg_min_gas_price: (u64) = 10_000_000_000u64, or |c: &Config| c.mining.as_ref()?.min_gas_price.clone(),
            "--min-gas-price=[NUM]",
            "Minimum amount of Wei per GAS to be paid for a transaction to be accepted for mining.",

            ARG arg_max_gas_price: (u64) = 9_000_000_000_000_000_000u64, or |c: &Config| c.mining.as_ref()?.max_gas_price.clone(),
            "--max-gas-price=[NUM]",
            "Maximum amount of Wei per GAS to be paid for a transaction to be accepted for mining.",

            ARG arg_local_max_gas_price: (u64) = 100_000_000_000u64, or |c: &Config| c.mining.as_ref()?.local_max_gas_price.clone(),
            "--local-max-gas-price=[NUM]",
            "Maximum amount of Wei per GAS to be set for a new local transaction to be accepted for mining when using dynamic gas price.",

            ARG arg_blk_price_window: (usize) = 20usize, or |c: &Config| c.mining.as_ref()?.blk_price_window.clone(),
            "--blk-price-window=[BLOCKS]",
            "Take BLOCKS blk_price in blocks which have transactions for dynamic gas price adjustment. It'll not work without --dynamic-gas-price.",

            ARG arg_max_blk_traverse: (usize) = 64usize, or |c: &Config| c.mining.as_ref()?.max_blk_traverse.clone(),
            "--max-blk-traverse=[BLOCKS]",
            "Maximum amount of blocks can be traversed. It'll not work without --dynamic-gas-price.",

            ARG arg_gas_price_percentile: (usize) = 60usize, or |c: &Config| c.mining.as_ref()?.gas_price_percentile.clone(),
            "--gas-price-percentile=[PCT]",
            "Set PCT percentile block price value from last blk_price_window blocks as default gas price when sending transactions. It'll not work without --dynamic-gas-price.",

            ARG arg_author: (Option<String>) = None, or |c: &Config| c.mining.as_ref()?.author.clone(),
            "--author=[ADDRESS]",
            "Specify the block author (aka \"coinbase\") address for sending block rewards from sealed blocks. NOTE: MINING WILL NOT WORK WITHOUT THIS OPTION.", // Sealing/Mining Option

            ARG arg_tx_gas_limit: (Option<String>) = None, or |c: &Config| c.mining.as_ref()?.tx_gas_limit.clone(),
            "--tx-gas-limit=[GAS]",
            "Apply a limit of GAS as the maximum amount of gas a single transaction may have for it to be mined.",

            ARG arg_tx_time_limit: (Option<u64>) = None, or |c: &Config| c.mining.as_ref()?.tx_time_limit.clone(),
            "--tx-time-limit=[MS]",
            "Maximal time for processing single transaction. If enabled senders/recipients/code of transactions offending the limit will be banned from being included in transaction queue for 180 seconds.",

            ARG arg_extra_data: (Option<String>) = None, or |c: &Config| c.mining.as_ref()?.extra_data.clone(),
            "--extra-data=[STRING]",
            "Specify a custom extra-data for authored blocks, no more than 32 characters.",

        ["Database Options"]
            FLAG flag_no_persistent_txqueue: (bool) = false, or |c: &Config| c.db.as_ref()?.no_persistent_txqueue,
            "--no-persistent-txqueue",
            "Don't save pending local transactions to disk to be restored whenever the node restarts.",

            FLAG flag_disable_wal: (bool) = false, or |c: &Config| c.db.as_ref()?.disable_wal.clone(),
            "--disable-wal",
            "Disables DB WAL, which gives a significant speed up but means an unclean exit is unrecoverable.",

            FLAG flag_scale_verifiers: (bool) = false, or |c: &Config| c.db.as_ref()?.scale_verifiers.clone(),
            "--scale-verifiers",
            "Automatically scale amount of verifier threads based on workload. Not guaranteed to be faster.",

            ARG arg_pruning: (String) = "archive", or |c: &Config| c.db.as_ref()?.pruning.clone(),
            "--pruning=[METHOD]",
            "Configure pruning of the state/storage trie. METHOD may be one of auto, archive, fast: archive - keep all state trie data. No pruning. fast - maintain journal overlay. Fast but 50MB used. auto - use the method most recently synced or default to fast if none synced.",

            ARG arg_pruning_history: (u64) = 64u64, or |c: &Config| c.db.as_ref()?.pruning_history.clone(),
            "--pruning-history=[NUM]",
            "Set a minimum number of recent states to keep when pruning is active.",

            ARG arg_pruning_memory: (usize) = 32usize, or |c: &Config| c.db.as_ref()?.pruning_memory.clone(),
            "--pruning-memory=[MB]",
            "The ideal amount of memory in megabytes to use to store recent states. As many states as possible will be kept within this limit, and at least --pruning-history states will always be kept.",
//
//            ARG arg_cache_size_db: (u32) = 128u32, or |c: &Config| c.db.as_ref()?.cache_size_db.clone(),
//            "--cache-size-db=[MB]",
//            "Override database cache size.",

            ARG arg_cache_size_blocks: (u32) = 8u32, or |c: &Config| c.db.as_ref()?.cache_size_blocks.clone(),
            "--cache-size-blocks=[MB]",
            "Specify the prefered size of the blockchain cache in megabytes.",

            ARG arg_cache_size_queue: (u32) = 40u32, or |c: &Config| c.db.as_ref()?.cache_size_queue.clone(),
            "--cache-size-queue=[MB]",
            "Specify the maximum size of memory to use for block queue.",

            ARG arg_cache_size_state: (u32) = 25u32, or |c: &Config| c.db.as_ref()?.cache_size_state.clone(),
            "--cache-size-state=[MB]",
            "Specify the maximum size of memory to use for the state cache.",

            ARG arg_db_compaction: (String) = "auto", or |c: &Config| c.db.as_ref()?.db_compaction.clone(),
            "--db-compaction=[TYPE]",
            "Database compaction type. TYPE may be one of: ssd - suitable for SSDs and fast HDDs; hdd - suitable for slow HDDs; auto - determine automatically.",

            ARG arg_fat_db: (String) = "auto", or |c: &Config| c.db.as_ref()?.fat_db.clone(),
            "--fat-db=[BOOL]",
            "Build appropriate information to allow enumeration of all accounts and storage keys. Doubles the size of the state database. BOOL may be one of on, off or auto.",

            ARG arg_cache_size: (Option<u32>) = None, or |c: &Config| c.db.as_ref()?.cache_size.clone(),
            "--cache-size=[MB]",
            "Set total amount of discretionary memory to use for the entire system, overrides other cache and queue options.",

            ARG arg_num_verifiers: (Option<usize>) = None, or |c: &Config| c.db.as_ref()?.num_verifiers.clone(),
            "--num-verifiers=[INT]",
            "Amount of verifier threads to use or to begin with, if verifier auto-scaling is enabled.",

        ["Log Options"]
            FLAG flag_no_color: (bool) = false, or |c: &Config| c.log.as_ref()?.no_color.clone(),
            "--no-color",
            "Don't use terminal color codes in output.",

            ARG arg_log_level: (String) = "info", or |c: &Config| c.log.as_ref()?.level.clone(),
            "--log-level=[LEVEL]",
            "Specify all modules' log level. LEVEL may be one of: off, error, warn, info, debug, trace.",

            ARG arg_log_targets: (Vec<String>) = Vec::new(), or |c: &Config| c.log.as_ref()?.targets.clone(),
            "--log-targets=[LOGGINGs]...",
            "Specify the log target you want and specify it's log level. Must conform to the same format as RUST_LOG.eq 'own_tx=debug'.",

            ARG arg_log_file: (Option<String>) = None, or |c: &Config| c.log.as_ref()?.log_file.clone(),
            "--log-file=[FILENAME]",
            "Specify a filename into which logging should be appended.",


    }
}

#[derive(Default, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    aion: Option<Operating>,
    account: Option<Account>,
    network: Option<Network>,
    rpc: Option<Rpc>,
    http: Option<Http>,
    websockets: Option<Ws>,
    ipc: Option<Ipc>,
    wallet: Option<WalletApi>,
    mining: Option<Mining>,
    db: Option<Database>,
    stratum: Option<Stratum>,
    log: Option<Log>,
}

#[derive(Default, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Operating {
    chain: Option<String>,
    base_path: Option<String>,
    db_path: Option<String>,
    keys_path: Option<String>,
}

#[derive(Default, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Account {
    unlock: Option<Vec<String>>,
    password: Option<Vec<String>>,
    keys_iterations: Option<u32>,
    refresh_time: Option<u64>,
    fast_signing: Option<bool>,
}

#[derive(Default, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Network {
    sync_from_boot_nodes_only: Option<bool>,
    max_peers: Option<u32>,
    net_id: Option<u32>,
    local_node: Option<String>,
    boot_nodes: Option<Vec<String>>,
    ip_black_list: Option<Vec<String>>,
}

#[derive(Default, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rpc {
    processing_threads: Option<usize>,
}

#[derive(Default, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Http {
    disable: Option<bool>,
    port: Option<u16>,
    interface: Option<String>,
    cors: Option<Vec<String>>,
    apis: Option<Vec<String>>,
    hosts: Option<Vec<String>>,
    server_threads: Option<usize>,
}

#[derive(Default, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ws {
    disable: Option<bool>,
    port: Option<u16>,
    interface: Option<String>,
    apis: Option<Vec<String>>,
    origins: Option<Vec<String>>,
    hosts: Option<Vec<String>>,
    max_connections: Option<usize>,
}

#[derive(Default, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ipc {
    disable: Option<bool>,
    path: Option<String>,
    apis: Option<Vec<String>>,
}

#[derive(Default, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct WalletApi {
    disable: Option<bool>,
    interface: Option<String>,
    port: Option<u16>,
    secure_connect: Option<bool>,
    zmq_key_path: Option<String>,
}

#[derive(Default, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Mining {
    author: Option<String>,
    force_sealing: Option<bool>,
    reseal_on_txs: Option<String>,
    reseal_min_period: Option<u64>,
    reseal_max_period: Option<u64>,
    work_queue_size: Option<usize>,
    tx_gas_limit: Option<String>,
    tx_time_limit: Option<u64>,
    relay_set: Option<String>,
    min_gas_price: Option<u64>,
    max_gas_price: Option<u64>,
    gas_floor_target: Option<String>,
    gas_cap: Option<String>,
    extra_data: Option<String>,
    tx_queue_mem_limit: Option<u32>,
    tx_queue_strategy: Option<String>,
    tx_queue_ban_count: Option<u16>,
    tx_queue_ban_time: Option<u64>,
    remove_solved: Option<bool>,
    infinite_pending_block: Option<bool>,
    dynamic_gas_price: Option<bool>,
    blk_price_window: Option<usize>,
    gas_price_percentile: Option<usize>,
    max_blk_traverse: Option<usize>,
    local_max_gas_price: Option<u64>,
}

#[derive(Default, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stratum {
    disable: Option<bool>,
    interface: Option<String>,
    port: Option<u16>,
    secret: Option<String>,
}

#[derive(Default, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Database {
    no_persistent_txqueue: Option<bool>,
    pruning: Option<String>,
    pruning_history: Option<u64>,
    pruning_memory: Option<usize>,
    disable_wal: Option<bool>,
    cache_size: Option<u32>,
    //    cache_size_db: Option<u32>,
    cache_size_blocks: Option<u32>,
    cache_size_queue: Option<u32>,
    cache_size_state: Option<u32>,
    db_compaction: Option<String>,
    fat_db: Option<String>,
    scale_verifiers: Option<bool>,
    num_verifiers: Option<usize>,
}

#[derive(Default, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Log {
    no_color: Option<bool>,
    level: Option<String>,
    targets: Option<Vec<String>>,
    log_file: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{
		Args, ArgsError,
		Config, Operating, Account, Network, Ws, Ipc, WalletApi, Mining, Database, Http
};
    use toml;
    use clap::{ErrorKind as ClapErrorKind};

    #[test]
    fn should_accept_any_argument_order() {
        let args = Args::parse(&["aion", "--no-config", "--chain=dev", "account", "list"]).unwrap();
        assert_eq!(args.arg_chain, "dev");

        let args = Args::parse(&["aion", "--no-config", "account", "list", "--chain=dev"]).unwrap();
        assert_eq!(args.arg_chain, "dev");
    }

    #[test]
    fn should_parse_args_and_flags() {
        let args = Args::parse(&["aion", "--no-config", "--pruning", "archive"]).unwrap();
        assert_eq!(args.arg_pruning, "archive");
    }

    #[test]
    fn should_exit_gracefully_on_unknown_argument() {
        let result = Args::parse(&["aion", "--please-exit-gracefully"]);
        assert!(match result {
            Err(ArgsError::Clap(ref clap_error))
                if clap_error.kind == ClapErrorKind::UnknownArgument =>
            {
                true
            }
            _ => false,
        });
    }

    #[test]
    fn should_parse_multiple_values() {
        let args =
            Args::parse(&["aion", "account", "--no-config", "import", "~/1", "~/2"]).unwrap();
        assert_eq!(
            args.arg_account_import_path,
            Some(vec!["~/1".to_owned(), "~/2".to_owned()])
        );

        let args = Args::parse(&["aion", "account", "--no-config", "import", "~/1,ext"]).unwrap();
        assert_eq!(
            args.arg_account_import_path,
            Some(vec!["~/1,ext".to_owned()])
        );

        let args = Args::parse(&[
            "aion",
            "--password",
            "~/.safe/1",
            "--no-config",
            "--password",
            "~/.safe/2",
        ])
        .unwrap();
        assert_eq!(
            args.arg_password,
            vec!["~/.safe/1".to_owned(), "~/.safe/2".to_owned()]
        );

        let args =
            Args::parse(&["aion", "--no-config", "--password", "~/.safe/1,~/.safe/2"]).unwrap();
        assert_eq!(
            args.arg_password,
            vec!["~/.safe/1".to_owned(), "~/.safe/2".to_owned()]
        );
    }

    #[test]
    fn should_parse_global_args_with_subcommand() {
        let args =
            Args::parse(&["aion", "--no-config", "--chain", "dev", "account", "list"]).unwrap();
        assert_eq!(args.arg_chain, "dev".to_owned());
    }

    #[test]
    fn should_parse_args_and_include_config() {
        // given
        let mut config = Config::default();
        let mut operating = Operating::default();
        operating.chain = Some("morden".into());
        config.aion = Some(operating);

        // when
        let args = Args::parse_with_config(&["aion"], config).unwrap();

        // then
        assert_eq!(args.arg_chain, "morden".to_owned());
    }

    #[test]
    fn should_not_use_config_if_cli_is_provided() {
        // given
        let mut config = Config::default();
        let mut operating = Operating::default();
        operating.chain = Some("morden".into());
        config.aion = Some(operating);

        // when
        let args = Args::parse_with_config(&["aion", "--chain", "xyz"], config).unwrap();

        // then
        assert_eq!(args.arg_chain, "xyz".to_owned());
    }

    #[test]
    fn should_use_config_if_cli_is_missing() {
        let mut config = Config::default();
        let mut db = Database::default();
        db.pruning_history = Some(128);
        config.db = Some(db);

        // when
        let args = Args::parse_with_config(&["aion"], config).unwrap();

        // then
        assert_eq!(args.arg_pruning_history, 128);
    }

    #[test]
    fn should_parse_full_config() {
        // given
        let config = toml::from_str(include_str!("./tests/config.full.toml")).unwrap();

        // when
        let args = Args::parse_with_config(&["aion", "--chain", "xyz"], config).unwrap();

        // then
        assert_eq!(
            args,
            Args {
                // Commands
                cmd_daemon: false,
                cmd_account: false,
                cmd_account_new: false,
                cmd_account_list: false,
                cmd_account_import: false,
                cmd_account_import_by_key: false,
                cmd_account_export_to_key: false,
                cmd_import: false,
                cmd_export: false,
                cmd_db: false,
                cmd_db_kill: false,
                cmd_revert: false,

                // Arguments
                arg_daemon_pid_file: None,
                arg_import_file: None,
                arg_import_format: None,
                arg_export_blocks_file: None,
                arg_export_blocks_format: None,
                arg_export_blocks_from: "1".into(),
                arg_export_blocks_to: "latest".into(),
                arg_account_import_path: None,
                arg_account_private_key: None,
                arg_account_address: None,
                arg_revert_blocks_to: "0".into(),

                // -- Operating Options
                arg_chain: "xyz".into(),
                arg_base_path: Some("base".into()),
                arg_db_path: Some("db".into()),
                arg_keys_path: Some("keys".into()),

                // -- Account Options
                arg_unlock: vec!["0xdeadbeefcafe0000000000000000000000000000".into()],
                arg_password: vec!["~/.safe/password.file".into()],
                arg_keys_iterations: 10240u32,
                arg_refresh_time: 2,
                flag_fast_signing: true,

                // -- Networking Options
                arg_max_peers: 50u32,
                arg_boot_nodes: vec![
                    "p2p://22345678-9abc-def0-1234-56789abcdef0@3.4.4.4:4444".into(),
                    "p2p://32345678-9abc-def0-1234-56789abcdef0@4.5.5.5:5555".into()
                ],
                arg_local_node: "p2p://12345678-9abc-def0-1234-56789abcdef0@2.3.3.3:3333".into(),
                arg_net_id: 128u32,
                flag_sync_from_boot_nodes_only: true,
                arg_ip_black_list: vec!["ip1".into(), "ip2".into()],

                // -- API and Console Options
                // RPC
                arg_rpc_processing_threads: Some(3usize),

                // Http
                flag_no_http: true,
                arg_http_port: 8545u16,
                arg_http_interface: "local".into(),
                arg_http_cors: vec!["cor1".into(), "cor2".into()],
                arg_http_apis: vec!["api1".into(), "api2".into()],
                arg_http_hosts: vec!["host1".into(), "host2".into()],
                arg_http_server_threads: Some(5usize),

                // WS
                flag_no_ws: true,
                arg_ws_port: 8546u16,
                arg_ws_interface: "local".into(),
                arg_ws_apis: vec!["api1".into(), "api2".into()],
                arg_ws_origins: vec!["origin1".into(), "origin2".into()],
                arg_ws_hosts: vec!["host1".into(), "host2".into()],
                arg_ws_max_connections: 12usize,

                // IPC
                flag_no_ipc: true,
                arg_ipc_path: "$HOME/.aion/jsonrpc.ipc".into(),
                arg_ipc_apis: vec!["api1".into(), "api2".into()],

                // Wallet
                arg_wallet_interface: "local".into(),
                arg_wallet_port: 8547u16,
                flag_enable_wallet: false,
                flag_secure_connect: true,
                arg_zmq_key_path: Some("zmq".into()),

                // -- Sealing/Mining Options
                arg_author: Some("0xdeadbeefcafe0000000000000000000000000001".into()),
                flag_force_sealing: true,
                arg_reseal_on_txs: "all".into(),
                arg_reseal_min_period: 4000u64,
                arg_reseal_max_period: 60000u64,
                arg_work_queue_size: 20usize,
                arg_tx_gas_limit: Some("6283184".into()),
                arg_tx_time_limit: Some(100u64),
                arg_relay_set: "cheap".into(),
                arg_min_gas_price: 10000000000u64,
                arg_max_gas_price: 9000000000000000000u64,
                arg_gas_price_percentile: 60usize,
                arg_gas_floor_target: "4700000".into(),
                arg_gas_cap: "6283184".into(),
                arg_extra_data: Some("Aion".into()),
                arg_tx_queue_mem_limit: 2u32,
                arg_tx_queue_strategy: "gas_factor".into(),
                arg_tx_queue_ban_count: 1u16,
                arg_tx_queue_ban_time: 180u64,
                flag_remove_solved: true,
                flag_infinite_pending_block: true,
                arg_max_blk_traverse: 64usize,
                arg_blk_price_window: 20usize,
                flag_dynamic_gas_price: true,
                arg_local_max_gas_price: 100000000000u64,

                // -- Stratum Options
                flag_no_stratum: true,
                arg_stratum_interface: "127.0.0.2".to_owned(),
                arg_stratum_port: 8089u16,
                arg_stratum_secret: Some("secret".into()),

                // -- Database Options
                flag_no_persistent_txqueue: true,
                arg_pruning: "auto".into(),
                arg_pruning_history: 64u64,
                arg_pruning_memory: 500usize,
                //                arg_cache_size_db: 64u32,
                arg_cache_size_blocks: 8u32,
                arg_cache_size_queue: 50u32,
                arg_cache_size_state: 25u32,
                arg_cache_size: Some(128),
                flag_disable_wal: true,
                arg_db_compaction: "ssd".into(),
                arg_fat_db: "auto".into(),
                flag_scale_verifiers: true,
                arg_num_verifiers: Some(6),

                // -- Miscellaneous Options
                flag_no_seal_check: false,
                flag_no_config: false,
                flag_version: false,
                flag_default_config: false,
                flag_full_help: false,
                arg_config: "$HOME/.aion/config.toml".into(),

                // -- Log Options
                flag_no_color: true,
                arg_log_file: Some("log file".into()),
                arg_log_level: "level".into(),
                arg_log_targets: vec!["target1".into(), "target2".into()],
            }
        );
    }

    #[test]
    fn should_parse_config_and_return_errors() {
        let config1 = Args::parse_config(include_str!("./tests/config.invalid1.toml"));
        let config2 = Args::parse_config(include_str!("./tests/config.invalid2.toml"));
        let config3 = Args::parse_config(include_str!("./tests/config.invalid3.toml"));
        let config4 = Args::parse_config(include_str!("./tests/config.invalid4.toml"));

        match (config1, config2, config3, config4) {
            (
                Err(ArgsError::Decode(_)),
                Err(ArgsError::Decode(_)),
                Err(ArgsError::Decode(_)),
                Err(ArgsError::Decode(_)),
            ) => {}
            (a, b, c, d) => {
                assert!(
                    false,
                    "Got invalid error types: {:?}, {:?}, {:?}, {:?}",
                    a, b, c, d
                );
            }
        }
    }

    #[test]
    fn should_deserialize_toml_file() {
        let config: Config = toml::from_str(include_str!("./tests/config.toml")).unwrap();

        assert_eq!(
            config,
            Config {
                aion: Some(Operating {
                    chain: Some("./chain.json".into()),
                    base_path: None,
                    db_path: None,
                    keys_path: None,
                }),
                account: Some(Account {
                    unlock: Some(vec!["0x1".into(), "0x2".into(), "0x3".into()]),
                    password: Some(vec!["passwdfile path".into()]),
                    keys_iterations: None,
                    refresh_time: None,
                    fast_signing: None,
                }),
                network: Some(Network {
                    max_peers: Some(20),
                    net_id: None,
                    local_node: None,
                    boot_nodes: None,
                    sync_from_boot_nodes_only: None,
                    ip_black_list: None,
                }),
                websockets: Some(Ws {
                    disable: Some(true),
                    port: None,
                    interface: None,
                    apis: None,
                    origins: Some(vec!["none".into()]),
                    hosts: None,
                    max_connections: None,
                }),
                rpc: None,
                http: Some(Http {
                    disable: Some(true),
                    port: Some(8180),
                    interface: None,
                    cors: None,
                    apis: None,
                    hosts: None,
                    server_threads: None,
                }),
                ipc: Some(Ipc {
                    disable: None,
                    path: None,
                    apis: Some(vec!["rpc".into(), "eth".into()]),
                }),
                wallet: Some(WalletApi {
                    disable: None,
                    interface: None,
                    port: Some(8181),
                    secure_connect: None,
                    zmq_key_path: None,
                }),
                mining: Some(Mining {
                    author: Some("0xdeadbeefcafe0000000000000000000000000001".into()),
                    force_sealing: Some(true),
                    reseal_on_txs: Some("all".into()),
                    reseal_min_period: Some(4000),
                    reseal_max_period: Some(60000),
                    work_queue_size: None,
                    relay_set: None,
                    min_gas_price: None,
                    max_gas_price: None,
                    gas_price_percentile: None,
                    gas_floor_target: None,
                    gas_cap: None,
                    tx_queue_mem_limit: None,
                    tx_queue_strategy: None,
                    tx_queue_ban_count: None,
                    tx_queue_ban_time: None,
                    tx_gas_limit: None,
                    tx_time_limit: None,
                    extra_data: None,
                    remove_solved: None,
                    infinite_pending_block: None,
                    blk_price_window: None,
                    dynamic_gas_price: None,
                    max_blk_traverse: None,
                    local_max_gas_price: None,
                }),
                db: Some(Database {
                    no_persistent_txqueue: None,
                    pruning: Some("fast".into()),
                    pruning_history: Some(64),
                    pruning_memory: None,
                    disable_wal: None,
                    cache_size: None,
                    //                    cache_size_db: Some(256),
                    cache_size_blocks: Some(16),
                    cache_size_queue: Some(100),
                    cache_size_state: Some(25),
                    db_compaction: Some("ssd".into()),
                    fat_db: Some("off".into()),
                    scale_verifiers: Some(false),
                    num_verifiers: None,
                }),
                stratum: None,
                log: None,
            }
        );
    }
}
