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

#![warn(unused_extern_crates)]
//! Custom panic hook with bug report link

extern crate backtrace;

use std::io::{self, Write};
use std::panic::{self, PanicInfo};
use std::thread;
use backtrace::Backtrace;

/// Set the panic hook
pub fn set() {
    if cfg!(debug_assertions) {
        // debug is enabled, do not set panic hook.
    } else {
        panic::set_hook(Box::new(panic_hook));
    }
}

static ABOUT_PANIC: &str = "
This is a bug. Please report it at:

    https://github.com/aionnetwork/aionr/issues/new";

fn panic_hook(info: &PanicInfo) {
    let location = info.location();
    let file = location.as_ref().map(|l| l.file()).unwrap_or("<unknown>");
    let line = location.as_ref().map(|l| l.line()).unwrap_or(0);

    let msg = match info.payload().downcast_ref::<&'static str>() {
        Some(s) => *s,
        None => {
            match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<Any>",
            }
        }
    };

    let thread = thread::current();
    let name = thread.name().unwrap_or("<unnamed>");

    let backtrace = Backtrace::new();

    let mut stderr = io::stderr();

    let _ = writeln!(stderr, "");
    let _ = writeln!(stderr, "====================");
    let _ = writeln!(stderr, "");
    let _ = writeln!(stderr, "{:?}", backtrace);
    let _ = writeln!(stderr, "");
    let _ = writeln!(
        stderr,
        "Thread '{}' panicked at '{}', {}:{}",
        name, msg, file, line
    );

    let _ = writeln!(stderr, "{}", ABOUT_PANIC);
}
