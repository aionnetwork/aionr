use serde_json;

use super::*;

#[test]
fn bytes_deserialization() {
    let s = r#"["", "0x", "0x12", "1234", "0x001"]"#;
    let deserialized: Vec<Bytes> = serde_json::from_str(s).unwrap();
    assert_eq!(
        deserialized,
        vec![
            Bytes::new(vec![]),
            Bytes::new(vec![]),
            Bytes::new(vec![0x12]),
            Bytes::new(vec![0x12, 0x34]),
            Bytes::new(vec![0, 1]),
        ]
    );
}

#[test]
fn bytes_into() {
    let bytes = Bytes::new(vec![0xff, 0x11]);
    let v: Vec<u8> = bytes.into();
    assert_eq!(vec![0xff, 0x11], v);
}
