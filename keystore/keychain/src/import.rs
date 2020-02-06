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

use std::collections::HashSet;
use std::path::Path;
use std::fs;

use key::Address;
use accounts_dir::{KeyDirectory, DiskKeyFileManager, KeyFileManager};
use Error;

/// Import an account from a file.
pub fn import_account(path: &Path, dst: &KeyDirectory) -> Result<Address, Error> {
    let key_manager = DiskKeyFileManager;
    let existing_accounts = dst
        .load()?
        .into_iter()
        .map(|a| a.address)
        .collect::<HashSet<_>>();
    let account = fs::File::open(&path)
        .map_err(Into::into)
        .and_then(|mut file| key_manager.read_encoded(&mut file))?;

    let address = account.address.clone();
    if !existing_accounts.contains(&address) {
        dst.insert(account)?;
    }
    Ok(address)
}

/// Import all accounts from one directory to the other.
pub fn import_accounts(src: &KeyDirectory, dst: &KeyDirectory) -> Result<Vec<Address>, Error> {
    let accounts = src.load()?;
    let existing_accounts = dst
        .load()?
        .into_iter()
        .map(|a| a.address)
        .collect::<HashSet<_>>();

    accounts
        .into_iter()
        .filter(|a| !existing_accounts.contains(&a.address))
        .map(|a| {
            let address = a.address.clone();
            dst.insert(a)?;
            Ok(address)
        })
        .collect()
}
