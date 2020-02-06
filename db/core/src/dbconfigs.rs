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

#[cfg(target_os = "linux")]
use std::path::PathBuf;
use std::path::Path;
/// A single db in dbrepository ' config
#[derive(Clone, Debug)]
pub struct RepositoryConfig {
    /// db name
    pub db_name: String,
    /// db config
    pub db_config: DatabaseConfig,
    /// db path
    pub db_path: String,
}

/// rocksdb config
#[derive(Clone, Debug)]
pub struct DatabaseConfig {
    /// How many files rocksdb can open at one time.
    pub max_open_files: i32,
    /// Memory budget for block based cache size (MB).
    pub memory_budget: usize,
    /// Block based cache size (MB).
    pub block_size: usize,
    /// Compact options.
    pub compact_options: CompactionProfile,
    /// Enable fsync thread.
    pub use_fsync: bool,
    /// Sector align
    pub bytes_per_sync: u64,
    /// Table shard cache (bit).
    pub table_cache_num_shard: i32,
    /// Write buffer number
    pub max_write_buffer_number: i32,
    /// Buffer size
    pub write_buffer_size: usize,
    /// Column Family file size
    pub target_file_size_base: u64,
    /// Merge cut-line
    pub min_write_buffer_number_to_merge: i32,
    /// Stop write to level-0 memtable if * operations aren't be handled.
    pub level_zero_stop_writes_trigger: i32,
    /// Slow down write to level-0 memtable if * operations aren't be handled.
    pub level_zero_slowdown_writes_trigger: i32,
    /// Max compact threads in background.
    pub max_background_compactions: i32,
    /// Max flush threads in background.
    pub max_background_flushes: i32,
    /// Disable auto compaction and handle it manually.
    pub disable_auto_compactions: bool,
    /// Disable wal log may cause data loss during recovery.
    pub wal: bool,
    /// Disable database compress.
    pub disable_compress: bool,
}

impl Default for DatabaseConfig {
    fn default() -> DatabaseConfig {
        DatabaseConfig {
            max_open_files: 4096,
            memory_budget: 128 * 1024 * 1024,
            block_size: 16 * 1024,
            compact_options: CompactionProfile::default(),
            use_fsync: false,
            bytes_per_sync: 4 * 1024 * 1024,
            table_cache_num_shard: 6,
            max_write_buffer_number: 32,
            write_buffer_size: 128 * 1024 * 1024,
            target_file_size_base: 128 * 1024 * 1024,
            min_write_buffer_number_to_merge: 4,
            level_zero_stop_writes_trigger: 2000,
            level_zero_slowdown_writes_trigger: 0,
            max_background_compactions: 4,
            max_background_flushes: 4,
            disable_auto_compactions: false,
            wal: false,
            disable_compress: false,
        }
    }
}

#[cfg(target_os = "linux")]
use regex::Regex;
#[cfg(target_os = "linux")]
use std::process::Command;
#[cfg(target_os = "linux")]
use std::fs::File;

/// Compaction profile for the database settings
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CompactionProfile {
    /// L0-L1 target file size
    pub initial_file_size: u64,
    /// block size
    pub block_size: usize,
    /// rate limiter for background flushes and compactions, bytes/sec, if any
    pub write_rate_limit: Option<u64>,
}

impl Default for CompactionProfile {
    /// Default profile suitable for most storage
    fn default() -> CompactionProfile { CompactionProfile::ssd() }
}

/// Given output of df command return Linux rotational flag file path.
#[cfg(target_os = "linux")]
pub fn rotational_from_df_output(df_out: Vec<u8>) -> Option<PathBuf> {
    use std::str;
    str::from_utf8(df_out.as_slice())
        .ok()
        // Get the drive name.
        .and_then(|df_str| {
            Regex::new(r"/dev/(sd[:alpha:]{1,2})")
                .ok()
                .and_then(|re| re.captures(df_str))
                .and_then(|captures| captures.get(1))
        })
        // Generate path e.g. /sys/block/sda/queue/rotational
        .map(|drive_path| {
            let mut p = PathBuf::from("/sys/block");
            p.push(drive_path.as_str());
            p.push("queue/rotational");
            p
        })
}

impl CompactionProfile {
    /// Attempt to determine the best profile automatically, only Linux for now.
    #[cfg(target_os = "linux")]
    pub fn auto(db_path: &Path) -> CompactionProfile {
        use std::io::Read;
        let hdd_check_file = db_path
            .to_str()
            .and_then(|path_str| Command::new("df").arg(path_str).output().ok())
            .and_then(|df_res| {
                match df_res.status.success() {
                    true => Some(df_res.stdout),
                    false => None,
                }
            })
            .and_then(rotational_from_df_output);
        // Read out the file and match compaction profile.
        if let Some(hdd_check) = hdd_check_file {
            if let Ok(mut file) = File::open(hdd_check.as_path()) {
                let mut buffer = [0; 1];
                if file.read_exact(&mut buffer).is_ok() {
                    // 0 means not rotational.
                    if buffer == [48] {
                        return Self::ssd();
                    }
                    // 1 means rotational.
                    if buffer == [49] {
                        return Self::hdd();
                    }
                }
            }
        }
        // Fallback if drive type was not determined.
        Self::default()
    }

    /// Just default for other platforms.
    #[cfg(not(target_os = "linux"))]
    pub fn auto(_db_path: &Path) -> CompactionProfile { Self::default() }

    /// Default profile suitable for SSD storage
    pub fn ssd() -> CompactionProfile {
        CompactionProfile {
            initial_file_size: 64 * 1024 * 1024,
            block_size: 16 * 1024,
            write_rate_limit: None,
        }
    }

    /// Slow HDD compaction profile
    pub fn hdd() -> CompactionProfile {
        CompactionProfile {
            initial_file_size: 256 * 1024 * 1024,
            block_size: 64 * 1024,
            write_rate_limit: Some(16 * 1024 * 1024),
        }
    }
}
