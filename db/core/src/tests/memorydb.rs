use blake2b::blake2b;
use super::*;

#[test]
fn memorydb_remove_and_purge() {
    let hello_bytes = b"Hello world!";
    let hello_key = blake2b(hello_bytes);

    let mut m = MemoryDB::new();
    m.remove(&hello_key);
    assert_eq!(m.raw(&hello_key).unwrap().1, -1);
    m.purge();
    assert_eq!(m.raw(&hello_key).unwrap().1, -1);
    m.insert(hello_bytes);
    assert_eq!(m.raw(&hello_key).unwrap().1, 0);
    m.purge();
    assert_eq!(m.raw(&hello_key), None);

    let mut m = MemoryDB::new();
    assert!(m.remove_and_purge(&hello_key).is_none());
    assert_eq!(m.raw(&hello_key).unwrap().1, -1);
    m.insert(hello_bytes);
    m.insert(hello_bytes);
    assert_eq!(m.raw(&hello_key).unwrap().1, 1);
    assert_eq!(&*m.remove_and_purge(&hello_key).unwrap(), hello_bytes);
    assert_eq!(m.raw(&hello_key), None);
    assert!(m.remove_and_purge(&hello_key).is_none());
}

#[test]
fn consolidate() {
    let mut main = MemoryDB::new();
    let mut other = MemoryDB::new();
    let remove_key = other.insert(b"doggo");
    main.remove(&remove_key);

    let insert_key = other.insert(b"arf");
    main.emplace(insert_key, DBValue::from_slice(b"arf"));

    let negative_remove_key = other.insert(b"negative");
    other.remove(&negative_remove_key); // ref cnt: 0
    other.remove(&negative_remove_key); // ref cnt: -1
    main.remove(&negative_remove_key); // ref cnt: -1

    main.consolidate(other);

    let overlay = main.drain();

    assert_eq!(
        overlay.get(&remove_key).unwrap(),
        &(DBValue::from_slice(b"doggo"), 0)
    );
    assert_eq!(
        overlay.get(&insert_key).unwrap(),
        &(DBValue::from_slice(b"arf"), 2)
    );
    assert_eq!(
        overlay.get(&negative_remove_key).unwrap(),
        &(DBValue::from_slice(b"negative"), -2)
    );
}
