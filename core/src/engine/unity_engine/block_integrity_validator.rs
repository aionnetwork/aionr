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

use triehash::ordered_trie_root;
use header::Header;
use types::error::{BlockError, Error};
use unexpected::Mismatch;
use rlp::Encodable;
use transaction::UnverifiedTransaction;

pub trait BlockIntegrityValidator {
    fn validate(&self, txs: &Vec<UnverifiedTransaction>, header: &Header) -> Result<(), Error>;
}

pub struct TxRootValidator;
impl BlockIntegrityValidator for TxRootValidator {
    /// Verify block data against header: transactions root and uncles hash.
    fn validate(&self, txs: &Vec<UnverifiedTransaction>, header: &Header) -> Result<(), Error> {
        let expected_root = &ordered_trie_root(txs.iter().map(|tx| tx.rlp_bytes()));
        let transactions_root = header.transactions_root();

        if expected_root != transactions_root {
            return Err(From::from(BlockError::InvalidTransactionsRoot(Mismatch {
                expected: expected_root.clone(),
                found: transactions_root.clone(),
            })));
        }
        Ok(())
    }
}
