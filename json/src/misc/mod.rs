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

//! Misc deserialization.

macro_rules! impl_serialization {
    ($key: ty => $name: ty) => {
        impl $name {
            /// Read a hash map of DappId -> $name
            pub fn read<R, S, D>(
                reader: R,
            ) -> Result<::std::collections::HashMap<D, S>, ::serde_json::Error>
            where
                R: ::std::io::Read,
                D: From<$key> + ::std::hash::Hash + Eq,
                S: From<$name> + Clone,
            {
                ::serde_json::from_reader(reader).map(
                    |ok: ::std::collections::HashMap<$key, $name>| {
                        ok.into_iter().map(|(a, m)| (a.into(), m.into())).collect()
                    },
                )
            }

            /// Write a hash map of DappId -> $name
            pub fn write<W, S, D>(
                m: &::std::collections::HashMap<D, S>,
                writer: &mut W,
            ) -> Result<(), ::serde_json::Error>
            where
                W: ::std::io::Write,
                D: Into<$key> + ::std::hash::Hash + Eq + Clone,
                S: Into<$name> + Clone,
            {
                ::serde_json::to_writer(
                    writer,
                    &m.iter()
                        .map(|(a, m)| (a.clone().into(), m.clone().into()))
                        .collect::<::std::collections::HashMap<$key, $name>>(),
                )
            }
        }
    };
}

mod account_meta;

pub use self::account_meta::AccountMeta;
