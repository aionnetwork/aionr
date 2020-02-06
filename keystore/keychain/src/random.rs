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

use rand::{Rng, OsRng};

pub trait Random {
    fn random() -> Self
    where Self: Sized;
}

impl Random for [u8; 16] {
    fn random() -> Self {
        let mut result = [0u8; 16];
        let mut rng = OsRng::new().unwrap();
        rng.fill_bytes(&mut result);
        result
    }
}

impl Random for [u8; 32] {
    fn random() -> Self {
        let mut result = [0u8; 32];
        let mut rng = OsRng::new().unwrap();
        rng.fill_bytes(&mut result);
        result
    }
}

/// Generate a random string of given length.
pub fn random_string(length: usize) -> String {
    let mut rng = OsRng::new().expect("Not able to operate without random source.");
    rng.gen_ascii_chars().take(length).collect()
}
