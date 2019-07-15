use std::str::FromStr;
use serde_json;
use aion_types;
use hash::H256;
use maybe::MaybeEmpty;

use super::*;

#[test]
fn maybe_deserialization() {
    let s = r#"["", "5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae"]"#;
    let deserialized: Vec<MaybeEmpty<H256>> = serde_json::from_str(s).unwrap();
    assert_eq!(
        deserialized,
        vec![
            MaybeEmpty::None,
            MaybeEmpty::Some(H256(
                aion_types::H256::from_str(
                    "5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae",
                )
                .unwrap(),
            )),
        ]
    );
}
