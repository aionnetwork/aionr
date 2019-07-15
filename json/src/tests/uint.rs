use serde_json;
use aion_types::U256;
use uint::Uint;

#[test]
fn uint_deserialization() {
    let s = r#"["0xa", "10", "", "0x", 0]"#;
    let deserialized: Vec<Uint> = serde_json::from_str(s).unwrap();
    assert_eq!(
        deserialized,
        vec![
            Uint(U256::from(10)),
            Uint(U256::from(10)),
            Uint(U256::from(0)),
            Uint(U256::from(0)),
            Uint(U256::from(0)),
        ]
    );
}

#[test]
fn uint_into() {
    assert_eq!(U256::from(10), Uint(U256::from(10)).into());
}
