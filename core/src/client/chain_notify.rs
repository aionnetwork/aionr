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

use aion_types::H256;
use bytes::Bytes;

/// Represents what has to be handled by actor listening to chain events
pub trait ChainNotify: Send + Sync {
    /// fires when chain has new blocks.
    fn new_blocks(
        &self,
        imported: Vec<H256>,
        invalid: Vec<H256>,
        enacted: Vec<H256>,
        retracted: Vec<H256>,
        sealed: Vec<H256>,
        // Block bytes.
        proposed: Vec<Bytes>,
        duration: u64,
    );

    /// fires when chain achieves active mode
    fn start(&self) {}

    /// fires when chain achieves passive mode
    fn stop(&self) {}

    /// fires when chain broadcasts a message
    fn broadcast(&self, _data: Vec<u8>) {}

    /// fires when new transactions are received from a peer
    fn transactions_received(&self, _transactions: &[Bytes]) {}
}
