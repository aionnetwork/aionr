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

use aion_types::H256;
use super::DBValue;
use std::collections::HashMap;
pub trait HashStore: AsHashStore + Send + Sync {
    fn keys(&self) -> HashMap<H256, i32>;

    fn get(&self, key: &H256) -> Option<DBValue>;

    fn contains(&self, key: &H256) -> bool;

    fn insert(&mut self, value: &[u8]) -> H256;

    fn emplace(&mut self, key: H256, value: DBValue);

    fn remove(&mut self, key: &H256);
}

pub trait AsHashStore {
    fn as_hashstore(&self) -> &HashStore;
    fn as_hashstore_mut(&mut self) -> &mut HashStore;
}

impl<T: HashStore> AsHashStore for T {
    fn as_hashstore(&self) -> &HashStore { self }
    fn as_hashstore_mut(&mut self) -> &mut HashStore { self }
}

impl<'a> AsHashStore for &'a mut HashStore {
    fn as_hashstore(&self) -> &HashStore { &**self }

    fn as_hashstore_mut(&mut self) -> &mut HashStore { &mut **self }
}
