use super::*;

type KEY = ElasticArray32<u8>;

#[test]
fn should_test_dbtransaction_get() {
    let mut batch = DBTransaction::new();
    let dbop1 = DBOp::Insert {
        key: KEY::from_slice(b"1"),
        value: DBValue::from_slice(b"cat"),
    };
    let dbop2 = DBOp::Insert {
        key: KEY::from_slice(b"2"),
        value: DBValue::from_slice(b"dog"),
    };
    batch.put("test", b"1", b"cat");
    batch.put("test", b"2", b"dog");
    assert_eq!(batch.get("test").unwrap(), dbop1);
    assert_eq!(batch.get_vec("test").unwrap(), vec![dbop1, dbop2]);
}
