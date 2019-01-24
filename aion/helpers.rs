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

use std::io;
use std::io::{Write, BufReader, BufRead};
use std::fs::File;
use aion_types::{U256, clean_0x, Address};
use journaldb::Algorithm;
use acore::client::{BlockId, VMType, DatabaseCompactionProfile, ClientConfig, VerifierType};
use acore::miner::PendingSet;
use acore::transaction::transaction_queue::PrioritizationStrategy;
use cache::CacheConfig;
use dir::helpers::replace_home;

pub fn to_block_id(s: &str) -> Result<BlockId, String> {
    if s == "latest" {
        Ok(BlockId::Latest)
    } else if let Ok(num) = s.parse() {
        Ok(BlockId::Number(num))
    } else if let Ok(hash) = s.parse() {
        Ok(BlockId::Hash(hash))
    } else {
        Err("Invalid block.".into())
    }
}

pub fn to_u256(s: &str) -> Result<U256, String> {
    if let Ok(decimal) = U256::from_dec_str(s) {
        Ok(decimal)
    } else if let Ok(hex) = clean_0x(s).parse() {
        Ok(hex)
    } else {
        Err(format!("Invalid numeric value: {}", s))
    }
}

pub fn to_pending_set(s: &str) -> Result<PendingSet, String> {
    match s {
        "cheap" => Ok(PendingSet::AlwaysQueue),
        "strict" => Ok(PendingSet::AlwaysSealing),
        "lenient" => Ok(PendingSet::SealingOrElseQueue),
        other => Err(format!("Invalid pending set value: {:?}", other)),
    }
}

pub fn to_queue_strategy(s: &str) -> Result<PrioritizationStrategy, String> {
    match s {
        "gas" => Ok(PrioritizationStrategy::GasAndGasPrice),
        "gas_price" => Ok(PrioritizationStrategy::GasPriceOnly),
        "gas_factor" => Ok(PrioritizationStrategy::GasFactorAndGasPrice),
        other => Err(format!("Invalid queue strategy: {}", other)),
    }
}

pub fn to_address(s: Option<String>) -> Result<Address, String> {
    match s {
        Some(ref a) => {
            clean_0x(a)
                .parse()
                .map_err(|_| format!("Invalid address: {:?}", a))
        }
        None => Ok(Address::default()),
    }
}

pub fn to_addresses(s: &Vec<String>) -> Result<Vec<Address>, String> {
    s.into_iter()
        .map(|s1| {
            clean_0x(s1)
                .parse()
                .map_err(|_| format!("Invalid address: {:?}", s1))
        })
        .collect()
}

/// Flush output buffer.
pub fn flush_stdout() { io::stdout().flush().expect("stdout is flushable; qed"); }

/// Formats and returns aion ipc path.
pub fn aion_ipc_path(base: &str, path: &str) -> String {
    let path = path.to_owned();
    replace_home(base, &path)
}

#[cfg(test)]
pub fn default_network_config() -> ::sync::p2p::NetworkConfig {
    use sync::p2p::NetworkConfig;
    NetworkConfig {
        boot_nodes: vec![
            "p2p://c33d2207-729a-4584-86f1-e19ab97cf9ce@51.144.42.220:30303".into(),
            "p2p://c33d302f-216b-47d4-ac44-5d8181b56e7e@52.231.187.227:30303".into(),
            "p2p://c33d4c07-6a29-4ca6-8b06-b2781ba7f9bf@191.232.164.119:30303".into(),
            "p2p://741b979e-6a06-493a-a1f2-693cafd37083@66.207.217.190:30303".into(),
            "p2p://c39d0a10-20d8-49d9-97d6-284f88da5c25@13.92.157.19:30303".into(),
            "p2p://c38d2a32-20d8-49d9-97d6-284f88da5c83@40.78.84.78:30303".into(),
            "p2p://c37d6b45-20d8-49d9-97d6-284f88da5c51@104.40.182.54:30303".into(),
        ],
        max_peers: 64,
        local_node: "p2p://00000000-0000-0000-0000-000000000000@0.0.0.0:30303".to_string(),
        net_id: 256,
        sync_from_boot_nodes_only: false,
        ip_black_list: Vec::new(),
    }
}

pub fn to_client_config(
    cache_config: &CacheConfig,
    spec_name: String,
    fat_db: bool,
    compaction: DatabaseCompactionProfile,
    wal: bool,
    vm_type: VMType,
    pruning: Algorithm,
    pruning_history: u64,
    pruning_memory: usize,
    check_seal: bool,
) -> ClientConfig
{
    let mut client_config = ClientConfig::default();

    let mb = 1024 * 1024;
    // in bytes
    client_config.blockchain.max_cache_size = cache_config.blockchain() as usize * mb;
    // in bytes
    client_config.blockchain.pref_cache_size = cache_config.blockchain() as usize * 3 / 4 * mb;
    // db cache size, in megabytes
    client_config.db_cache_size = Some(cache_config.db_cache_size() as usize);
    // db queue cache size, in bytes
    client_config.queue.max_mem_use = cache_config.queue() as usize * mb;
    // in bytes
    client_config.state_cache_size = cache_config.state() as usize * mb;
    // in bytes
    client_config.jump_table_size = cache_config.jump_tables() as usize * mb;
    // in bytes
    client_config.history_mem = pruning_memory * mb;

    client_config.fat_db = fat_db;
    client_config.pruning = pruning;
    client_config.history = pruning_history;
    client_config.db_compaction = compaction;
    client_config.db_wal = wal;
    client_config.vm_type = vm_type;
    client_config.verifier_type = if check_seal {
        VerifierType::Canon
    } else {
        VerifierType::CanonNoSeal
    };
    client_config.spec_name = spec_name;
    client_config
}

/// Prompts user asking for password.
pub fn password_prompt() -> Result<String, String> {
    use rpassword::read_password;
    const STDIN_ERROR: &'static str = "Unable to ask for password on non-interactive terminal.";

    print!("please type password: ");
    flush_stdout();

    let password = read_password().map_err(|_| STDIN_ERROR.to_owned())?;

    print!("please repeat password: ");
    flush_stdout();

    let password_repeat = read_password().map_err(|_| STDIN_ERROR.to_owned())?;

    if password != password_repeat {
        return Err("Passwords do not match!".into());
    }

    Ok(password)
}

/// ask user for password once.
pub fn password_once() -> Result<String, String> {
    use rpassword::read_password;
    const STDIN_ERROR: &'static str = "Unable to ask for password on non-interactive terminal.";
    print!("please type password: ");
    flush_stdout();
    let password = read_password().map_err(|_| STDIN_ERROR.to_owned())?;
    Ok(password)
}

/// Read a password from password file.
pub fn password_from_file(path: String) -> Result<String, String> {
    let passwords = passwords_from_files(&[path])?;
    // use only first password from the file
    passwords
        .get(0)
        .map(String::to_owned)
        .ok_or_else(|| "Password file seems to be empty.".to_owned())
}

/// Reads passwords from files. Treats each line as a separate password.
pub fn passwords_from_files(files: &[String]) -> Result<Vec<String>, String> {
    let passwords = files
        .iter()
        .map(|filename| {
            let file = File::open(filename).map_err(|_| {
                format!(
                    "{} Unable to read password file. Ensure it exists and permissions are \
                     correct.",
                    filename
                )
            })?;
            let reader = BufReader::new(&file);
            let lines = reader
                .lines()
                .filter_map(|l| l.ok())
                .map(|pwd| pwd.trim().to_owned())
                .collect::<Vec<String>>();
            Ok(lines)
        })
        .collect::<Result<Vec<Vec<String>>, String>>();
    Ok(passwords?.into_iter().flat_map(|x| x).collect())
}

pub fn validate_log_level(level: String, target: &str) -> String {
    match level.clone().to_lowercase().as_str() {
        "off" | "error" | "warn" | "info" | "debug" | "trace" => level,
        _ => {
            warn!(
                target: "run",
                "{} log level is invalid:{} .ignore it, using default level:Info",
                target, level
            );
            "info".into()
        }
    }
}

pub fn parse_log_target(targets: Vec<String>) -> Option<String> {
    if targets.is_empty() {
        return None;
    }
    Some(
        targets
            .into_iter()
            .map(|t| {
                let spl: Vec<&str> = t.split("=").collect();
                if spl.len() != 2 {
                    warn!(
                        target: "run",
                        "{} is an invalid target. Must conform to the same format as RUST_LOG.eq \
                         'own_tx=debug'.",
                        t
                    );
                    "".into()
                } else {
                    format!("{}={}", spl[0], validate_log_level(spl[1].into(), spl[0]))
                }
            })
            .filter(|t| !t.is_empty())
            .collect::<Vec<String>>()
            .join(","),
    )
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Write;
    use tempdir::TempDir;
    use aion_types::U256;
    use acore::client::BlockId;
    use acore::miner::PendingSet;
    use super::{to_block_id, to_u256, to_pending_set, to_address, to_addresses, password_from_file,parse_log_target};

    #[test]
    fn test_parse_log_target() {
        // empty
        let v1: Vec<String> = Vec::new();
        // normal
        let v2: Vec<String> = vec!["p2p=iNfo".into(), "net=TraCe".into()];
        // error1
        let v3: Vec<String> = vec!["p2p=iNfo".into(), "23333".into(), "net=TraCe".into()];
        // error2
        let v4: Vec<String> = vec!["p2p=iNfo".into(), "a=b=c".into(), "net=TraCe".into()];
        // error3
        let v5: Vec<String> = vec![
            "p2p=iNfo".into(),
            "k=233".into(),
            "net=TraCe".into(),
            "".into(),
        ];

        assert_eq!(parse_log_target(v1), None);
        assert_eq!(parse_log_target(v2), Some("p2p=iNfo,net=TraCe".into()));
        assert_eq!(parse_log_target(v3), Some("p2p=iNfo,net=TraCe".into()));
        assert_eq!(parse_log_target(v4), Some("p2p=iNfo,net=TraCe".into()));
        assert_eq!(
            parse_log_target(v5),
            Some("p2p=iNfo,k=info,net=TraCe".into())
        );
    }

    #[test]
    fn test_to_block_id() {
        assert_eq!(to_block_id("latest").unwrap(), BlockId::Latest);
        assert_eq!(to_block_id("0").unwrap(), BlockId::Number(0));
        assert_eq!(to_block_id("2").unwrap(), BlockId::Number(2));
        assert_eq!(to_block_id("15").unwrap(), BlockId::Number(15));
        assert_eq!(
            to_block_id("9fc84d84f6a785dc1bd5abacfcf9cbdd3b6afb80c0f799bfb2fd42c44a0c224e")
                .unwrap(),
            BlockId::Hash(
                "9fc84d84f6a785dc1bd5abacfcf9cbdd3b6afb80c0f799bfb2fd42c44a0c224e"
                    .parse()
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_to_u256() {
        assert_eq!(to_u256("0").unwrap(), U256::from(0));
        assert_eq!(to_u256("11").unwrap(), U256::from(11));
        assert_eq!(to_u256("0x11").unwrap(), U256::from(17));
        assert!(to_u256("u").is_err())
    }

    #[test]
    fn test_pending_set() {
        assert_eq!(to_pending_set("cheap").unwrap(), PendingSet::AlwaysQueue);
        assert_eq!(to_pending_set("strict").unwrap(), PendingSet::AlwaysSealing);
        assert_eq!(
            to_pending_set("lenient").unwrap(),
            PendingSet::SealingOrElseQueue
        );
        assert!(to_pending_set("othe").is_err());
    }

    #[test]
    fn test_to_address() {
        assert_eq!(
            to_address(Some(
                "0xD9A111feda3f362f55Ef1744347CDC8Dd9964a41D9A111feda3f362f55Ef1744".into()
            ))
            .unwrap(),
            "D9A111feda3f362f55Ef1744347CDC8Dd9964a41D9A111feda3f362f55Ef1744"
                .parse()
                .unwrap()
        );
        assert_eq!(
            to_address(Some(
                "D9A111feda3f362f55Ef1744347CDC8Dd9964a41D9A111feda3f362f55Ef1744".into()
            ))
            .unwrap(),
            "D9A111feda3f362f55Ef1744347CDC8Dd9964a41D9A111feda3f362f55Ef1744"
                .parse()
                .unwrap()
        );
        assert_eq!(to_address(None).unwrap(), Default::default());
    }

    #[test]
    fn test_to_addresses() {
        let addresses = to_addresses(&vec![
            "0xD9A111feda3f362f55Ef1744347CDC8Dd9964a41D9A111feda3f362f55Ef1744".into(),
            "D9A111feda3f362f55Ef1744347CDC8Dd9964a42D9A111feda3f362f55Ef1744".into(),
        ])
        .unwrap();
        assert_eq!(
            addresses,
            vec![
                "D9A111feda3f362f55Ef1744347CDC8Dd9964a41D9A111feda3f362f55Ef1744"
                    .parse()
                    .unwrap(),
                "D9A111feda3f362f55Ef1744347CDC8Dd9964a42D9A111feda3f362f55Ef1744"
                    .parse()
                    .unwrap(),
            ]
        );
    }

    #[test]
    fn test_password() {
        let tempdir = TempDir::new("").unwrap();
        let path = tempdir.path().join("file");
        let mut file = File::create(&path).unwrap();
        file.write_all(b"a bc ").unwrap();
        assert_eq!(
            password_from_file(path.to_str().unwrap().into())
                .unwrap()
                .as_bytes(),
            b"a bc"
        );
    }

    #[test]
    fn test_password_multiline() {
        let tempdir = TempDir::new("").unwrap();
        let path = tempdir.path().join("file");
        let mut file = File::create(path.as_path()).unwrap();
        file.write_all(
            br#"    password with trailing whitespace
those passwords should be
ignored
but the first password is trimmed

"#,
        )
        .unwrap();
        assert_eq!(
            &password_from_file(path.to_str().unwrap().into()).unwrap(),
            "password with trailing whitespace"
        );
    }
}
