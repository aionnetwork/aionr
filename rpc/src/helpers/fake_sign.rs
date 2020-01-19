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

use acore::transaction::{Transaction, SignedTransaction, Action, DEFAULT_TRANSACTION_TYPE};
use aion_types::U256;

use jsonrpc_core::Error;
use crate::helpers::CallRequest;

pub fn sign_call(request: CallRequest) -> Result<SignedTransaction, Error> {
    let gas = match request.gas {
        Some(gas) => gas,
        None => U256::from(2) << 50,
    };
    let from = request.from.unwrap_or(0.into());

    Ok(Transaction::new(
        request.nonce.unwrap_or_else(|| 0.into()),
        request.gas_price.unwrap_or_else(|| 1.into()),
        gas,
        request.to.map_or(Action::Create, Action::Call),
        request.value.unwrap_or(0.into()),
        request.data.unwrap_or_default(),
        request.req_type.unwrap_or(DEFAULT_TRANSACTION_TYPE),
        None,
    )
    .fake_sign(from))
}
