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

use blake2b_util;

pub trait Blake2b<T> {
    fn blake2b(&self) -> T
    where T: Sized;
}

impl Blake2b<[u8; 32]> for [u8] {
    fn blake2b(&self) -> [u8; 32] {
        let mut blake2b = blake2b_util::Blake2b::new(32);
        let mut result = [0u8; 32];
        blake2b.update(self);
        blake2b.finalize(&mut result);
        result
    }
}
