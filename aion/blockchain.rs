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

use std::str::{FromStr, from_utf8};
use std::{io, fs};
use std::io::{BufReader, BufRead};
use std::time::{Instant, Duration};
use std::thread::sleep;
use std::sync::Arc;
use rustc_hex::FromHex;
use bytes::ToPretty;
use rlp::PayloadInfo;
use acore::service::ClientService;
use acore::client::{DatabaseCompactionProfile, VMType, BlockImportError, BlockChainClient, BlockId};
use acore::ImportError;
use acore::miner::Miner;
use acore::verification::queue::VerifierSettings;
use cache::CacheConfig;
use params::{SpecType, Pruning, Switch, fatdb_switch_to_bool};
use helpers::{to_client_config};
use dir::Directories;
use user_defaults::UserDefaults;
use fdlimit;

/// Something that can be converted to milliseconds.
pub trait MillisecondDuration {
    /// Get the value in milliseconds.
    fn as_milliseconds(&self) -> u64;
}

impl MillisecondDuration for Duration {
    fn as_milliseconds(&self) -> u64 {
        self.as_secs() * 1000 + self.subsec_nanos() as u64 / 1_000_000
    }
}

/// blockchain data format
#[derive(Debug, PartialEq)]
pub enum DataFormat {
    /// Hex format
    Hex,
    /// Binary format
    Binary,
}

impl Default for DataFormat {
    fn default() -> Self { DataFormat::Binary }
}

impl FromStr for DataFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "binary" | "bin" => Ok(DataFormat::Binary),
            "hex" => Ok(DataFormat::Hex),
            x => Err(format!("Invalid format: {}", x)),
        }
    }
}

/// Config params for blockchain sub-command
#[derive(Debug, PartialEq)]
pub enum BlockchainCmd {
    /// sub-command `db kill`
    Kill(KillBlockchain),
    /// sub-command `import`
    Import(ImportBlockchain),
    /// sub-command `export`
    Export(ExportBlockchain),
    /// sub-command `revert`
    Revert(RevertBlockchain),
}

/// Config for sub-command `db kill`
#[derive(Debug, PartialEq)]
pub struct KillBlockchain {
    pub spec: SpecType,
    pub dirs: Directories,
    pub pruning: Pruning,
}

/// Config for sub-command `import`
#[derive(Debug, PartialEq)]
pub struct ImportBlockchain {
    pub spec: SpecType,
    pub cache_config: CacheConfig,
    pub dirs: Directories,
    pub file_path: Option<String>,
    pub format: Option<DataFormat>,
    pub pruning: Pruning,
    pub pruning_history: u64,
    pub pruning_memory: usize,
    pub compaction: DatabaseCompactionProfile,
    pub wal: bool,
    pub fat_db: Switch,
    pub vm_type: VMType,
    pub verifier_settings: VerifierSettings,
}

/// Config for sub-command `export`
#[derive(Debug, PartialEq)]
pub struct ExportBlockchain {
    pub spec: SpecType,
    pub cache_config: CacheConfig,
    pub dirs: Directories,
    pub file_path: Option<String>,
    pub format: Option<DataFormat>,
    pub pruning: Pruning,
    pub pruning_history: u64,
    pub pruning_memory: usize,
    pub compaction: DatabaseCompactionProfile,
    pub wal: bool,
    pub fat_db: Switch,
    pub from_block: BlockId,
    pub to_block: BlockId,
}

/// Config for sub-command `revert`
#[derive(Debug, PartialEq)]
pub struct RevertBlockchain {
    pub spec: SpecType,
    pub cache_config: CacheConfig,
    pub dirs: Directories,
    pub pruning: Pruning,
    pub pruning_history: u64,
    pub pruning_memory: usize,
    pub compaction: DatabaseCompactionProfile,
    pub wal: bool,
    pub fat_db: Switch,
    pub to_block: BlockId,
}

/// Execute the blockchain subcommand related code
pub fn execute(cmd: BlockchainCmd) -> Result<(), String> {
    match cmd {
        BlockchainCmd::Kill(kill_cmd) => kill_db(kill_cmd),
        BlockchainCmd::Import(import_cmd) => execute_import(import_cmd),
        BlockchainCmd::Export(export_cmd) => execute_export(export_cmd),
        BlockchainCmd::Revert(revert_cmd) => execute_revert(revert_cmd),
    }
}

/// import blocks from data file
fn execute_import(cmd: ImportBlockchain) -> Result<(), String> {
    let timer = Instant::now();
    // load spec file
    let spec = cmd.spec.spec()?;

    // load genesis hash
    let genesis_hash = spec.genesis_header().hash();

    // database paths
    let db_dirs = cmd.dirs.database(genesis_hash, spec.data_dir.clone());

    // user defaults path
    let user_defaults_path = db_dirs.user_defaults_path();

    // load user defaults
    let mut user_defaults = UserDefaults::load(&user_defaults_path)?;

    fdlimit::raise_fd_limit();

    // select pruning algorithm
    let algorithm = cmd.pruning.to_algorithm(&user_defaults);

    // check if fatdb is on
    let fat_db = fatdb_switch_to_bool(cmd.fat_db, &user_defaults, algorithm)?;

    // prepare client paths.
    let client_path = db_dirs.client_path(algorithm);

    // create dirs used by aion
    cmd.dirs.create_dirs()?;

    // prepare client config
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

    // build client
    let service = ClientService::start(
        client_config,
        &spec,
        &client_path,
        &cmd.dirs.ipc_path(),
        Arc::new(Miner::with_spec(&spec)),
    )
    .map_err(|e| format!("Client service error: {:?}", e))?;

    // free up the spec in memory.
    drop(spec);

    let client = service.client();

    let mut instream: Box<io::Read> = match cmd.file_path {
        Some(f) => {
            Box::new(fs::File::open(&f).map_err(|_| format!("Cannot open given file: {}", f))?)
        }
        None => Box::new(io::stdin()),
    };

    const READAHEAD_BYTES: usize = 8;

    let mut first_bytes: Vec<u8> = vec![0; READAHEAD_BYTES];
    let mut first_read = 0;

    let format = match cmd.format {
        Some(format) => format,
        None => {
            first_read = instream
                .read(&mut first_bytes)
                .map_err(|_| "Error reading from the file/stream.")?;
            match first_bytes[0] {
                0xf9 => DataFormat::Binary,
                _ => DataFormat::Hex,
            }
        }
    };

    let do_import = |bytes| {
        while client.queue_info().is_full() {
            sleep(Duration::from_secs(1));
        }
        match client.import_block(bytes) {
            Err(BlockImportError::Import(ImportError::AlreadyInChain)) => {
                trace!(target: "import","Skipping block already in chain.");
            }
            Err(e) => return Err(format!("Cannot import block: {:?}", e)),
            Ok(_) => {}
        }
        Ok(())
    };

    match format {
        DataFormat::Binary => {
            loop {
                let mut bytes = if first_read > 0 {
                    first_bytes.clone()
                } else {
                    vec![0; READAHEAD_BYTES]
                };
                let n = if first_read > 0 {
                    first_read
                } else {
                    instream
                        .read(&mut bytes)
                        .map_err(|_| "Error reading from the file/stream.")?
                };
                if n == 0 {
                    break;
                }
                first_read = 0;
                let s = PayloadInfo::from(&bytes)
                    .map_err(|e| format!("Invalid RLP in the file/stream: {:?}", e))?
                    .total();
                bytes.resize(s, 0);
                instream
                    .read_exact(&mut bytes[n..])
                    .map_err(|_| "Error reading from the file/stream.")?;
                do_import(bytes)?;
            }
        }
        DataFormat::Hex => {
            for line in BufReader::new(instream).lines() {
                let s = line.map_err(|_| "Error reading from the file/stream.")?;
                let s = if first_read > 0 {
                    from_utf8(&first_bytes)
                        .map_err(|e| format!("Error reading : {}", e))?
                        .to_owned()
                        + &(s[..])
                } else {
                    s
                };
                first_read = 0;
                let bytes = s.from_hex().map_err(|_| "Invalid hex in file/stream.")?;
                do_import(bytes)?;
            }
        }
    }
    client.flush_queue();

    // save user defaults
    user_defaults.pruning = algorithm;
    user_defaults.fat_db = fat_db;
    user_defaults.save(&user_defaults_path)?;

    let report = client.report();

    let ms = timer.elapsed().as_milliseconds();
    info!(
        target: "import",
        "Import completed in {} ms, {} blocks, {} blk/s, {} transactions, {} tx/s, {} Mgas, {} \
         Mgas/s",
        ms,
        report.blocks_imported,
        (report.blocks_imported * 1000) as u64 / ms,
        report.transactions_applied,
        (report.transactions_applied * 1000) as u64 / ms,
        report.gas_processed / From::from(1_000_000),
        (report.gas_processed / From::from(ms * 1000)).low_u64(),
    );
    Ok(())
}

/// run client
fn start_client(
    dirs: Directories,
    spec: SpecType,
    pruning: Pruning,
    pruning_history: u64,
    pruning_memory: usize,
    fat_db: Switch,
    compaction: DatabaseCompactionProfile,
    wal: bool,
    cache_config: CacheConfig,
    require_fat_db: bool,
) -> Result<ClientService, String>
{
    // load spec file
    let spec = spec.spec()?;

    // load genesis hash
    let genesis_hash = spec.genesis_header().hash();

    // database paths
    let db_dirs = dirs.database(genesis_hash, spec.data_dir.clone());

    // user defaults path
    let user_defaults_path = db_dirs.user_defaults_path();

    // load user defaults
    let user_defaults = UserDefaults::load(&user_defaults_path)?;

    fdlimit::raise_fd_limit();

    // select pruning algorithm
    let algorithm = pruning.to_algorithm(&user_defaults);

    // check if fatdb is on
    let fat_db = fatdb_switch_to_bool(fat_db, &user_defaults, algorithm)?;
    if !fat_db && require_fat_db {
        return Err("This command requires Aion to be synced with --fat-db on.".to_owned());
    }

    // prepare client paths.
    let client_path = db_dirs.client_path(algorithm);

    // create dirs used by aion
    dirs.create_dirs()?;

    // prepare client config
    let client_config = to_client_config(
        &cache_config,
        spec.name.to_lowercase(),
        fat_db,
        compaction,
        wal,
        VMType::default(),
        algorithm,
        pruning_history,
        pruning_memory,
    );

    let service = ClientService::start(
        client_config,
        &spec,
        &client_path,
        &dirs.ipc_path(),
        Arc::new(Miner::with_spec(&spec)),
    )
    .map_err(|e| format!("Client service error: {:?}", e))?;

    drop(spec);
    Ok(service)
}

/// export block chain to a data file
fn execute_export(cmd: ExportBlockchain) -> Result<(), String> {
    let timer = Instant::now();
    let service = start_client(
        cmd.dirs,
        cmd.spec,
        cmd.pruning,
        cmd.pruning_history,
        cmd.pruning_memory,
        cmd.fat_db,
        cmd.compaction,
        cmd.wal,
        cmd.cache_config,
        false,
    )?;
    let format = cmd.format.unwrap_or_default();

    let client = service.client();

    let mut out: Box<io::Write> = match cmd.file_path {
        Some(f) => {
            Box::new(
                fs::File::create(&f).map_err(|_| format!("Cannot write to file given: {}", f))?,
            )
        }
        None => Box::new(io::stdout()),
    };

    let from = client
        .block_number(cmd.from_block)
        .ok_or("From block could not be found")?;
    let to = client
        .block_number(cmd.to_block)
        .ok_or("To block could not be found")?;

    if from > to {
        return Err(format!(
            "Invalid value: cannot export blocks from block {} to block {}",
            from, to
        ));
    }

    for i in from..(to + 1) {
        if i % 10000 == 0 {
            info!(target:"export","#{}", i);
        }
        let b = client
            .block(BlockId::Number(i))
            .ok_or("Error exporting incomplete chain")?
            .into_inner();
        match format {
            DataFormat::Binary => {
                out.write(&b)
                    .map_err(|e| format!("Couldn't write to stream. Cause: {}", e))?;
            }
            DataFormat::Hex => {
                out.write_fmt(format_args!("{}", b.pretty()))
                    .map_err(|e| format!("Couldn't write to stream. Cause: {}", e))?;
            }
        }
    }

    let ms = timer.elapsed().as_milliseconds();

    info!(target: "export","Export {} blocks completed in {} ms", to - from + 1, ms);
    Ok(())
}

/// remove specified db
pub fn kill_db(cmd: KillBlockchain) -> Result<(), String> {
    let spec = cmd.spec.spec()?;
    let genesis_hash = spec.genesis_header().hash();
    let db_dirs = cmd.dirs.database(genesis_hash, spec.data_dir);
    let user_defaults_path = db_dirs.user_defaults_path();
    let mut user_defaults = UserDefaults::load(&user_defaults_path)?;
    let algorithm = cmd.pruning.to_algorithm(&user_defaults);
    let dir = db_dirs.db_path(algorithm);
    fs::remove_dir_all(&dir).map_err(|e| format!("Error removing database: {:?}", e))?;
    user_defaults.is_first_launch = true;
    user_defaults.save(&user_defaults_path)?;
    info!(target: "db_kill", "Database {:?} deleted.", &dir);
    Ok(())
}

/// revert db to specified block number
fn execute_revert(cmd: RevertBlockchain) -> Result<(), String> {
    let timer = Instant::now();
    let service = start_client(
        cmd.dirs,
        cmd.spec,
        cmd.pruning,
        cmd.pruning_history,
        cmd.pruning_memory,
        cmd.fat_db,
        cmd.compaction,
        cmd.wal,
        cmd.cache_config,
        false,
    )?;
    let client = service.client();
    let to = client
        .block_number(cmd.to_block)
        .ok_or("To block could not be found")?;
    let from = client
        .block_number(BlockId::Latest)
        .ok_or("Latest block could not be found")?;
    if from == 0 {
        info!(target: "revert", "Empty database, nothing to do");
        return Ok(());
    }
    info!(
        target: "revert",
        "Attempting to revert best block from {} to {} ...",
        from, to
    );
    if to > from {
        return Err(format!(
            "The block #{} is greater than the current best block #{} stored in the database. \
             Cannot move to that block.",
            to, from
        ));
    }
    match client.revert_block(to) {
        Ok(blk) => {
            let ms = timer.elapsed().as_milliseconds();
            info!(target: "revert", "Revert BlockChain to #{} completed in {} ms", blk, ms);
        }
        Err(e) => {
            println!("{}", e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::DataFormat;
    // use super::*;
    // use std::fs::{File, self};
    // use std::io::BufReader;
    // use std::path::Path;

    #[test]
    fn test_data_format_parsing() {
        assert_eq!(DataFormat::Binary, "binary".parse().unwrap());
        assert_eq!(DataFormat::Binary, "bin".parse().unwrap());
        assert_eq!(DataFormat::Hex, "hex".parse().unwrap());
    }
    // Comment out temporarily, cause the size of rawdata is too big.
    // #[test]
    // fn benchtest_import_block() {
    //     let spec: SpecType = "./ethcore/res/aion/mainnet.json".parse().unwrap();
    //     let dir = Directories {
    //         base: "./temp".into(),
    //         db: "./temp/db".into(),
    //         cache: "./temp/cache".into(),
    //         keys: "./temp/keys".into(),
    //     };
    //     let pruning = Pruning::default();
    //     let fatdb = Switch::Auto;
    //     let compaction = DatabaseCompactionProfile::Auto;
    //     let cache_config = CacheConfig::new(128, 8, 40, 25);
    //     // remove database first
    //     {
    //         let test_path = Path::new(&dir.base);
    //         if test_path.exists() {
    //             fs::remove_dir_all(&dir.base).unwrap();
    //         }
    //     }
    //     // start client
    //     let service = start_client(
    //         dir,
    //         spec,
    //         pruning,
    //         64,
    //         32,
    //         fatdb,
    //         compaction,
    //         true,
    //         cache_config,
    //         false,
    //     ).unwrap();
    //     let client = service.client();
    //     // read raw data from file
    //     let file = File::open("./aion/res/block_rawdata.txt").unwrap();
    //     let fin = BufReader::new(file);
    //     let mut v: Vec<Vec<u8>> = vec![];
    //     for line in fin.lines() {
    //         let string = line.unwrap();
    //         v.push(string.from_hex().unwrap());
    //     }

    //     let do_import = |bytes| {
    //         while client.queue_info().is_full() {
    //             sleep(Duration::from_secs(1));
    //         }
    //         match client.import_block(bytes) {
    //             Err(BlockImportError::Import(ImportError::AlreadyInChain)) => {
    //                 trace!("Skipping block already in chain.");
    //             }
    //             Err(e) => return Err(format!("Cannot import block: {:?}", e)),
    //             Ok(_) => {}
    //         }
    //         Ok(())
    //     };
    //     // insert block
    //     let timer = Instant::now();
    //     let mut insert = 0;
    //     for rawdata in v {
    //         match do_import(rawdata) {
    //             Ok(_) => insert += 1,
    //             Err(e) => {
    //                 println!("{:?}", e);
    //             }
    //         }
    //     }
    //     let end = timer.elapsed().as_milliseconds();
    //     // remove database
    //     {
    //         fs::remove_dir_all("./temp").unwrap();
    //     }
    //     println!("insert {} blocks: {} ms", insert, end);
    // }
}
