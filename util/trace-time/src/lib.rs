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

//! Performance timer with logging

extern crate time;
#[macro_use]
extern crate log;

use time::precise_time_ns;

#[macro_export]
macro_rules! trace_time {
    ($name:expr) => {
        let _timer = $crate::PerfTimer::new($name);
    };
}

/// Performance timer with logging. Starts measuring time in the constructor, prints
/// elapsed time in the destructor or when `stop` is called.
pub struct PerfTimer {
    name: &'static str,
    start: u64,
}

impl PerfTimer {
    /// Create an instance with given name.
    pub fn new(name: &'static str) -> PerfTimer {
        PerfTimer {
            name,
            start: precise_time_ns(),
        }
    }
}

impl Drop for PerfTimer {
    fn drop(&mut self) {
        trace!(target: "perf", "{}: {:.2}ms", self.name, (precise_time_ns()  - self.start) as f32 / 1000_000.0);
    }
}

/// corresponding to aion's TimeInstant toEpochMicro.
pub fn to_epoch_micro() -> i64 {
    let now = time::get_time();
    let seconds = now.sec;
    let nanos = now.nsec;
    if seconds < 0 && nanos > 0 {
        let micros = (seconds + 1) * 1000000;
        let adjustment = (nanos / 1000 - 1000000) as i64;
        micros + adjustment
    } else {
        let micros = seconds * 1000000;
        let adjustment = (nanos / 1000) as i64;
        micros + adjustment
    }
}
