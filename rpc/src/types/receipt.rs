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

use acore::receipt::{Receipt as EthReceipt, RichReceipt, LocalizedReceipt};
use serde::ser::{Serialize, Serializer, SerializeStruct};
use rustc_hex::ToHex;
use aion_types::{H256, U256, Address};
use ethbloom::Bloom;

use types::{Log, Bytes};
#[derive(Debug, Serialize, PartialEq, Eq, Hash, Clone)]
struct ReceiptLog {
    /// H256
    pub address: H256,
    /// Topics
    pub topics: Vec<H256>,
    /// Data
    pub data: Bytes,
    /// Block Number
    #[serde(rename = "blockNumber")]
    pub block_number: Option<U256>,
    /// Transaction Index
    #[serde(rename = "transactionIndex")]
    pub transaction_index: Option<U256>,
    /// Log Index in Block
    #[serde(rename = "logIndex")]
    pub log_index: Option<U256>,
}

/// Receipt
#[derive(Debug)]
pub struct Receipt {
    /// Transaction Hash
    pub transaction_hash: Option<H256>,
    /// Transaction index
    pub transaction_index: Option<U256>,
    /// Block hash
    pub block_hash: Option<H256>,
    /// Block number
    pub block_number: Option<U256>,
    /// Cumulative gas used
    pub cumulative_gas_used: U256,
    /// Gas used
    pub gas_used: Option<U256>,
    /// Contract address
    pub contract_address: Option<H256>,
    /// Logs
    pub logs: Vec<Log>,
    /// State Root
    pub state_root: Option<H256>,
    /// Logs bloom
    pub logs_bloom: Bloom,
    /// gas price
    pub gas_price: Option<U256>,
    /// gas limit
    pub gas_limit: Option<U256>,
    /// from address
    pub from: Option<Address>,
    /// to address
    pub to: Option<Address>,
    /// output
    pub output: Option<Bytes>,
    /// status
    pub status: Option<String>,
}

impl Serialize for Receipt {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        let mut receipt = serializer.serialize_struct("Receipt", 18)?;
        receipt.serialize_field("transactionHash", &self.transaction_hash)?;
        receipt.serialize_field("transactionIndex", &self.transaction_index)?;
        receipt.serialize_field("blockHash", &self.block_hash)?;
        receipt.serialize_field("blockNumber", &self.block_number)?;
        receipt.serialize_field("cumulativeGasUsed", &self.cumulative_gas_used)?;
        receipt.serialize_field("cumulativeNrgUsed", &self.cumulative_gas_used)?;
        receipt.serialize_field("gasUsed", &self.gas_used)?;
        receipt.serialize_field("nrgUsed", &self.gas_used)?;
        receipt.serialize_field("gasPrice", &self.gas_price)?;
        receipt.serialize_field("nrgPrice", &self.gas_price)?;
        receipt.serialize_field("gasLimit", &self.gas_limit)?;
        receipt.serialize_field("contractAddress", &self.contract_address)?;
        receipt.serialize_field("from", &self.from)?;
        receipt.serialize_field("to", &self.to)?;
        receipt.serialize_field("logsBloom", &self.logs_bloom.0.to_hex())?;
        receipt.serialize_field("root", &self.state_root.clone().map(|x| x.0.to_hex()))?;
        receipt.serialize_field("status", &self.status)?;

        let mut receipt_logs = Vec::new();
        for l in &self.logs {
            let mut rlogs = ReceiptLog {
                address: l.address.clone(),
                topics: l.topics.clone(),
                data: l.data.clone(),
                block_number: l.block_number.clone(),
                transaction_index: l.transaction_index.clone(),
                log_index: l.log_index.clone(),
            };
            receipt_logs.push(rlogs);
        }

        receipt.serialize_field("logs", &receipt_logs)?;
        receipt.end()
    }
}

impl From<LocalizedReceipt> for Receipt {
    fn from(r: LocalizedReceipt) -> Self {
        Receipt {
            transaction_hash: Some(r.transaction_hash),
            transaction_index: Some(r.transaction_index.into()),
            block_hash: Some(r.block_hash),
            block_number: Some(r.block_number.into()),
            cumulative_gas_used: r.cumulative_gas_used,
            gas_used: Some(r.gas_used),
            contract_address: r.contract_address,
            logs: r.logs.into_iter().map(Into::into).collect(),
            state_root: Some(r.state_root),
            logs_bloom: r.log_bloom,
            gas_price: Some(r.gas_price),
            gas_limit: Some(r.gas_limit),
            from: r.from,
            to: r.to,
            output: Some(r.output.into()),
            status: match r.error_message.as_str() {
                "" => Some(String::from("0x1")),
                _ => Some(String::from("0x0")),
            },
        }
    }
}

impl From<RichReceipt> for Receipt {
    fn from(r: RichReceipt) -> Self {
        Receipt {
            transaction_hash: Some(r.transaction_hash),
            transaction_index: Some(r.transaction_index.into()),
            block_hash: None,
            block_number: None,
            cumulative_gas_used: r.cumulative_gas_used,
            gas_used: Some(r.gas_used),
            contract_address: r.contract_address,
            logs: r.logs.into_iter().map(Into::into).collect(),
            state_root: Some(r.state_root),
            logs_bloom: r.log_bloom,
            gas_price: None,
            gas_limit: None,
            from: None,
            to: None,
            output: None,
            status: None,
        }
    }
}

impl From<EthReceipt> for Receipt {
    fn from(r: EthReceipt) -> Self {
        Receipt {
            transaction_hash: None,
            transaction_index: None,
            block_hash: None,
            block_number: None,
            cumulative_gas_used: r.gas_used,
            gas_used: None,
            contract_address: None,
            logs: r.logs().clone().into_iter().map(Into::into).collect(),
            state_root: Some(r.state_root().clone()),
            logs_bloom: r.log_bloom().clone(),
            gas_price: None,
            gas_limit: None,
            from: None,
            to: None,
            output: None,
            status: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json;
    use types::{Log, Receipt, Bytes};

    #[test]
    fn receipt_serialization() {
        let s = r#"{"transactionHash":"0x0000000000000000000000000000000000000000000000000000000000000000","transactionIndex":"0x0","blockHash":"0x4ded588468a978226870a3b44388439a9debfccb7bcae9843b8503fe744cc599","blockNumber":"0x4510c","cumulativeGasUsed":"0x20","cumulativeNrgUsed":"0x20","gasUsed":"0x10","nrgUsed":"0x10","gasPrice":"0x10","nrgPrice":"0x10","gasLimit":"0x10","contractAddress":null,"from":"0xa00a2d0d10ce8a2ea47a76fbb935405df2a12b0e2bc932f188f84b5f16da9c2c","to":"0xa054340a3152d10006b66c4248cfa73e5725056294081c476c0e67ef5ad25334","logsBloom":"0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000f","root":"000000000000000000000000000000000000000000000000000000000000000a","status":null,"logs":[{"address":"0xa00a2d0d10ce8a2ea47a76fbb935405df2a12b0e2bc932f188f84b5f16da9c2c","topics":["0xa6697e974e6a320f454390be03f74955e8978f1a6971ea6730542e37b66179bc","0x4861736852656700000000000000000000000000000000000000000000000000"],"data":"0x","blockNumber":"0x4510c","transactionIndex":"0x0","logIndex":"0x1"}]}"#;

        let receipt = Receipt {
            transaction_hash: Some(0.into()),
            transaction_index: Some(0.into()),
            block_hash: Some(
                "4ded588468a978226870a3b44388439a9debfccb7bcae9843b8503fe744cc599"
                    .parse()
                    .unwrap(),
            ),
            block_number: Some(0x4510c.into()),
            cumulative_gas_used: 0x20.into(),
            gas_used: Some(0x10.into()),
            contract_address: None,
            logs: vec![Log {
                address: "a00a2d0d10ce8a2ea47a76fbb935405df2a12b0e2bc932f188f84b5f16da9c2c"
                    .parse()
                    .unwrap(),
                topics: vec![
                    "a6697e974e6a320f454390be03f74955e8978f1a6971ea6730542e37b66179bc"
                        .parse()
                        .unwrap(),
                    "4861736852656700000000000000000000000000000000000000000000000000"
                        .parse()
                        .unwrap(),
                ],
                data: vec![].into(),
                block_hash: Some(
                    "4ded588468a978226870a3b44388439a9debfccb7bcae9843b8503fe744cc599"
                        .parse()
                        .unwrap(),
                ),
                block_number: Some(0x4510c.into()),
                transaction_hash: Some(0.into()),
                transaction_index: Some(0.into()),
                transaction_log_index: None,
                log_index: Some(1.into()),
            }],
            logs_bloom: 15.into(),
            state_root: Some(10.into()),
            from: Some(
                "a00a2d0d10ce8a2ea47a76fbb935405df2a12b0e2bc932f188f84b5f16da9c2c"
                    .parse()
                    .unwrap(),
            ),
            to: Some(
                "a054340a3152d10006b66c4248cfa73e5725056294081c476c0e67ef5ad25334"
                    .parse()
                    .unwrap(),
            ),
            gas_limit: Some(0x10.into()),
            gas_price: Some(0x10.into()),
            output: Some(Bytes::new(vec![])),
            status: None,
        };

        let serialized = serde_json::to_string(&receipt).unwrap();
        assert_eq!(serialized, s);
    }
}
