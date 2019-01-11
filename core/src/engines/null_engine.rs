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

use aion_types::U256;
use engines::Engine;
use aion_machine::{Header, LiveBlock, WithBalances};

/// Params for a null engine.
#[derive(Clone, Default)]
pub struct NullEngineParams {
    /// base reward for a block.
    pub block_reward: U256,
}

impl From<::ajson::spec::NullEngineParams> for NullEngineParams {
    fn from(p: ::ajson::spec::NullEngineParams) -> Self {
        NullEngineParams {
            block_reward: p.block_reward.map_or_else(Default::default, Into::into),
        }
    }
}

/// An engine which does not provide any consensus mechanism and does not seal blocks.
pub struct NullEngine<M> {
    params: NullEngineParams,
    machine: M,
}

impl<M> NullEngine<M> {
    /// Returns new instance of NullEngine with default VM Factory
    pub fn new(params: NullEngineParams, machine: M) -> Self {
        NullEngine {
            params: params,
            machine: machine,
        }
    }
}

impl<M: Default> Default for NullEngine<M> {
    fn default() -> Self { Self::new(Default::default(), Default::default()) }
}

impl<M: WithBalances> Engine<M> for NullEngine<M> {
    fn name(&self) -> &str { "NullEngine" }

    fn machine(&self) -> &M { &self.machine }

    fn on_close_block(&self, block: &mut M::LiveBlock) -> Result<(), M::Error> {
        let author = *LiveBlock::header(&*block).author();

        let reward = self.params.block_reward;
        if reward == U256::zero() {
            return Ok(());
        }

        // Bestow block reward
        let result_block_reward = reward;
        self.machine
            .add_balance(block, &author, &result_block_reward)?;

        // note and trace.
        self.machine
            .note_rewards(block, &[(author, result_block_reward)])
    }

    fn verify_local_seal(&self, _header: &M::Header) -> Result<(), M::Error> { Ok(()) }
}
