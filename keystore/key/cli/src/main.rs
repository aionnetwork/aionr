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

extern crate docopt;
extern crate key;
extern crate panic_hook;
extern crate rustc_hex;
extern crate serde;
extern crate threadpool;

#[macro_use]
extern crate serde_derive;

use std::num::ParseIntError;
use std::{env, fmt, process, io};

use docopt::Docopt;
use key::{Ed25519KeyPair, generate_keypair, Error as KeyError, sign_ed25519, verify_signature_ed25519, recover_ed25519, public_to_address_ed25519, Address};
use rustc_hex::{ToHex, FromHexError};

pub const USAGE: &'static str = r#"
keys generator.
  Copyright (c) 2017-2018 Aion foundation.

Usage:
    keygen info <secret> [options]
    keygen generate [options]
    keygen sign <secret> <message>
    keygen verify public <public> <signature> <message>
    keygen verify address <address> <signature> <message>
    keygen [-h | --help]

Options:
    -h, --help         Display this message and exit.
    -s, --secret       Display only the secret.
    -p, --public       Display only the public.
    -a, --address      Display only the address.

Commands:
    info               Display public and address of the secret.
    generate           Generates new random key.
    sign               Sign message using secret.
    verify             Verify signer of the signature.
"#;

#[derive(Debug, Deserialize)]
struct Args {
    cmd_info: bool,
    cmd_generate: bool,
    cmd_sign: bool,
    cmd_verify: bool,
    cmd_public: bool,
    cmd_address: bool,
    arg_secret: String,
    arg_message: String,
    arg_public: String,
    arg_address: String,
    arg_signature: String,
    flag_secret: bool,
    flag_public: bool,
    flag_address: bool,
}

#[derive(Debug)]
enum Error {
    Key(KeyError),
    FromHex(FromHexError),
    ParseInt(ParseIntError),
    Docopt(docopt::Error),
    Io(io::Error),
}

impl From<KeyError> for Error {
    fn from(err: KeyError) -> Self { Error::Key(err) }
}

impl From<FromHexError> for Error {
    fn from(err: FromHexError) -> Self { Error::FromHex(err) }
}

impl From<ParseIntError> for Error {
    fn from(err: ParseIntError) -> Self { Error::ParseInt(err) }
}

impl From<docopt::Error> for Error {
    fn from(err: docopt::Error) -> Self { Error::Docopt(err) }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self { Error::Io(err) }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            Error::Key(ref e) => write!(f, "{}", e),
            Error::FromHex(ref e) => write!(f, "{}", e),
            Error::ParseInt(ref e) => write!(f, "{}", e),
            Error::Docopt(ref e) => write!(f, "{}", e),
            Error::Io(ref e) => write!(f, "{}", e),
        }
    }
}

enum DisplayMode {
    KeyPair,
    Secret,
    Public,
    Address,
}

impl DisplayMode {
    fn new(args: &Args) -> Self {
        if args.flag_secret {
            DisplayMode::Secret
        } else if args.flag_public {
            DisplayMode::Public
        } else if args.flag_address {
            DisplayMode::Address
        } else {
            DisplayMode::KeyPair
        }
    }
}

fn main() {
    panic_hook::set();

    match execute(env::args()) {
        Ok(ok) => println!("{}", ok),
        Err(err) => {
            println!("{}", err);
            process::exit(1);
        }
    }
}

fn display(result: (Ed25519KeyPair, Option<String>), mode: DisplayMode) -> String {
    let keypair = result.0;
    match mode {
        DisplayMode::KeyPair => {
            match result.1 {
                Some(extra_data) => format!("{}\n{}", extra_data, keypair),
                None => format!("{}", keypair),
            }
        }
        DisplayMode::Secret => format!("{}", keypair.secret().to_hex()),
        DisplayMode::Public => format!("{:?}", keypair.public()),
        DisplayMode::Address => format!("{:?}", keypair.address()),
    }
}

fn execute<S, I>(command: I) -> Result<String, Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args: Args = Docopt::new(USAGE).and_then(|d| d.argv(command).deserialize())?;

    return if args.cmd_info {
        let display_mode = DisplayMode::new(&args);
        let secret = args
            .arg_secret
            .parse()
            .map_err(|_| KeyError::InvalidSecret)?;
        Ok(display(
            (Ed25519KeyPair::from_secret(secret)?, None),
            display_mode,
        ))
    } else if args.cmd_generate {
        let display_mode = DisplayMode::new(&args);
        let result = generate_keypair();
        Ok(display((result, None), display_mode))
    } else if args.cmd_sign {
        let secret = args
            .arg_secret
            .parse()
            .map_err(|_| KeyError::InvalidSecret)?;
        let message = args
            .arg_message
            .parse()
            .map_err(|_| KeyError::InvalidMessage)?;
        let signature = sign_ed25519(&secret, &message)?;
        Ok(format!("{}", signature.get_signature().to_hex()))
    } else if args.cmd_verify {
        let signature = args
            .arg_signature
            .parse()
            .map_err(|_| KeyError::InvalidSignature)?;
        let message = args
            .arg_message
            .parse()
            .map_err(|_| KeyError::InvalidMessage)?;
        let ok = if args.cmd_public {
            let public = args
                .arg_public
                .parse()
                .map_err(|_| KeyError::InvalidPublic)?;
            verify_signature_ed25519(public, signature, &message)
        } else if args.cmd_address {
            let address: Address = args
                .arg_address
                .parse()
                .map_err(|_| KeyError::InvalidAddress)?;
            let pk =
                recover_ed25519(&signature, &message).map_err(|_| KeyError::InvalidSignature)?;
            if address == public_to_address_ed25519(&pk) {
                true
            } else {
                false
            }
        } else {
            return Ok(format!("{}", USAGE));
        };
        Ok(format!("{}", ok))
    } else {
        Ok(format!("{}", USAGE))
    };
}

#[cfg(test)]
mod tests {

    use super::execute;

    #[test]
    fn info() {
        let command = vec![
           "key",
           "info",
           "b6d549bb4efc5157ca9d2ad370410877f0cd359eda3fa68fe5f4524af344827bc05fab21975181a8e621476d16fbb06bf80f7d108cb62832a3acad43a3eca90a",
       ].into_iter()
       .map(Into::into)
       .collect::<Vec<String>>();

        let expected =
"secret:  b6d549bb4efc5157ca9d2ad370410877f0cd359eda3fa68fe5f4524af344827bc05fab21975181a8e621476d16fbb06bf80f7d108cb62832a3acad43a3eca90a
public:  c05fab21975181a8e621476d16fbb06bf80f7d108cb62832a3acad43a3eca90a
address: a07f800b473a241878e9d7a3504509feef213bebea3ccea7bcc2149a5b73e9f3".to_owned();
        assert_eq!(execute(command).unwrap(), expected);
    }

    #[test]
    fn sign() {
        let command = vec![
           "key",
           "sign",
           "b6d549bb4efc5157ca9d2ad370410877f0cd359eda3fa68fe5f4524af344827bc05fab21975181a8e621476d16fbb06bf80f7d108cb62832a3acad43a3eca90a",
           "bd50b7370c3f96733b31744c6c45079e7ae6c8d299613246d28ebcef507ec987",
       ].into_iter()
       .map(Into::into)
       .collect::<Vec<String>>();

        let expected = "c05fab21975181a8e621476d16fbb06bf80f7d108cb62832a3acad43a3eca90a4f77b39cb07ed411ed45468beb68b6c7da8eba7b98ea9381cd01c0e381db9b4a766bb3eb58daed4a1af34c62e1fbba97d280a163f213fc8b8fc061162855b804".to_owned();
        assert_eq!(execute(command).unwrap(), expected);
    }

    #[test]
    fn verify_valid_public() {
        let command = vec!["key", "verify", "public", "6da7cf320bdfae063a44dfa9c703323d9dbc1438f2b69b3e642f63ef920f7392", "6da7cf320bdfae063a44dfa9c703323d9dbc1438f2b69b3e642f63ef920f7392a2b74d7ec23f96676a280633acfcff4d0c8c02e5078915f7635f0a2b37dea37f3c16c9fa4023aedca9785d9bfc83cfe8636603f5399719fc1f85184b20887c0d", "a6697e974e6a320f454390be03f74955e8978f1a6971ea6730542e37b66179bc"]
           .into_iter()
           .map(Into::into)
           .collect::<Vec<String>>();

        let expected = "true".to_owned();
        assert_eq!(execute(command).unwrap(), expected);
    }

    #[test]
    fn verify_invalid() {
        let command = vec!["key", "verify", "public", "6da7cf320bdfae063a44dfa9c703323d9dbc1438f2b69b3e642f63ef920f7392", "6da7cf320bdfae063a44dfa9c703323d9dbc1438f2b69b3e642f63ef920f7392a2b74d7ec23f96676a280633acfcff4d0c8c02e5078915f7635f0a2b37dea37f3c16c9fa4023aedca9785d9bfc83cfe8636603f5399719fc1f85184b20887c0d", "a6697e974e6a320f454390be03f74955e8978f1a6971ea6730542e37b66179bd"]
           .into_iter()
           .map(Into::into)
           .collect::<Vec<String>>();

        let expected = "false".to_owned();
        assert_eq!(execute(command).unwrap(), expected);
    }
}
