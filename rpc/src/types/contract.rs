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

/// The result of an `eth_compileSolidity` call
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Contract {
    /// Compiled contract code
    pub code: String,
    /// Compiled contract information
    pub info: ContractInfo,
}

/// Compiled contract infomation
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ContractInfo {
    /// abi
    #[serde(rename = "abiDefinition")]
    pub abi: Vec<Abi>,
    /// language version
    #[serde(rename = "languageVersion")]
    pub language_version: String,
    /// language
    pub language: String,
    /// compiler version
    #[serde(rename = "compilerVersion")]
    pub compiler_version: String,
    /// source
    pub source: String,
}

/// Abi information of compiled contract
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Abi {
    /// constant
    pub constant: Option<bool>,
    /// inputs
    pub inputs: Option<Vec<AbiIO>>,
    /// name
    pub name: Option<String>,
    /// outputs
    pub outputs: Option<Vec<AbiIO>>,
    /// payable
    pub payable: Option<bool>,
    /// type
    #[serde(rename = "type")]
    pub abi_type: String,
    /// anonymous
    pub anonymous: Option<bool>,
}

/// Input and output object of abi
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct AbiIO {
    /// name
    pub name: Option<String>,
    /// type
    #[serde(rename = "type")]
    pub abi_io_type: Option<String>,
    /// indexed
    pub indexed: Option<bool>,
}
