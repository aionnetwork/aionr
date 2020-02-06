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

use bloomchain::group as bc;
use heapsize::HeapSizeOf;
use ethbloom::Bloom;

/// Represents group of X consecutive blooms.
#[derive(Debug, Clone, RlpEncodableWrapper, RlpDecodableWrapper)]
pub struct BloomGroup {
    blooms: Vec<Bloom>,
}

impl BloomGroup {
    pub fn accrue_bloom_group(&mut self, group: &BloomGroup) {
        for (bloom, other) in self.blooms.iter_mut().zip(group.blooms.iter()) {
            bloom.accrue_bloom(other);
        }
    }
}

impl From<bc::BloomGroup> for BloomGroup {
    fn from(group: bc::BloomGroup) -> Self {
        BloomGroup {
            blooms: group.blooms,
        }
    }
}

impl Into<bc::BloomGroup> for BloomGroup {
    fn into(self) -> bc::BloomGroup {
        bc::BloomGroup {
            blooms: self.blooms,
        }
    }
}

impl HeapSizeOf for BloomGroup {
    fn heap_size_of_children(&self) -> usize { self.blooms.heap_size_of_children() }
}
