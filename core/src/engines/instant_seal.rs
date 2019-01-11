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

use engines::{Engine, Seal};
use aion_machine::{Machine, Transactions};

/// An engine which does not provide any consensus mechanism, just seals blocks internally.
/// Only seals blocks which have transactions.
pub struct InstantSeal<M> {
    machine: M,
}

impl<M> InstantSeal<M> {
    /// Returns new instance of InstantSeal over the given state machine.
    pub fn new(machine: M) -> Self {
        InstantSeal {
            machine: machine,
        }
    }
}

impl<M: Machine> Engine<M> for InstantSeal<M>
where M::LiveBlock: Transactions
{
    fn name(&self) -> &str { "InstantSeal" }

    fn machine(&self) -> &M { &self.machine }

    fn seals_internally(&self) -> Option<bool> { Some(true) }

    fn generate_seal(&self, block: &M::LiveBlock, _parent: &M::Header) -> Seal {
        if block.transactions().is_empty() {
            Seal::None
        } else {
            Seal::Regular(Vec::new())
        }
    }

    fn verify_local_seal(&self, _header: &M::Header) -> Result<(), M::Error> { Ok(()) }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use aion_types::{H520, Address};
    use tests::helpers::*;
    use spec::Spec;
    use header::Header;
    use block::*;
    use engines::Seal;
    use kvdb::MemoryDBRepository;

    #[test]
    fn instant_can_seal() {
        let spec = Spec::new_instant();
        let engine = &*spec.engine;
        let db = spec
            .ensure_db_good(get_temp_state_db(), &Default::default())
            .unwrap();
        let genesis_header = spec.genesis_header();
        let last_hashes = Arc::new(vec![genesis_header.hash()]);
        let b = OpenBlock::new(
            engine,
            Default::default(),
            db,
            &genesis_header,
            None,
            last_hashes,
            Address::default(),
            (3141562.into(), 31415620.into()),
            vec![],
            false,
            Arc::new(MemoryDBRepository::new()),
        )
        .unwrap();
        let b = b.close_and_lock();
        if let Seal::Regular(seal) = engine.generate_seal(b.block(), &genesis_header) {
            assert!(b.try_seal(engine, seal).is_ok());
        }
    }

    #[test]
    fn instant_cant_verify() {
        let engine = Spec::new_instant().engine;
        let mut header: Header = Header::default();

        assert!(engine.verify_block_basic(&header).is_ok());

        header.set_seal(vec![::rlp::encode(&H520::default()).into_vec()]);

        assert!(engine.verify_block_unordered(&header).is_ok());
    }
}
