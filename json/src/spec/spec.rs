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

//! Spec deserialization.

use std::io::Read;
use serde_json;
use serde_json::Error;
use crate::spec::{Params, Genesis, Engine, State};

/// Spec deserialization.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Spec {
    /// Spec name.
    pub name: String,
    /// Special fork name.
    #[serde(rename = "dataDir")]
    pub data_dir: Option<String>,
    /// Engine.
    pub engine: Engine,
    /// Spec params.
    pub params: Params,
    /// Genesis header.
    pub genesis: Genesis,
    /// Genesis state.
    pub accounts: State,
}

impl Spec {
    /// Loads test from json.
    pub fn load<R>(reader: R) -> Result<Self, Error>
    where R: Read {
        serde_json::from_reader(reader)
    }
}
