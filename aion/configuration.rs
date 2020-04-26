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

use std::time::Duration;
use cli::{Args, ArgsError};
use aion_types::{U256, Address};
use bytes::Bytes;
use p2p::Config;
use acore::client::{VMType};
use acore::miner::{MinerOptions, Banning};
use acore::verification::queue::VerifierSettings;

use rpc::{IpcConfiguration, HttpConfiguration, WsConfiguration};
use aion_rpc::dispatch::DynamicGasPrice;
use cache::CacheConfig;
use helpers::{
    to_block_id, to_u256, to_pending_set, aion_ipc_path, to_addresses,
    to_address, to_queue_strategy,string_to_address
};
use dir::helpers::{replace_home, replace_home_and_local, absolute};
use params::{AccountsConfig, StakeConfig, MinerExtras, SpecType};
use logger::{LogConfig};
use dir::{self, Directories, default_local_path, default_data_path};
use run::RunCmd;
use blockchain::{
    BlockchainCmd, ImportBlockchain, ExportBlockchain, KillBlockchain, RevertBlockchain, DataFormat,
};
use account::{AccountCmd, NewAccount, ListAccounts, ImportAccounts, ImportAccount, ExportAccount};

/// Command enum
#[derive(Debug, PartialEq)]
pub enum Cmd {
    Run(RunCmd),
    Version,
    Account(AccountCmd),
    Blockchain(BlockchainCmd),
}

/// params for executing
pub struct Execute {
    pub logger: LogConfig,
    pub cmd: Cmd,
}

/// overall configration
#[derive(Debug, PartialEq)]
pub struct Configuration {
    /// all config params
    pub args: Args,
}

impl Configuration {
    /// convert config params to Args
    pub fn parse<S: AsRef<str>>(command: &[S]) -> Result<Self, ArgsError> {
        let args = Args::parse(command)?;

        let config = Configuration {
            args,
        };

        Ok(config)
    }

    /// convert Args to Cmd
    pub fn into_command(self) -> Result<Execute, String> {
        let dirs = self.directories();
        let pruning = self.args.arg_pruning.parse()?;
        let pruning_history = self.args.arg_pruning_history;
        let pruning_memory = self.args.arg_pruning_memory;
        let vm_type = VMType::FastVM;
        let spec = self.chain()?;
        let logger_config = self.logger_config();
        let ws_conf = self.ws_config()?;
        let http_conf = self.http_config()?;
        let ipc_conf = self.ipc_config()?;
        let net_conf = self.net_config()?;
        let cache_config = self.cache_config();
        let fat_db = self.args.arg_fat_db.parse()?;
        let compaction = self.args.arg_db_compaction.parse()?;
        let wal = !self.args.flag_disable_wal;
        let format = self.format()?;

        let cmd = if self.args.flag_version {
            Cmd::Version
        } else if self.args.cmd_db && self.args.cmd_db_kill {
            Cmd::Blockchain(BlockchainCmd::Kill(KillBlockchain {
                spec,
                dirs,
                pruning,
            }))
        } else if self.args.cmd_account {
            let account_cmd = if self.args.cmd_account_new {
                let new_acc = NewAccount {
                    iterations: self.args.arg_keys_iterations,
                    path: dirs.keys,
                    spec,
                    password_file: self
                        .accounts_config()?
                        .password_files
                        .first()
                        .map(|x| x.to_owned()),
                };
                AccountCmd::New(new_acc)
            } else if self.args.cmd_account_list {
                let list_acc = ListAccounts {
                    path: dirs.keys,
                    spec,
                };
                AccountCmd::List(list_acc)
            } else if self.args.cmd_account_import {
                let import_acc = ImportAccounts {
                    from: self
                        .args
                        .arg_account_import_path
                        .expect("CLI argument is required; qed")
                        .clone(),
                    to: dirs.keys,
                    spec,
                };
                AccountCmd::Import(import_acc)
            } else if self.args.cmd_account_import_by_key {
                let import_acc = ImportAccount {
                    iterations: self.args.arg_keys_iterations,
                    path: dirs.keys,
                    spec,
                    pri_keys: self.args.arg_account_private_key,
                };
                AccountCmd::ImportByPrivkey(import_acc)
            } else if self.args.cmd_account_export_to_key {
                let export_acc = ExportAccount {
                    iterations: self.args.arg_keys_iterations,
                    path: dirs.keys,
                    spec,
                    address: self.args.arg_account_address,
                };
                AccountCmd::ExportToProvkey(export_acc)
            } else {
                unreachable!();
            };
            Cmd::Account(account_cmd)
        } else if self.args.cmd_import {
            let import_cmd = ImportBlockchain {
                spec,
                cache_config,
                dirs,
                file_path: self.args.arg_import_file.clone(),
                format,
                pruning,
                pruning_history,
                pruning_memory,
                compaction,
                wal,
                fat_db,
                vm_type,
                // with_color: logger_config.color,
                verifier_settings: self.verifier_settings(),
                stake_conf: self.stake_config()?,
            };
            Cmd::Blockchain(BlockchainCmd::Import(import_cmd))
        } else if self.args.cmd_export {
            let export_cmd = ExportBlockchain {
                spec,
                cache_config,
                dirs,
                file_path: self.args.arg_export_blocks_file.clone(),
                format,
                pruning,
                pruning_history,
                pruning_memory,
                compaction,
                wal,
                fat_db,
                from_block: to_block_id(&self.args.arg_export_blocks_from)?,
                to_block: to_block_id(&self.args.arg_export_blocks_to)?,
            };
            Cmd::Blockchain(BlockchainCmd::Export(export_cmd))
        } else if self.args.cmd_revert {
            let revert_cmd = RevertBlockchain {
                spec,
                cache_config,
                dirs,
                pruning,
                pruning_history,
                pruning_memory,
                compaction,
                wal,
                fat_db,
                to_block: to_block_id(&self.args.arg_revert_blocks_to)?,
            };
            Cmd::Blockchain(BlockchainCmd::Revert(revert_cmd))
        } else {
            let verifier_settings = self.verifier_settings();

            let run_cmd = RunCmd {
                cache_config,
                dirs,
                spec,
                pruning,
                pruning_history,
                pruning_memory,
                miner_options: self.miner_options()?,
                dynamic_gas_price: self.dynamic_gas_price()?,
                ws_conf,
                http_conf,
                ipc_conf,
                net_conf,
                acc_conf: self.accounts_config()?,
                stake_conf: self.stake_config()?,
                miner_extras: self.miner_extras()?,
                fat_db,
                compaction,
                wal,
                vm_type,
                verifier_settings,
            };
            Cmd::Run(run_cmd)
        };

        Ok(Execute {
            logger: logger_config,
            cmd,
        })
    }

    /// parse miner startup parameters
    fn miner_extras(&self) -> Result<MinerExtras, String> {
        let extras = MinerExtras {
            author: self.author()?,
            extra_data: self.extra_data()?,
            gas_floor_target: to_u256(&self.args.arg_gas_floor_target)?,
            gas_ceil_target: to_u256(&self.args.arg_gas_cap)?,
        };

        Ok(extras)
    }

    /// parse miner author
    fn author(&self) -> Result<Address, String> { to_address(self.args.arg_author.clone()) }

    /// parse import/export blockchain data format
    fn format(&self) -> Result<Option<DataFormat>, String> {
        match self
            .args
            .arg_import_format
            .clone()
            .or(self.args.arg_export_blocks_format.clone())
        {
            Some(ref f) => Ok(Some(f.parse()?)),
            None => Ok(None),
        }
    }

    /// parse cache sizes.
    fn cache_config(&self) -> CacheConfig {
        match self.args.arg_cache_size {
            Some(size) => CacheConfig::new_with_total_cache_size(size),
            None => {
                CacheConfig::new(
                    //                    self.args.arg_cache_size_db,
                    self.args.arg_cache_size_blocks,
                    self.args.arg_cache_size_queue,
                    self.args.arg_cache_size_state,
                )
            }
        }
    }

    /// parse logger config params
    fn logger_config(&self) -> LogConfig {
        // let level = validate_log_level(self.args.arg_log_level.clone(), "total");
        // let targets = parse_log_target(self.args.arg_log_targets.clone());
        // LogConfig {
        //     targets,
        //     level,
        //     color: !self.args.flag_no_color && !cfg!(windows),
        //     file: self
        //         .args
        //         .arg_log_file
        //         .as_ref()
        //         .map(|log_file| replace_home(&self.directories().base, log_file)),
        // }
        LogConfig {
            config: self.args.arg_log_config.clone(),
        }
    }

    /// parse genesis spec
    fn chain(&self) -> Result<SpecType, String> {
        let name = self.args.arg_chain.clone();

        let name = ::dir::helpers::replace_home(
            self.args
                .arg_base_path
                .as_ref()
                .map_or(&self.directories().base, |s| s.as_str().clone()),
            name.as_str(),
        );
        Ok(name.parse()?)
    }

    /// parse max peers
    fn max_peers(&self) -> u32 {
        let peers = self.args.arg_max_peers;
        peers
    }

    /// parse stake config
    fn stake_config(&self) -> Result<StakeConfig, String> {
        let stake_contract = &self.args.arg_stake_contract;
        let cfg = StakeConfig {
            contract: string_to_address(stake_contract)?,
        };

        Ok(cfg)
    }

    /// parse account config
    fn accounts_config(&self) -> Result<AccountsConfig, String> {
        let cfg = AccountsConfig {
            iterations: self.args.arg_keys_iterations,
            refresh_time: self.args.arg_refresh_time,
            password_files: self
                .args
                .arg_password
                .iter()
                .map(|s| replace_home(&self.directories().base, s))
                .collect(),
            unlocked_accounts: to_addresses(&self.args.arg_unlock)?,
            enable_fast_signing: false,
        };

        Ok(cfg)
    }

    /// parse miner behaviours
    fn miner_options(&self) -> Result<MinerOptions, String> {
        let options = MinerOptions {
            force_sealing: self.args.flag_force_sealing,
            tx_gas_limit: match self.args.arg_tx_gas_limit {
                Some(ref d) => to_u256(d)?,
                None => U256::max_value(),
            },
            tx_queue_memory_limit: if self.args.arg_tx_queue_mem_limit > 0 {
                Some(self.args.arg_tx_queue_mem_limit as usize * 1024 * 1024)
            } else {
                None
            },
            tx_queue_strategy: to_queue_strategy(&self.args.arg_tx_queue_strategy)?,
            pending_set: to_pending_set(&self.args.arg_relay_set)?,
            reseal_min_period: Duration::from_millis(self.args.arg_reseal_min_period),
            prepare_block_interval: Duration::from_millis(self.args.arg_reseal_min_period),
            work_queue_size: self.args.arg_work_queue_size,
            enable_resubmission: !self.args.flag_remove_solved,
            tx_queue_banning: match self.args.arg_tx_time_limit {
                Some(limit) => {
                    Banning::Enabled {
                        min_offends: self.args.arg_tx_queue_ban_count,
                        offend_threshold: Duration::from_millis(limit),
                        ban_duration: Duration::from_secs(self.args.arg_tx_queue_ban_time),
                    }
                }
                None => Banning::Disabled,
            },
            infinite_pending_block: self.args.flag_infinite_pending_block,
            minimal_gas_price: U256::from(self.args.arg_min_gas_price),
            maximal_gas_price: U256::from(self.args.arg_max_gas_price),
            local_max_gas_price: U256::from(self.args.arg_local_max_gas_price),
            staker_private_key: self.args.arg_staker_private_key.to_owned(),
        };

        Ok(options)
    }

    /// parse auto-adjusted gas price params
    fn dynamic_gas_price(&self) -> Result<Option<DynamicGasPrice>, String> {
        if !self.args.flag_dynamic_gas_price {
            return Ok(None);
        }
        let mut dynamic = DynamicGasPrice::default();

        dynamic.blk_price_window = self.args.arg_blk_price_window;
        dynamic.max_blk_traverse = self.args.arg_max_blk_traverse;
        dynamic.gas_price_percentile = self.args.arg_gas_price_percentile;

        Ok(Some(dynamic))
    }

    /// parse extra data
    fn extra_data(&self) -> Result<Bytes, String> {
        match self.args.arg_extra_data.as_ref() {
            Some(x) if x.len() <= 32 => Ok(x.as_bytes().to_owned()),
            None => Ok("AION".as_bytes().to_vec()),
            Some(_) => Err("Extra data must be at most 32 characters".into()),
        }
    }

    /// parse network config params
    fn net_config(&self) -> Result<Config, String> {
        let mut ret = Config::new();
        ret.max_peers = self.max_peers();
        ret.local_node = self.args.arg_local_node.clone();
        ret.boot_nodes = self.args.arg_boot_nodes.clone();
        ret.sync_from_boot_nodes_only = self.args.flag_sync_from_boot_nodes_only;
        ret.net_id = self.args.arg_net_id.clone();
        ret.ip_black_list = self.args.arg_ip_black_list.clone();
        Ok(ret)
    }

    /// parse rpc http apis
    fn rpc_apis(&self) -> String { self.args.arg_http_apis.clone().join(",") }

    /// parse rpc http cors
    fn cors(cors: &str) -> Option<Vec<String>> {
        match cors {
            "none" => return Some(Vec::new()),
            "*" | "all" | "any" => return None,
            _ => {}
        }

        Some(cors.split(',').map(Into::into).collect())
    }

    /// parse rpc http cors
    fn rpc_cors(&self) -> Option<Vec<String>> {
        let cors = self.args.arg_http_cors.to_owned().join(",");
        Self::cors(&cors)
    }

    /// parse hosts
    fn hosts(&self, hosts: &str, interface: &str) -> Option<Vec<String>> {
        if interface == "0.0.0.0" && hosts == "none" {
            return None;
        }

        Self::parse_hosts(hosts)
    }

    /// parse hosts
    fn parse_hosts(hosts: &str) -> Option<Vec<String>> {
        match hosts {
            "none" => return Some(Vec::new()),
            "*" | "all" | "any" => return None,
            _ => {}
        }
        let hosts = hosts.split(',').map(Into::into).collect();
        Some(hosts)
    }

    /// parse http hosts
    fn rpc_hosts(&self) -> Option<Vec<String>> {
        self.hosts(
            &self.args.arg_http_hosts.clone().join(","),
            &self.rpc_interface(),
        )
    }

    /// parse web socket hosts
    fn ws_hosts(&self) -> Option<Vec<String>> {
        self.hosts(&self.args.arg_ws_hosts.join(","), &self.ws_interface())
    }

    /// parse websocket origins
    fn ws_origins(&self) -> Option<Vec<String>> {
        Self::parse_hosts(&self.args.arg_ws_origins.join(","))
    }

    /// parse ipc config params
    fn ipc_config(&self) -> Result<IpcConfiguration, String> {
        let conf = IpcConfiguration {
            enabled: !self.args.flag_no_ipc,
            socket_addr: self.ipc_path(),
            apis: self.args.arg_ipc_apis.join(",").parse()?,
        };

        Ok(conf)
    }

    /// parse http config params
    fn http_config(&self) -> Result<HttpConfiguration, String> {
        let conf = HttpConfiguration {
            enabled: self.rpc_enabled(),
            interface: self.rpc_interface(),
            port: self.args.arg_http_port,
            apis: self.rpc_apis().parse()?,
            hosts: self.rpc_hosts(),
            cors: self.rpc_cors(),
            server_threads: match self.args.arg_http_server_threads {
                Some(threads) if threads > 0 => threads,
                _ => 1,
            },
            processing_threads: match self.args.arg_rpc_processing_threads {
                Some(threads) if threads > 0 => threads,
                _ => 4,
            },
        };

        Ok(conf)
    }

    /// parse websocket config params
    fn ws_config(&self) -> Result<WsConfiguration, String> {
        let conf = WsConfiguration {
            enabled: self.ws_enabled(),
            interface: self.ws_interface(),
            port: self.args.arg_ws_port,
            apis: self.args.arg_ws_apis.join(",").parse()?,
            hosts: self.ws_hosts(),
            origins: self.ws_origins(),
            max_connections: self.args.arg_ws_max_connections,
        };

        Ok(conf)
    }

    /// parse directories
    fn directories(&self) -> Directories {
        let local_path = default_local_path();
        let base_path = self
            .args
            .arg_base_path
            .as_ref()
            .map_or_else(|| default_data_path(), |s| s.clone());
        let data_path = replace_home("", &base_path);
        let is_using_base_path = self.args.arg_base_path.is_some();
        // If base_path is set and db_path is not we default to base path subdir instead of LOCAL.
        let base_db_path = if is_using_base_path && self.args.arg_db_path.is_none() {
            "$BASE/chains"
        } else {
            self.args
                .arg_db_path
                .as_ref()
                .map_or(dir::CHAINS_PATH, |s| &s)
        };
        let base_keys_path = if is_using_base_path && self.args.arg_keys_path.is_none() {
            "$BASE/keys"
        } else {
            self.args
                .arg_keys_path
                .as_ref()
                .map_or(dir::KEYS_PATH, |s| &s)
        };
        let cache_path = if is_using_base_path {
            "$BASE/cache"
        } else {
            dir::CACHE_PATH
        };

        let db_path = absolute(replace_home_and_local(
            &data_path,
            &local_path,
            base_db_path,
        ));
        let cache_path = absolute(replace_home_and_local(&data_path, &local_path, cache_path));
        let keys_path = absolute(replace_home_and_local(
            &data_path,
            &local_path,
            base_keys_path,
        ));
        let config_path = if self.args.flag_no_config {
            None
        } else {
            Some(absolute(replace_home_and_local(
                &data_path,
                &local_path,
                &self.args.arg_config,
            )))
        };
        Directories {
            keys: keys_path,
            base: data_path,
            cache: cache_path,
            db: db_path,
            config: config_path,
        }
    }

    /// parse ipc path
    fn ipc_path(&self) -> String {
        aion_ipc_path(&self.directories().base, &self.args.arg_ipc_path.clone())
    }

    /// parse interface
    fn interface(&self, interface: &str) -> String {
        match interface {
            "all" => "0.0.0.0",
            "local" => "127.0.0.1",
            x => x,
        }
        .into()
    }

    /// parse rpc interface
    fn rpc_interface(&self) -> String {
        let rpc_interface = self.args.arg_http_interface.clone();
        self.interface(&rpc_interface)
    }

    /// parse websocket interface
    fn ws_interface(&self) -> String { self.interface(&self.args.arg_ws_interface) }

    /// parse whether http apis are enabled
    fn rpc_enabled(&self) -> bool { !self.args.flag_no_http }

    /// parse whether web socket apis are enabled
    fn ws_enabled(&self) -> bool { !self.args.flag_no_ws }

    /// parse verifier settings
    fn verifier_settings(&self) -> VerifierSettings {
        let mut settings = VerifierSettings::default();
        settings.scale_verifiers = self.args.flag_scale_verifiers;
        if let Some(num_verifiers) = self.args.arg_num_verifiers {
            settings.num_verifiers = num_verifiers;
        }

        settings
    }
}

#[cfg(test)]
mod tests {
    use acore::client::{BlockId};
    use acore::transaction::transaction_queue::PrioritizationStrategy;
    use account::{AccountCmd, NewAccount, ImportAccounts, ListAccounts};
    use blockchain::{BlockchainCmd, ImportBlockchain, ExportBlockchain, DataFormat};
    use cli::Args;
    use dir::Directories;
    use run::RunCmd;
    use p2p::Config;
    use super::*;

    #[derive(Debug, PartialEq)]
    struct TestPasswordReader(&'static str);

    fn parse(args: &[&str]) -> Configuration {
        Configuration {
            args: Args::parse_without_config(args).unwrap(),
        }
    }

    pub fn default_network_config() -> Config {
        Config {
            boot_nodes: vec![
                "p2p://c33d2207-729a-4584-86f1-e19ab97cf9ce@51.144.42.220:30303".into(),
                "p2p://c33d302f-216b-47d4-ac44-5d8181b56e7e@52.231.187.227:30303".into(),
                "p2p://c33d4c07-6a29-4ca6-8b06-b2781ba7f9bf@191.232.164.119:30303".into(),
                "p2p://c39d0a10-20d8-49d9-97d6-284f88da5c25@13.92.157.19:30303".into(),
                "p2p://c38d2a32-20d8-49d9-97d6-284f88da5c83@40.78.84.78:30303".into(),
                "p2p://c37d6b45-20d8-49d9-97d6-284f88da5c51@104.40.182.54:30303".into(),
                "p2p://c36d4208-fe4b-41fa-989b-c7eeafdffe72@35.208.215.219:30303".into(),
            ],
            max_peers: 64,
            local_node: "p2p://00000000-0000-0000-0000-000000000000@0.0.0.0:30303".to_string(),
            net_id: 256,
            sync_from_boot_nodes_only: false,
            ip_black_list: Vec::new(),
        }
    }

    #[test]
    fn test_command_version() {
        let args = vec!["aion", "--version"];
        let conf = parse(&args);
        assert_eq!(conf.into_command().unwrap().cmd, Cmd::Version);
    }

    #[test]
    fn test_command_account_new() {
        let args = vec!["aion", "account", "new"];
        let conf = parse(&args);
        assert_eq!(
            conf.into_command().unwrap().cmd,
            Cmd::Account(AccountCmd::New(NewAccount {
                iterations: 10240,
                path: Directories::default().keys,
                password_file: None,
                spec: Default::default(),
            }))
        );
    }

    #[test]
    fn test_command_account_list() {
        let args = vec!["aion", "account", "list"];
        let conf = parse(&args);
        assert_eq!(
            conf.into_command().unwrap().cmd,
            Cmd::Account(AccountCmd::List(ListAccounts {
                path: Directories::default().keys,
                spec: Default::default(),
            }))
        );
    }

    #[test]
    fn test_command_account_import() {
        let args = vec!["aion", "account", "import", "my_dir", "another_dir"];
        let conf = parse(&args);
        assert_eq!(
            conf.into_command().unwrap().cmd,
            Cmd::Account(AccountCmd::Import(ImportAccounts {
                from: vec!["my_dir".into(), "another_dir".into()],
                to: Directories::default().keys,
                spec: Default::default(),
            }))
        );
    }

    #[test]
    fn test_command_blockchain_import() {
        let args = vec!["aion", "import", "blockchain.json"];
        let conf = parse(&args);
        assert_eq!(
            conf.into_command().unwrap().cmd,
            Cmd::Blockchain(BlockchainCmd::Import(ImportBlockchain {
                spec: Default::default(),
                cache_config: Default::default(),
                dirs: Default::default(),
                file_path: Some("blockchain.json".into()),
                format: Default::default(),
                pruning: Default::default(),
                pruning_history: 64,
                pruning_memory: 32,
                compaction: Default::default(),
                wal: true,
                fat_db: Default::default(),
                vm_type: Default::default(),
                verifier_settings: Default::default(),
                stake_conf: Default::default()
            }))
        );
    }

    #[test]
    fn test_command_blockchain_export() {
        let args = vec!["aion", "export", "blockchain.json"];
        let conf = parse(&args);
        assert_eq!(
            conf.into_command().unwrap().cmd,
            Cmd::Blockchain(BlockchainCmd::Export(ExportBlockchain {
                spec: Default::default(),
                cache_config: Default::default(),
                dirs: Default::default(),
                file_path: Some("blockchain.json".into()),
                pruning: Default::default(),
                pruning_history: 64,
                pruning_memory: 32,
                format: Default::default(),
                compaction: Default::default(),
                wal: true,
                fat_db: Default::default(),
                from_block: BlockId::Number(1),
                to_block: BlockId::Latest,
            }))
        );
    }

    #[test]
    fn test_command_blockchain_export_with_custom_format() {
        let args = vec!["aion", "export", "--format", "hex", "blockchain.json"];
        let conf = parse(&args);
        assert_eq!(
            conf.into_command().unwrap().cmd,
            Cmd::Blockchain(BlockchainCmd::Export(ExportBlockchain {
                spec: Default::default(),
                cache_config: Default::default(),
                dirs: Default::default(),
                file_path: Some("blockchain.json".into()),
                pruning: Default::default(),
                pruning_history: 64,
                pruning_memory: 32,
                format: Some(DataFormat::Hex),
                compaction: Default::default(),
                wal: true,
                fat_db: Default::default(),
                from_block: BlockId::Number(1),
                to_block: BlockId::Latest,
            }))
        );
    }

    #[test]
    fn test_run_cmd() {
        let args = vec!["aion"];
        let conf = parse(&args);
        let expected = RunCmd {
            cache_config: Default::default(),
            dirs: Default::default(),
            spec: Default::default(),
            pruning: Default::default(),
            pruning_history: 64,
            pruning_memory: 32,
            miner_options: Default::default(),
            dynamic_gas_price: Default::default(),
            ws_conf: Default::default(),
            http_conf: Default::default(),
            ipc_conf: Default::default(),
            net_conf: default_network_config(),
            acc_conf: Default::default(),
            stake_conf: Default::default(),
            miner_extras: Default::default(),
            compaction: Default::default(),
            wal: true,
            vm_type: Default::default(),
            fat_db: Default::default(),
            verifier_settings: Default::default(),
        };
        assert_eq!(conf.into_command().unwrap().cmd, Cmd::Run(expected));
    }

    #[test]
    fn should_parse_mining_options() {
        // given
        let mut mining_options = Default::default();

        // when
        let conf0 = parse(&["aion"]);
        let conf1 = parse(&["aion", "--tx-queue-strategy", "gas_factor"]);
        let conf2 = parse(&["aion", "--tx-queue-strategy", "gas_price"]);
        let conf3 = parse(&["aion", "--tx-queue-strategy", "gas"]);

        // then
        assert_eq!(conf0.miner_options().unwrap(), mining_options);
        mining_options.tx_queue_strategy = PrioritizationStrategy::GasFactorAndGasPrice;
        assert_eq!(conf1.miner_options().unwrap(), mining_options);
        mining_options.tx_queue_strategy = PrioritizationStrategy::GasPriceOnly;
        assert_eq!(conf2.miner_options().unwrap(), mining_options);
        mining_options.tx_queue_strategy = PrioritizationStrategy::GasAndGasPrice;
        assert_eq!(conf3.miner_options().unwrap(), mining_options);
    }

    #[test]
    fn should_parse_rpc_hosts() {
        // given

        // when
        let conf0 = parse(&["aion"]);
        let conf1 = parse(&["aion", "--http-hosts", "none"]);
        let conf2 = parse(&["aion", "--http-hosts", "all"]);
        let conf3 = parse(&["aion", "--http-hosts", "aion.io,something.io"]);

        // then
        assert_eq!(conf0.rpc_hosts(), Some(Vec::new()));
        assert_eq!(conf1.rpc_hosts(), Some(Vec::new()));
        assert_eq!(conf2.rpc_hosts(), None);
        assert_eq!(
            conf3.rpc_hosts(),
            Some(vec!["aion.io".into(), "something.io".into()])
        );
    }

    #[test]
    fn should_use_correct_cache_path_if_base_is_set() {
        let std = parse(&["aion"]);
        let base = parse(&["aion", "--base-path", "/test"]);

        let base_path = ::dir::default_data_path();
        let local_path = ::dir::default_local_path();
        assert_eq!(
            std.directories().cache,
            dir::helpers::replace_home_and_local(&base_path, &local_path, ::dir::CACHE_PATH)
        );
        assert_eq!(base.directories().cache, "/test/cache");
    }
}
