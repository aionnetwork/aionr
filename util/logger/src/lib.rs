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

//! Logger for Aion executables
#![warn(unused_extern_crates)]
extern crate log;
extern crate log4rs;

use log::LogLevelFilter;
use log4rs::append::console::ConsoleAppender;
use log4rs::encode::pattern::PatternEncoder;
use log4rs::config::{Appender, Config, Logger, Root};

pub struct LogConfig {
    pub config: Option<String>,
}

fn default_config() -> Result<(), String> {
    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} {h({l})} {t}: {m}{n}",
        )))
        .build();

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .logger(Logger::builder().build("sync", LogLevelFilter::Info))
        .logger(Logger::builder().build("ws", LogLevelFilter::Warn))
        .build(
            Root::builder()
                .appender("stdout")
                .build(LogLevelFilter::Info),
        )
        .unwrap();

    log4rs::init_config(config).expect("init log config");
    Ok(())
}

pub fn setup_compression_log(path: Option<String>) -> Result<(), String> {
    path.map_or_else(
        || default_config(),
        |path| {
            log4rs::init_file(path, Default::default())
                .map_err(|e| format!("log4rs: {}, pls check your log config path", e))
        },
    )
}
