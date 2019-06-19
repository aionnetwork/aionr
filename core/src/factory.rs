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

use trie::TrieFactory;
use account_db::Factory as AccountFactory;
use vms::{VMType, Factory, FastVMFactory, AVMFactory};

/// Virtual machine factory
#[derive(Clone)]
pub struct VmFactory {
    fastvm: FastVMFactory,
    avm: AVMFactory,
}

impl VmFactory {
    pub fn create(&mut self, vm: VMType) -> &mut Factory {
        match vm {
            VMType::FastVM => &mut self.fastvm,
            VMType::AVM => &mut self.avm,
        }
    }

    pub fn new() -> Self {
        VmFactory {
            fastvm: FastVMFactory::new(),
            avm: AVMFactory::new(),
        }
    }
}

impl Default for VmFactory {
    fn default() -> Self { VmFactory::new() }
}

/// Collection of factories.
#[derive(Default, Clone)]
pub struct Factories {
    /// factory for evm.
    pub vm: VmFactory,
    /// factory for tries.
    pub trie: TrieFactory,
    /// factory for account databases.
    pub accountdb: AccountFactory,
}
