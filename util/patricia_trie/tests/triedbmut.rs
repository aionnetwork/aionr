extern crate patricia_trie;
extern crate triehash;
extern crate db;
extern crate trie_standardmap;
extern crate acore_bytes;
extern crate aion_types;
extern crate blake2b;
extern crate log;

use db::*;
use patricia_trie::*;
use triehash::trie_root;
use acore_bytes::ToPretty;
use blake2b::BLAKE2B_NULL_RLP;
use trie_standardmap::*;
use aion_types::H256;

fn populate_trie<'db>(
    db: &'db mut HashStore,
    root: &'db mut H256,
    v: &[(Vec<u8>, Vec<u8>)],
) -> TrieDBMut<'db> {
    let mut t = TrieDBMut::new(db, root);
    for i in 0..v.len() {
        let key: &[u8] = &v[i].0;
        let val: &[u8] = &v[i].1;
        t.insert(key, val).unwrap();
    }
    t
}

fn unpopulate_trie<'db>(t: &mut TrieDBMut<'db>, v: &[(Vec<u8>, Vec<u8>)]) {
    for i in v {
        let key: &[u8] = &i.0;
        t.remove(key).unwrap();
    }
}

#[test]
fn playpen() {
    let mut seed = H256::new();
    for test_i in 0..10 {
        if test_i % 50 == 0 {
            println!("{:?} of 10000 stress tests done", test_i);
        }
        let x = StandardMap {
            alphabet: Alphabet::Custom(b"@QWERTYUIOPASDFGHJKLZXCVBNM[/]^_".to_vec()),
            min_key: 5,
            journal_key: 0,
            value_mode: ValueMode::Index,
            count: 100,
        }
            .make_with(&mut seed);

        let real = trie_root(x.clone());
        let mut memdb = MemoryDB::new();
        let mut root = H256::new();
        let mut memtrie = populate_trie(&mut memdb, &mut root, &x);

        memtrie.commit();
        if *memtrie.root() != real {
            println!("TRIE MISMATCH");
            println!("");
            println!("{:?} vs {:?}", memtrie.root(), real);
            for i in &x {
                println!("{:?} -> {:?}", i.0.pretty(), i.1.pretty());
            }
        }
        assert_eq!(*memtrie.root(), real);
        unpopulate_trie(&mut memtrie, &x);
        memtrie.commit();
        if *memtrie.root() != BLAKE2B_NULL_RLP {
            println!("- TRIE MISMATCH");
            println!("");
            println!("{:?} vs {:?}", memtrie.root(), real);
            for i in &x {
                println!("{:?} -> {:?}", i.0.pretty(), i.1.pretty());
            }
        }
        assert_eq!(*memtrie.root(), BLAKE2B_NULL_RLP);
    }
}

#[test]
fn init() {
    let mut memdb = MemoryDB::new();
    let mut root = H256::new();
    let mut t = TrieDBMut::new(&mut memdb, &mut root);
    assert_eq!(*t.root(), BLAKE2B_NULL_RLP);
}

#[test]
fn insert_on_empty() {
    let mut memdb = MemoryDB::new();
    let mut root = H256::new();
    let mut t = TrieDBMut::new(&mut memdb, &mut root);
    t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
    assert_eq!(
        *t.root(),
        trie_root(vec![(vec![0x01u8, 0x23], vec![0x01u8, 0x23])])
    );
}

#[test]
fn remove_to_empty() {
    let big_value = b"00000000000000000000000000000000";

    let mut memdb = MemoryDB::new();
    let mut root = H256::new();
    let mut t1 = TrieDBMut::new(&mut memdb, &mut root);
    t1.insert(&[0x01, 0x23], big_value).unwrap();
    t1.insert(&[0x01, 0x34], big_value).unwrap();
    let mut memdb2 = MemoryDB::new();
    let mut root2 = H256::new();
    let mut t2 = TrieDBMut::new(&mut memdb2, &mut root2);
    t2.insert(&[0x01], big_value).unwrap();
    t2.insert(&[0x01, 0x23], big_value).unwrap();
    t2.insert(&[0x01, 0x34], big_value).unwrap();
    t2.remove(&[0x01]).unwrap();
}

#[test]
fn insert_replace_root() {
    let mut memdb = MemoryDB::new();
    let mut root = H256::new();
    let mut t = TrieDBMut::new(&mut memdb, &mut root);
    t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
    t.insert(&[0x01u8, 0x23], &[0x23u8, 0x45]).unwrap();
    assert_eq!(
        *t.root(),
        trie_root(vec![(vec![0x01u8, 0x23], vec![0x23u8, 0x45])])
    );
}

#[test]
fn insert_make_branch_root() {
    let mut memdb = MemoryDB::new();
    let mut root = H256::new();
    let mut t = TrieDBMut::new(&mut memdb, &mut root);
    t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
    t.insert(&[0x11u8, 0x23], &[0x11u8, 0x23]).unwrap();
    assert_eq!(
        *t.root(),
        trie_root(vec![
            (vec![0x01u8, 0x23], vec![0x01u8, 0x23]),
            (vec![0x11u8, 0x23], vec![0x11u8, 0x23]),
        ])
    );
}

#[test]
fn insert_into_branch_root() {
    let mut memdb = MemoryDB::new();
    let mut root = H256::new();
    let mut t = TrieDBMut::new(&mut memdb, &mut root);
    t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
    t.insert(&[0xf1u8, 0x23], &[0xf1u8, 0x23]).unwrap();
    t.insert(&[0x81u8, 0x23], &[0x81u8, 0x23]).unwrap();
    assert_eq!(
        *t.root(),
        trie_root(vec![
            (vec![0x01u8, 0x23], vec![0x01u8, 0x23]),
            (vec![0x81u8, 0x23], vec![0x81u8, 0x23]),
            (vec![0xf1u8, 0x23], vec![0xf1u8, 0x23]),
        ])
    );
}

#[test]
fn insert_value_into_branch_root() {
    let mut memdb = MemoryDB::new();
    let mut root = H256::new();
    let mut t = TrieDBMut::new(&mut memdb, &mut root);
    t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
    t.insert(&[], &[0x0]).unwrap();
    assert_eq!(
        *t.root(),
        trie_root(vec![
            (vec![], vec![0x0]),
            (vec![0x01u8, 0x23], vec![0x01u8, 0x23]),
        ])
    );
}

#[test]
fn insert_split_leaf() {
    let mut memdb = MemoryDB::new();
    let mut root = H256::new();
    let mut t = TrieDBMut::new(&mut memdb, &mut root);
    t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
    t.insert(&[0x01u8, 0x34], &[0x01u8, 0x34]).unwrap();
    assert_eq!(
        *t.root(),
        trie_root(vec![
            (vec![0x01u8, 0x23], vec![0x01u8, 0x23]),
            (vec![0x01u8, 0x34], vec![0x01u8, 0x34]),
        ])
    );
}

#[test]
fn insert_split_extenstion() {
    let mut memdb = MemoryDB::new();
    let mut root = H256::new();
    let mut t = TrieDBMut::new(&mut memdb, &mut root);
    t.insert(&[0x01, 0x23, 0x45], &[0x01]).unwrap();
    t.insert(&[0x01, 0xf3, 0x45], &[0x02]).unwrap();
    t.insert(&[0x01, 0xf3, 0xf5], &[0x03]).unwrap();
    assert_eq!(
        *t.root(),
        trie_root(vec![
            (vec![0x01, 0x23, 0x45], vec![0x01]),
            (vec![0x01, 0xf3, 0x45], vec![0x02]),
            (vec![0x01, 0xf3, 0xf5], vec![0x03]),
        ])
    );
}

#[test]
fn insert_big_value() {
    let big_value0 = b"00000000000000000000000000000000";
    let big_value1 = b"11111111111111111111111111111111";

    let mut memdb = MemoryDB::new();
    let mut root = H256::new();
    let mut t = TrieDBMut::new(&mut memdb, &mut root);
    t.insert(&[0x01u8, 0x23], big_value0).unwrap();
    t.insert(&[0x11u8, 0x23], big_value1).unwrap();
    assert_eq!(
        *t.root(),
        trie_root(vec![
            (vec![0x01u8, 0x23], big_value0.to_vec()),
            (vec![0x11u8, 0x23], big_value1.to_vec()),
        ])
    );
}

#[test]
fn insert_duplicate_value() {
    let big_value = b"00000000000000000000000000000000";

    let mut memdb = MemoryDB::new();
    let mut root = H256::new();
    let mut t = TrieDBMut::new(&mut memdb, &mut root);
    t.insert(&[0x01u8, 0x23], big_value).unwrap();
    t.insert(&[0x11u8, 0x23], big_value).unwrap();
    assert_eq!(
        *t.root(),
        trie_root(vec![
            (vec![0x01u8, 0x23], big_value.to_vec()),
            (vec![0x11u8, 0x23], big_value.to_vec()),
        ])
    );
}

#[test]
fn test_at_empty() {
    let mut memdb = MemoryDB::new();
    let mut root = H256::new();
    let t = TrieDBMut::new(&mut memdb, &mut root);
    assert_eq!(t.get(&[0x5]), Ok(None));
}

#[test]
fn test_at_one() {
    let mut memdb = MemoryDB::new();
    let mut root = H256::new();
    let mut t = TrieDBMut::new(&mut memdb, &mut root);
    t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
    assert_eq!(
        t.get(&[0x1, 0x23]).unwrap().unwrap(),
        DBValue::from_slice(&[0x1u8, 0x23])
    );
    t.commit();
    assert_eq!(
        t.get(&[0x1, 0x23]).unwrap().unwrap(),
        DBValue::from_slice(&[0x1u8, 0x23])
    );
}

#[test]
fn test_at_three() {
    let mut memdb = MemoryDB::new();
    let mut root = H256::new();
    let mut t = TrieDBMut::new(&mut memdb, &mut root);
    t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
    t.insert(&[0xf1u8, 0x23], &[0xf1u8, 0x23]).unwrap();
    t.insert(&[0x81u8, 0x23], &[0x81u8, 0x23]).unwrap();
    assert_eq!(
        t.get(&[0x01, 0x23]).unwrap().unwrap(),
        DBValue::from_slice(&[0x01u8, 0x23])
    );
    assert_eq!(
        t.get(&[0xf1, 0x23]).unwrap().unwrap(),
        DBValue::from_slice(&[0xf1u8, 0x23])
    );
    assert_eq!(
        t.get(&[0x81, 0x23]).unwrap().unwrap(),
        DBValue::from_slice(&[0x81u8, 0x23])
    );
    assert_eq!(t.get(&[0x82, 0x23]), Ok(None));
    t.commit();
    assert_eq!(
        t.get(&[0x01, 0x23]).unwrap().unwrap(),
        DBValue::from_slice(&[0x01u8, 0x23])
    );
    assert_eq!(
        t.get(&[0xf1, 0x23]).unwrap().unwrap(),
        DBValue::from_slice(&[0xf1u8, 0x23])
    );
    assert_eq!(
        t.get(&[0x81, 0x23]).unwrap().unwrap(),
        DBValue::from_slice(&[0x81u8, 0x23])
    );
    assert_eq!(t.get(&[0x82, 0x23]), Ok(None));
}

#[test]
fn stress() {
    let mut seed = H256::new();
    for _ in 0..50 {
        let x = StandardMap {
            alphabet: Alphabet::Custom(b"@QWERTYUIOPASDFGHJKLZXCVBNM[/]^_".to_vec()),
            min_key: 5,
            journal_key: 0,
            value_mode: ValueMode::Index,
            count: 4,
        }
            .make_with(&mut seed);

        let real = trie_root(x.clone());
        let mut memdb = MemoryDB::new();
        let mut root = H256::new();
        let mut memtrie = populate_trie(&mut memdb, &mut root, &x);
        let mut y = x.clone();
        y.sort_by(|ref a, ref b| a.0.cmp(&b.0));
        let mut memdb2 = MemoryDB::new();
        let mut root2 = H256::new();
        let mut memtrie_sorted = populate_trie(&mut memdb2, &mut root2, &y);
        if *memtrie.root() != real || *memtrie_sorted.root() != real {
            println!("TRIE MISMATCH");
            println!("");
            println!("ORIGINAL... {:?}", memtrie.root());
            for i in &x {
                println!("{:?} -> {:?}", i.0.pretty(), i.1.pretty());
            }
            println!("SORTED... {:?}", memtrie_sorted.root());
            for i in &y {
                println!("{:?} -> {:?}", i.0.pretty(), i.1.pretty());
            }
        }
        assert_eq!(*memtrie.root(), real);
        assert_eq!(*memtrie_sorted.root(), real);
    }
}

#[test]
fn test_trie_existing() {
    let mut root = H256::new();
    let mut db = MemoryDB::new();
    {
        let mut t = TrieDBMut::new(&mut db, &mut root);
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
    }

    {
        let _ = TrieDBMut::from_existing(&mut db, &mut root);
    }
}

#[test]
fn insert_empty() {
    let mut seed = H256::new();
    let x = StandardMap {
        alphabet: Alphabet::Custom(b"@QWERTYUIOPASDFGHJKLZXCVBNM[/]^_".to_vec()),
        min_key: 5,
        journal_key: 0,
        value_mode: ValueMode::Index,
        count: 4,
    }
        .make_with(&mut seed);

    let mut db = MemoryDB::new();
    let mut root = H256::new();
    let mut t = TrieDBMut::new(&mut db, &mut root);
    for &(ref key, ref value) in &x {
        t.insert(key, value).unwrap();
    }

    assert_eq!(*t.root(), trie_root(x.clone()));

    for &(ref key, _) in &x {
        t.insert(key, &[]).unwrap();
    }

    assert!(t.is_empty());
    assert_eq!(*t.root(), BLAKE2B_NULL_RLP);
}

#[test]
fn return_old_values() {
    let mut seed = H256::new();
    let x = StandardMap {
        alphabet: Alphabet::Custom(b"@QWERTYUIOPASDFGHJKLZXCVBNM[/]^_".to_vec()),
        min_key: 5,
        journal_key: 0,
        value_mode: ValueMode::Index,
        count: 4,
    }
        .make_with(&mut seed);

    let mut db = MemoryDB::new();
    let mut root = H256::new();
    let mut t = TrieDBMut::new(&mut db, &mut root);
    for &(ref key, ref value) in &x {
        assert!(t.insert(key, value).unwrap().is_none());
        assert_eq!(
            t.insert(key, value).unwrap(),
            Some(DBValue::from_slice(value))
        );
    }

    for (key, value) in x {
        assert_eq!(t.remove(&key).unwrap(), Some(DBValue::from_slice(&value)));
        assert!(t.remove(&key).unwrap().is_none());
    }
}