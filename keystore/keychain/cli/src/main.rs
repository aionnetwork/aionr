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

extern crate dir;
extern crate docopt;
extern crate keychain;
extern crate num_cpus;
extern crate panic_hook;
extern crate parking_lot;
extern crate rustc_hex;
extern crate serde;

#[macro_use]
extern crate serde_derive;

use std::io::Read;
use std::{env, process, fs, fmt};

use docopt::Docopt;
use keychain::accounts_dir::{KeyDirectory, RootDiskDirectory};
use keychain::ethkey::Address;
use keychain::{EthStore, SimpleSecretStore, import_accounts, StoreAccountRef};

pub const USAGE: &'static str = r#"
key management.
  Copyright (c) 2017-2018 Aion foundation.

Usage:
    keychain insert <secret> <password> [--dir DIR]
    keychain list [--dir DIR]
    keychain import [--src DIR] [--dir DIR]
    keychain remove <address> <password> [--dir DIR]
    keychain sign <address> <password> <message> [--dir DIR]
    keychain [-h | --help]

Options:
    -h, --help               Display this message and exit.
    --dir DIR                Specify the secret store directory. It may be either
                             aion, aion-(chain)
                             or a path [default: aion].
    --src DIR                Specify import source. It may be either
                             aion, aion-(chain)
                             or a path [default: aion].

Commands:
    insert             Save account with password.
    list               List accounts.
    import             Import accounts from src.
    remove             Remove account.
    sign               Sign message.
"#;

#[derive(Debug, Deserialize)]
struct Args {
    cmd_insert: bool,
    cmd_list: bool,
    cmd_import: bool,
    cmd_remove: bool,
    cmd_sign: bool,
    arg_secret: String,
    arg_password: String,
    arg_address: String,
    arg_message: String,
    flag_src: String,
    flag_dir: String,
}

enum Error {
    Ethstore(keychain::Error),
    Docopt(docopt::Error),
}

impl From<keychain::Error> for Error {
    fn from(err: keychain::Error) -> Self { Error::Ethstore(err) }
}

impl From<docopt::Error> for Error {
    fn from(err: docopt::Error) -> Self { Error::Docopt(err) }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Ethstore(ref err) => fmt::Display::fmt(err, f),
            Error::Docopt(ref err) => fmt::Display::fmt(err, f),
        }
    }
}

fn main() {
    panic_hook::set();

    match execute(env::args()) {
        Ok(result) => println!("{}", result),
        Err(err) => {
            println!("{}", err);
            process::exit(1);
        }
    }
}

fn key_dir(location: &str) -> Result<Box<KeyDirectory>, Error> {
    let dir: Box<KeyDirectory> = match location {
        path if path.starts_with("aion") => {
            let chain = path.split('-').nth(1).unwrap_or("aion");
            let path = dir::aion(chain);
            Box::new(RootDiskDirectory::create(path)?)
        }
        path => Box::new(RootDiskDirectory::create(path)?),
    };

    Ok(dir)
}

fn format_accounts(accounts: &[Address]) -> String {
    accounts
        .iter()
        .enumerate()
        .map(|(i, a)| format!("{:2}: 0x{:?}", i, a))
        .collect::<Vec<String>>()
        .join("\n")
}

fn load_password(path: &str) -> Result<String, Error> {
    let mut file = fs::File::open(path).map_err(|e| {
        keychain::Error::Custom(format!("Error opening password file {}: {}", path, e))
    })?;
    let mut password = String::new();
    file.read_to_string(&mut password).map_err(|e| {
        keychain::Error::Custom(format!("Error reading password file {}: {}", path, e))
    })?;
    // drop EOF
    let _ = password.pop();
    Ok(password)
}

fn execute<S, I>(command: I) -> Result<String, Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args: Args = Docopt::new(USAGE).and_then(|d| d.argv(command).deserialize())?;

    let store = EthStore::open(key_dir(&args.flag_dir)?)?;

    return if args.cmd_insert {
        let secret = args
            .arg_secret
            .parse()
            .map_err(|_| keychain::Error::InvalidSecret)?;
        let password = load_password(&args.arg_password)?;
        let account_ref = store.insert_account_ed25519(secret, &password)?;
        Ok(format!("0x{:?}", account_ref.address))
    } else if args.cmd_list {
        let accounts = store.accounts()?;
        let accounts: Vec<_> = accounts.into_iter().map(|a| a.address).collect();
        Ok(format_accounts(&accounts))
    } else if args.cmd_import {
        let src = key_dir(&args.flag_src)?;
        let dst = key_dir(&args.flag_dir)?;
        let accounts = import_accounts(&*src, &*dst)?;
        Ok(format_accounts(&accounts))
    } else if args.cmd_remove {
        let address = args
            .arg_address
            .parse()
            .map_err(|_| keychain::Error::InvalidAccount)?;
        let password = load_password(&args.arg_password)?;
        let account_ref = StoreAccountRef::new(address);
        let ok = store.remove_account(&account_ref, &password).is_ok();
        Ok(format!("{}", ok))
    } else if args.cmd_sign {
        let address = args
            .arg_address
            .parse()
            .map_err(|_| keychain::Error::InvalidAccount)?;
        let message = args
            .arg_message
            .parse()
            .map_err(|_| keychain::Error::InvalidMessage)?;
        let password = load_password(&args.arg_password)?;
        let account_ref = StoreAccountRef::new(address);
        let signature = store.sign_ed25519(&account_ref, &password, &message)?;
        Ok(format!("0x{}", signature))
    } else {
        Ok(format!("{}", USAGE))
    };
}
