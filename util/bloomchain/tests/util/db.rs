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

use std::collections::HashMap;
use bloomchain::{Position, Bloom, BloomDatabase};
use bloomchain::group::{GroupPosition, BloomGroup, BloomGroupDatabase};

#[derive(Default)]
pub struct BloomMemoryDatabase {
    mem: HashMap<Position, Bloom>,
}

impl BloomMemoryDatabase {
    #[allow(dead_code)]
    pub fn insert_blooms(&mut self, blooms: HashMap<Position, Bloom>) { self.mem.extend(blooms); }
}

impl BloomDatabase for BloomMemoryDatabase {
    fn bloom_at(&self, position: &Position) -> Option<Bloom> { self.mem.get(position).cloned() }
}

#[derive(Default)]
pub struct BloomGroupMemoryDatabase {
    mem: HashMap<GroupPosition, BloomGroup>,
}

impl BloomGroupMemoryDatabase {
    #[allow(dead_code)]
    pub fn insert_blooms(&mut self, groups: HashMap<GroupPosition, BloomGroup>) {
        self.mem.extend(groups);
    }
}

impl BloomGroupDatabase for BloomGroupMemoryDatabase {
    fn blooms_at(&self, position: &GroupPosition) -> Option<BloomGroup> {
        self.mem.get(position).cloned()
    }
}
