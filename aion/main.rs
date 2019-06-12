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

extern crate sync;
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
extern crate blake2b;
extern crate journaldb;
extern crate aion_pb_apiserver as pb;
extern crate tokio;
#[macro_use]
extern crate log as rlog;

#[cfg(feature = "stratum")]
extern crate acore_stratum;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

#[cfg(windows)]
extern crate ws2_32;
#[cfg(windows)]
extern crate winapi;

#[cfg(test)]
extern crate tempdir;

mod account;
mod blockchain;
mod cache;
mod cli;
mod configuration;
mod helpers;
mod modules;
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

#[cfg(windows)]
fn global_cleanup() {
    // We need to cleanup all sockets before spawning another Aion process. This makes shure everything is cleaned up.
    // The loop is required because of internal refernce counter for winsock dll. We don't know how many crates we use do
    // initialize it. There's at least 2 now.
    for _ in 0..10 {
        unsafe {
            ::ws2_32::WSACleanup();
        }
    }
}

#[cfg(not(windows))]
fn global_init() {}

#[cfg(windows)]
fn global_init() {
    // When restarting in the same process this reinits windows sockets.
    unsafe {
        const WS_VERSION: u16 = 0x202;
        let mut wsdata: ::winapi::winsock2::WSADATA = ::std::mem::zeroed();
        ::ws2_32::WSAStartup(WS_VERSION, &mut wsdata);
    }
}

#[cfg(not(windows))]
fn global_cleanup() {}

// Run our version of aion.
// Returns the exit error code.
fn main_direct() -> i32 {
    global_init();

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
    global_cleanup();
    res
}

fn println_trace_main(s: String) {
    if env::var("RUST_LOG")
        .ok()
        .and_then(|s| s.find("main=trace"))
        .is_some()
    {
        println!("{}", s);
    }
}

#[macro_export]
macro_rules! trace_main {
    ($arg:expr) => (println_trace_main($arg.into()));
    ($($arg:tt)*) => (println_trace_main(format!("{}", format_args!($($arg)*))));
}

fn main() {
    panic_hook::set();
    trace_main!("Running direct");
    process::exit(main_direct());
}
