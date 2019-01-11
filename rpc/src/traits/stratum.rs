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

//! Stratum rpc interface.
use jsonrpc_core::Result;
use aion_types::{H256, U256};
use jsonrpc_macros::Trailing;

use types::{Work, AddressValidation, Info, MiningInfo, TemplateParam, StratumHeader, BlockNumber};

build_rpc_trait! {
    /// Stratum rpc interface.
    pub trait Stratum {
        /// Returns the work of current block
        #[rpc(name = "getblocktemplate")]
        fn work(&self, Trailing<TemplateParam>) -> Result<Work>;

        /// Submit a proof-of-work solution
        #[rpc(name = "submitblock")]
        fn submit_work(&self, String, String, String) -> Result<bool>;

        /// Get information
        #[rpc(name = "getinfo")]
        fn get_info(&self) -> Result<Info>;

        /// Check if address is valid
        #[rpc(name = "validateaddress")]
        fn validate_address(&self, H256) -> Result<AddressValidation>;

        /// Get difficulty
        #[rpc(name = "getdifficulty")]
        fn get_difficulty(&self) -> Result<U256>;

        /// Get mining information
        #[rpc(name = "getmininginfo")]
        fn get_mining_info(&self) -> Result<MiningInfo>;

        /// Get block header by number
        #[rpc(name = "getHeaderByBlockNumber")]
        fn get_block_by_number(&self, BlockNumber) -> Result<StratumHeader>;
    }
}
