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

#![warn(unused_extern_crates)]

use db::{DBTransaction, DbRepository, KeyValueDB, RepositoryConfig, DatabaseConfig};
use rand;
use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Instant;

const SLICE_LEN: usize = 1024 * 1024;
const DB_NAME: &'static str = "test";
const COMPRESS_RATIO: f32 = 0.5;
const TEST_REPEAT: usize = 1;
struct RandomGenerator {
    slice: Vec<u8>,
    position: usize,
}
impl RandomGenerator {
    fn new(compressratio: f32) -> RandomGenerator {
        let totallen = SLICE_LEN + 100;
        let plen = (SLICE_LEN as f32 * compressratio) as usize;
        let plen = if plen < 1 { 1 } else { plen };
        let mut compress_vec = Vec::new();
        for _i in 0..plen {
            let pt = rand::random::<u8>();
            compress_vec.push(pt);
        }
        let mut slice_vec = Vec::new();
        while slice_vec.len() < totallen {
            if slice_vec.len() + compress_vec.len() < totallen {
                slice_vec.extend_from_slice(compress_vec.as_slice());
            } else {
                let restlen = totallen - slice_vec.len();
                slice_vec.extend_from_slice(&compress_vec.as_slice()[0..restlen]);
            }
        }
        RandomGenerator {
            slice: slice_vec,
            position: 0,
        }
    }
    fn generate(&mut self, len: u64) -> Vec<u8> {
        let len = len as usize;
        if len + self.position > self.slice.len() {
            self.position = 0;
        }
        let v = self.slice.as_slice()[self.position..self.position + len].to_vec();
        self.position += len;
        v
    }
}
#[derive(PartialEq)]
enum Order {
    SEQUENTIAL,
    RANDOM,
}
struct BenchmarkTest {
    db: Arc<RwLock<KeyValueDB>>,
    generater: RandomGenerator,
    opCount: u32,
    byteCount: u64,
    startTime: Instant,
    dbpath: String,
    keySizeBytes: u32,
    valueSizeBytes: u32,
    compressionRatio: f32,
    keyCount: u32,
}

impl BenchmarkTest {
    fn new(
        db: Arc<RwLock<KeyValueDB>>,
        generater: RandomGenerator,
        dbpath: String,
        keyCount: u32,
        valueSizeBytes: u32,
    ) -> Self
    {
        BenchmarkTest {
            db,
            generater,
            opCount: 0,
            byteCount: 0,
            startTime: Instant::now(),
            dbpath,
            keySizeBytes: 16,
            valueSizeBytes,
            compressionRatio: COMPRESS_RATIO,
            keyCount,
        }
    }
    // ---------------------------------------------------------------
    // ====================== Timer Utilities ========================
    // ---------------------------------------------------------------
    fn start(&mut self) {
        self.startTime = Instant::now();
        self.byteCount = 0;
        self.opCount = 0;
    }

    fn stop(&mut self, benchmark: String, keyCount: u32, valueSizeBytes: u64, batchSizeBytes: u32) {
        let duration = self.startTime.elapsed();
        let elapsedSeconds =
            duration.as_secs() as f32 + duration.subsec_nanos() as f32 / 1000_000_000 as f32;
        if self.opCount < 1 {
            self.opCount = 1;
        }
        let mut db = self.db.write().unwrap();
        db.close_all();
        let filesize = get_directory_size_bytes(&Path::new(&self.dbpath));
        println!(
            "[{}] keycount (times): {}, valueSizeBytes (bytes): {}, batchSizeBytes (bytes): {}, \
             elapsed_seconds (s): {}, opcount (times): {}, disk_mb (mb): {}, raw_mb (mb): {}",
            benchmark,
            keyCount,
            valueSizeBytes,
            batchSizeBytes,
            elapsedSeconds,
            self.opCount,
            filesize as f32 / (1024f32 * 1024f32),
            self.byteCount as f32 / (1024f32 * 1024f32)
        );
    }

    fn finishedSingleOp(&mut self) { self.opCount += 1; }

    // ---------------------------------------------------------------
    // ====================== Informant ==============================
    // ---------------------------------------------------------------
    fn print_header(&self) {
        print_environment();
        println!("Keys:     {} bytes each", self.keySizeBytes);
        println!(
            "Values:   {} bytes each ({} bytes after compression)",
            self.valueSizeBytes,
            (self.valueSizeBytes as f32 * self.compressionRatio + 0.5) as i32
        );
        println!("Entries:  {}", self.keyCount);
        println!("Compression Ration:   {}", self.compressionRatio);
        println!(
            "RawSize:  {} MB (estimated)",
            ((self.keySizeBytes + self.valueSizeBytes) * self.keyCount) as f32
                / (1024f32 * 1024f32)
        );
        println!(
            "CompressedSize    {} MB (estimated)",
            ((self.keySizeBytes as f32 + self.valueSizeBytes as f32 * self.compressionRatio)
                * self.keyCount as f32)
                / (1024f32 * 1024f32)
        );
        println!("------------------------------------------------\n");
    }

    // ---------------------------------------------------------------
    // ====================== Method  ================================
    // ---------------------------------------------------------------
    fn write(&mut self, order: Order, numEntries: u64, valueSize: u64, entriesPerBatch: u64) {
        if entriesPerBatch < 1 {
            println!("invalid entriesPerBatch");
            return;
        }
        let db = self.db.clone();
        let db = db.read().unwrap();
        // batch insert
        for i in (0..numEntries).step_by(entriesPerBatch as usize) {
            let mut batch = DBTransaction::new();
            for j in 0..entriesPerBatch {
                let k = if order == Order::SEQUENTIAL {
                    i + j
                } else {
                    rand::random::<u64>() % numEntries
                };
                let key = formatNumber(k);
                let value = self.generater.generate(valueSize);
                batch.put(DB_NAME, &key, &value);
                self.byteCount += valueSize + key.len() as u64;
                self.finishedSingleOp();
            }
            let _ = db.write(batch);
        }
    }

    fn overwrite(&mut self, order: Order, numEntries: u64, valueSize: u64, entriesPerBatch: u64) {
        if entriesPerBatch < 1 {
            println!("invalid entriesPerBatch");
            return;
        }
        let db = self.db.clone();
        let db = db.read().unwrap();
        // batch insert
        for i in (0..numEntries).step_by(entriesPerBatch as usize) {
            let mut batch = DBTransaction::new();
            for j in 0..entriesPerBatch {
                let k = if order == Order::SEQUENTIAL {
                    i + j
                } else {
                    rand::random::<u64>() % numEntries
                };
                let key = formatNumber(k);
                let value = self.generater.generate(valueSize);
                // make sure every operation is overwrite
                assert_eq!(db.get(DB_NAME, &key).unwrap().is_some(), true);
                batch.put(DB_NAME, &key, &value);
                self.byteCount += valueSize + key.len() as u64;
                self.finishedSingleOp();
            }
            let _ = db.write(batch);
        }
    }
}

impl Drop for BenchmarkTest {
    fn drop(&mut self) {
        // remove dirs
        let path = Path::new(&self.dbpath);
        let _ = fs::remove_dir_all(path);
    }
}
// ---------------------------------------------------------------
// ====================== Utilities ==============================
// ---------------------------------------------------------------

fn formatNumber(k: u64) -> Vec<u8> {
    let mut slice = [b'0'; 16];
    let mut i = 15;
    let mut k = k;
    while k > 0 {
        let pk = k % 10;
        slice[i] = b'0' + pk as u8;
        i -= 1;
        k /= 10;
    }
    slice.to_vec()
}

fn get_directory_size_bytes(dir: &Path) -> u64 {
    let mut count = 0;
    if dir.exists() {
        count += 4096
    }
    if dir.is_dir() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                count += get_directory_size_bytes(&path);
            } else {
                count += path.metadata().unwrap().len();
            }
        }
    } else if dir.is_file() {
        let metadata = fs::metadata(dir).unwrap();
        count = metadata.len();
    }
    count
}
#[cfg(not(target_os = "linux"))]
fn print_environment() {
    //unimplemented!();
}
#[cfg(target_os = "linux")]
fn print_environment() {
    let mut numberOfCpus = 0;
    let mut cpuType = String::new();
    let mut cacheSize = String::new();
    let s = fs::read_to_string("/proc/cpuinfo").unwrap();
    let v: Vec<String> = s.split("\n").map(|x| x.to_string()).collect();
    for i in v {
        let parts: Vec<String> = i.splitn(2, ":").map(|x| x.to_string()).collect();
        if parts.len() != 2 {
            continue;
        }
        let key = parts.get(0).unwrap().clone();
        let value = parts.get(1).unwrap().clone();
        if key.contains("model name") {
            numberOfCpus += 1;
            cpuType = value;
        } else if key.contains("cache size") {
            cacheSize = value;
        }
    }
    println!("CPU:      {}*{}", numberOfCpus, cpuType);
    println!("CPUCache: {}", cacheSize);
}

fn new_bench(dbpath: String, keyCount: u32, valueSizeBytes: u32) -> BenchmarkTest {
    let dbrepository_configs = vec![RepositoryConfig {
        db_name: DB_NAME.into(),
        db_config: DatabaseConfig::default(),
        db_path: dbpath.clone(),
    }];
    let db = DbRepository::init(dbrepository_configs).unwrap();
    let db = Arc::new(RwLock::new(db));
    let generator = RandomGenerator::new(COMPRESS_RATIO);

    BenchmarkTest::new(db, generator, dbpath, keyCount, valueSizeBytes)
}

// ---------------------------------------------------------------
// ====================== Unit tests =============================
// ---------------------------------------------------------------

#[test]
fn benchtest_fillSequentialKeys() {
    for _i in 0..TEST_REPEAT {
        let mut bench = new_bench("./temp/fsk".into(), 1000_000, 100);
        if _i == 0 {
            bench.print_header();
        }
        bench.start();
        bench.write(Order::SEQUENTIAL, 1000_000, 100, 1);
        bench.stop("benchtest_fillSequentialKeys".into(), 1000_000, 100, 1);
    }
}

#[test]
fn benchtest_fillSequentialBatch1K() {
    for _i in 0..TEST_REPEAT {
        let mut bench = new_bench("./temp/fskb1k".into(), 1000_000, 100);
        bench.start();
        bench.write(Order::SEQUENTIAL, 1000_000, 100, 1000);
        bench.stop(
            "benchtest_fillSequentialBatch1K".into(),
            1000_000,
            100,
            1000,
        );
    }
}

#[test]
fn benchtest_fillRandomkeys() {
    for _i in 0..TEST_REPEAT {
        let mut bench = new_bench("./temp/frk".into(), 1000_000, 100);
        bench.start();
        bench.write(Order::RANDOM, 1000_000, 100, 1);
        bench.stop("benchtest_fillRandomkeys".into(), 1000_000, 100, 1);
    }
}

#[test]
fn benchtest_fillRandomBatch1K() {
    for _i in 0..TEST_REPEAT {
        let mut bench = new_bench("./temp/frkb1k".into(), 1000_000, 100);
        bench.start();
        bench.write(Order::RANDOM, 1000_000, 100, 1000);
        bench.stop("benchtest_fillRandomBatch1K".into(), 1000_000, 100, 1000);
    }
}

#[test]
fn benchtest_fillRandomValue10K() {
    for _i in 0..TEST_REPEAT {
        let mut bench = new_bench("./temp/frv10k".into(), 10_000, 100_000);
        bench.start();
        bench.write(Order::RANDOM, 10_000, 100_000, 1);
        bench.stop("benchtest_fillRandomValue10K".into(), 10_000, 100_000, 1);
    }
}

#[test]
fn benchtest_overwriteRandom() {
    for _i in 0..TEST_REPEAT {
        let mut bench = new_bench("./temp/or".into(), 1000_000, 100);
        bench.write(Order::SEQUENTIAL, 1000_000, 100, 1);
        {
            let db = bench.db.clone();
            let mut db = db.write().unwrap();
            db.close_all();
        }
        let _filesizeinitial = get_directory_size_bytes(Path::new("./temp/or")) as i64;
        {
            let db = bench.db.clone();
            let mut db = db.write().unwrap();
            db.open_all();
        }
        bench.start();
        bench.overwrite(Order::RANDOM, 1000_000, 100, 1);
        bench.stop("benchtest_overwriteRandom".into(), 1000_000, 100, 1);
    }
}

#[test]
fn benchtest_readSequential() {
    for _i in 0..TEST_REPEAT {
        let mut bench = new_bench("./temp/rs".into(), 1000_000, 100);
        bench.write(Order::SEQUENTIAL, 1000_000, 100, 1);
        bench.start();
        let keyCount: u64 = 1000_000;
        let mut byteCount: u64 = 0;
        {
            let db = bench.db.clone();
            let db = db.read().unwrap();
            for k in 0..keyCount {
                let key = formatNumber(k);
                let value = db.get(DB_NAME, &key).unwrap().unwrap();
                byteCount += key.len() as u64 + value.len() as u64;
                bench.finishedSingleOp();
            }
        }
        bench.byteCount = byteCount;
        bench.stop("benchtest_readSequential".into(), 1000_000, 100, 1);
    }
}

#[test]
fn benchtest_readRandom() {
    for _i in 0..TEST_REPEAT {
        let mut bench = new_bench("./temp/rr".into(), 1000_000, 100);
        bench.write(Order::SEQUENTIAL, 1000_000, 100, 1);
        bench.start();
        let keyCount: u64 = 1000_000;
        let mut byteCount: u64 = 0;
        {
            let db = bench.db.clone();
            let db = db.read().unwrap();
            for _k in 0..keyCount {
                let key = formatNumber(rand::random::<u64>() % keyCount);
                let value = db.get(DB_NAME, &key).unwrap().unwrap();
                byteCount += key.len() as u64 + value.len() as u64;
                bench.finishedSingleOp();
            }
        }
        bench.byteCount = byteCount;
        bench.stop("benchtest_readRandom".into(), 1000_000, 100, 1);
    }
}
