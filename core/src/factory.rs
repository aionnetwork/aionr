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
use db::Factory as AccountFactory;
use vms::{FastVMFactory, AVMFactory};

/// Virtual machine factory
#[derive(Clone)]
pub struct VmFactory {
    fastvm: Option<FastVMFactory>,
    avm: Option<AVMFactory>,
}

impl VmFactory {
    pub fn create_avm(&mut self) -> AVMFactory {
        match self.avm {
            None => self.avm = Some(AVMFactory::new()),
            _ => {}
        }
        self.avm.clone().unwrap()
    }

    pub fn create_fvm(&mut self) -> FastVMFactory {
        match self.fastvm {
            None => self.fastvm = Some(FastVMFactory::new()),
            _ => {}
        }
        self.fastvm.clone().unwrap()
    }

    pub fn new() -> Self {
        VmFactory {
            avm: Some(AVMFactory::new()),
            fastvm: Some(FastVMFactory::new()),
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
