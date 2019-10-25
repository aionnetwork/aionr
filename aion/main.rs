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

extern crate ansi_term;
extern crate ctrlc;
#[macro_use]
extern crate clap;
extern crate dir;
extern crate fdlimit;
extern crate jsonrpc_core;
extern crate num_cpus;
extern crate parking_lot;
extern crate rlp;
extern crate rpassword;
extern crate rustc_hex;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate toml;
extern crate acore;
extern crate acore_bytes as bytes;
extern crate acore_io as io;
extern crate logger;
extern crate aion_types;
extern crate key;
extern crate keychain;
extern crate panic_hook;
extern crate aion_rpc;
extern crate aion_version;
extern crate journaldb;
extern crate tokio;
extern crate p2p;
#[macro_use]
extern crate log as rlog;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

#[cfg(test)]
extern crate tempdir;
#[cfg(test)]
extern crate regex;

mod account;
mod blockchain;
mod cache;
mod cli;
mod configuration;
mod helpers;
mod params;
mod rpc;
mod rpc_apis;
mod run;
mod user_defaults;

use std::{process, env};
use std::io::{self as stdio, Write};
use cli::Args;
use configuration::{Cmd, Execute, Configuration};
use logger::setup_log;

enum PostExecutionAction {
    Print(String),
    Quit,
}

fn execute(command: Execute) -> Result<PostExecutionAction, String> {
    let _ = setup_log(&command.logger).expect("Logger is initialized only once; qed");

    match command.cmd {
        Cmd::Run(run_cmd) => {
            run::execute(run_cmd)?;
            Ok(PostExecutionAction::Quit)
        }
        Cmd::Version => Ok(PostExecutionAction::Print(Args::print_version())),
        Cmd::Account(account_cmd) => {
            account::execute(account_cmd).map(|s| PostExecutionAction::Print(s))
        }
        Cmd::Blockchain(blockchain_cmd) => {
            blockchain::execute(blockchain_cmd).map(|_| PostExecutionAction::Quit)
        }
    }
}

fn start() -> Result<PostExecutionAction, String> {
    let args: Vec<String> = env::args().collect();
    let conf = Configuration::parse(&args).unwrap_or_else(|e| e.exit());
    let cmd = conf.into_command()?;
    execute(cmd)
}

fn main() {
    panic_hook::set();
    let res = match start() {
        Ok(result) => {
            match result {
                PostExecutionAction::Print(s) => {
                    println!("{}", s);
                    0
                }
                PostExecutionAction::Quit => 0,
            }
        }
        Err(err) => {
            writeln!(&mut stdio::stderr(), "{}", err).expect("StdErr available; qed");
            1
        }
    };
    process::exit(res);
}
