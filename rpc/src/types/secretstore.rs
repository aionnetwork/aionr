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

use aion_types::H512;

use crate::types::Bytes;

/// Encrypted document key.
#[derive(Default, Debug, Serialize, PartialEq)]
#[cfg_attr(test, derive(Deserialize))]
pub struct EncryptedDocumentKey {
    /// Common encryption point. Pass this to Secret Store 'Document key storing session'
    pub common_point: H512,
    /// Ecnrypted point. Pass this to Secret Store 'Document key storing session'.
    pub encrypted_point: H512,
    /// Document key itself, encrypted with passed account public. Pass this to 'secretstore_encrypt'.
    pub encrypted_key: Bytes,
}

#[cfg(test)]
mod tests {
    use serde_json;
    use super::EncryptedDocumentKey;

    #[test]
    fn test_serialize_encrypted_document_key() {
        let initial = EncryptedDocumentKey {
            common_point: 1.into(),
            encrypted_point: 2.into(),
            encrypted_key: vec![3].into(),
        };

        let serialized = serde_json::to_string(&initial).unwrap();
        assert_eq!(serialized, r#"{"common_point":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001","encrypted_point":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002","encrypted_key":"0x03"}"#);

        let deserialized: EncryptedDocumentKey = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.common_point, 1.into());
        assert_eq!(deserialized.encrypted_point, 2.into());
        assert_eq!(deserialized.encrypted_key, vec![3].into());
    }
}
