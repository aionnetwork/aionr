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

//! Personal rpc interface.
use jsonrpc_core::{BoxFuture, Result};
use aion_types::{H256, H768, Address};

use types::{Bytes, TransactionRequest, RichRawTransaction as RpcRichRawTransaction};

build_rpc_trait! {
    /// Personal rpc interface. Safe (read-only) functions.
    pub trait Personal {

        /// Lists all stored accounts
        #[rpc(name = "personal_listAccounts")]
        fn accounts(&self) -> Result<Vec<Address>>;

        /// Creates new account (it becomes new current unlocked account)
        /// Param is the password for the account.
        #[rpc(name = "personal_newAccount")]
        fn new_account(&self, String) -> Result<Address>;

        /// Unlocks specified account for use (can only be one unlocked account at one moment)
        #[rpc(name = "personal_unlockAccount")]
        fn unlock_account(&self, Address, String, Option<u64>) -> Result<bool>;

        /// Locks specified account for use
        #[rpc(name = "personal_lockAccount")]
        fn lock_account(&self, Address, String) -> Result<bool>;

        /// Check if a specified account is unlocked
        #[rpc(name = "personal_isAccountUnlocked")]
        fn is_account_unlocked(&self, Address) -> Result<bool>;

        /// Signs the hash of data with given account signature using the given password to unlock the account during
        /// the request.
        #[rpc(name = "personal_sign")]
        fn sign(&self, Bytes, H256, String) -> BoxFuture<H768>;

//        /// Returns the account associated with the private key that was used to calculate the signature in
//        /// `personal_sign`.
//        #[rpc(name = "personal_ecRecover")]
//        fn ec_recover(&self, Bytes, H520) -> BoxFuture<H256>;

        /// Signs transaction. The account is not unlocked in such case.
        #[rpc(name = "personal_signTransaction")]
        fn sign_transaction(&self, TransactionRequest, String) -> BoxFuture<RpcRichRawTransaction>;

        /// Sends transaction and signs it in single call. The account is not unlocked in such case.
        #[rpc(name = "personal_sendTransaction")]
        fn send_transaction(&self, TransactionRequest, String) -> BoxFuture<H256>;
    }
}
