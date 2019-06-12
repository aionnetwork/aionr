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

//! Utils common types and macros global reexport.
#![warn(unused_extern_crates)]

use std::io;

#[macro_export]
macro_rules! vec_into {
    ( $( $x:expr ),* ) => {
        vec![ $( $x.into() ),* ]
    }
}

#[macro_export]
macro_rules! slice_into {
    ( $( $x:expr ),* ) => {
        &[ $( $x.into() ),* ]
    }
}

#[macro_export]
macro_rules! hash_map {
    () => { HashMap::new() };
    ( $( $x:expr => $y:expr ),* ) => {{
        let mut x = HashMap::new();
        $(
            x.insert($x, $y);
        )*
        x
    }}
}

#[macro_export]
macro_rules! hash_map_into {
    () => { HashMap::new() };
    ( $( $x:expr => $y:expr ),* ) => {{
        let mut x = HashMap::new();
        $(
            x.insert($x.into(), $y.into());
        )*
        x
    }}
}

#[macro_export]
macro_rules! map {
    () => { BTreeMap::new() };
    ( $( $x:expr => $y:expr ),* ) => {{
        let mut x = BTreeMap::new();
        $(
            x.insert($x, $y);
        )*
        x
    }}
}

#[macro_export]
macro_rules! map_into {
    () => { BTreeMap::new() };
    ( $( $x:expr => $y:expr ),* ) => {{
        let mut x = BTreeMap::new();
        $(
            x.insert($x.into(), $y.into());
        )*
        x
    }}
}

#[macro_export]
macro_rules! flush {
    ($arg:expr) => ($crate::flush($arg.into()));
    ($($arg:tt)*) => ($crate::flush(format!("{}", format_args!($($arg)*))));
}

#[macro_export]
macro_rules! flushln {
    ($fmt:expr) => (flush!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (flush!(concat!($fmt, "\n"), $($arg)*));
}

#[doc(hidden)]
pub fn flush(s: String) {
    let _ = io::Write::write(&mut io::stdout(), s.as_bytes());
    let _ = io::Write::flush(&mut io::stdout());
}

#[test]
fn test_flush() {
    flushln!("hello_world {:?}", 1);
}
