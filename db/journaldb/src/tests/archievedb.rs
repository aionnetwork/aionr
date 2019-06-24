use blake2b::blake2b;
use kvdb::{HashStore, DBValue};
use super::*;
use kvdb::{MockDbRepository};

use aion_types::H256;

#[test]
fn insert_same_in_fork() {
    // history is 1
    let mut jdb = ArchiveDB::new(
        Arc::new(MockDbRepository::init(vec!["test".into()])),
        "test",
    );
    let x = jdb.insert(b"X");
    jdb.commit_batch(1, &blake2b(b"1"), None).unwrap();
    jdb.commit_batch(2, &blake2b(b"2"), None).unwrap();
    jdb.commit_batch(3, &blake2b(b"1002a"), Some((1, blake2b(b"1"))))
        .unwrap();
    jdb.commit_batch(4, &blake2b(b"1003a"), Some((2, blake2b(b"2"))))
        .unwrap();

    jdb.remove(&x);
    jdb.commit_batch(3, &blake2b(b"1002b"), Some((1, blake2b(b"1"))))
        .unwrap();
    let x = jdb.insert(b"X");
    jdb.commit_batch(4, &blake2b(b"1003b"), Some((2, blake2b(b"2"))))
        .unwrap();

    jdb.commit_batch(5, &blake2b(b"1004a"), Some((3, blake2b(b"1002a"))))
        .unwrap();
    jdb.commit_batch(6, &blake2b(b"1005a"), Some((4, blake2b(b"1003a"))))
        .unwrap();

    assert!(jdb.contains(&x));
}

#[test]
fn long_history() {
    // history is 3
    let mut jdb = ArchiveDB::new(
        Arc::new(MockDbRepository::init(vec!["test".into()])),
        "test",
    );
    let h = jdb.insert(b"foo");
    jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
    assert!(jdb.contains(&h));
    jdb.remove(&h);
    jdb.commit_batch(1, &blake2b(b"1"), None).unwrap();
    assert!(jdb.contains(&h));
    jdb.commit_batch(2, &blake2b(b"2"), None).unwrap();
    assert!(jdb.contains(&h));
    jdb.commit_batch(3, &blake2b(b"3"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.contains(&h));
    jdb.commit_batch(4, &blake2b(b"4"), Some((1, blake2b(b"1"))))
        .unwrap();
    assert!(jdb.contains(&h));
}

#[test]
#[should_panic]
fn multiple_owed_removal_not_allowed() {
    let mut jdb = ArchiveDB::new(
        Arc::new(MockDbRepository::init(vec!["test".into()])),
        "test",
    );
    let h = jdb.insert(b"foo");
    jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
    assert!(jdb.contains(&h));
    jdb.remove(&h);
    jdb.remove(&h);
    // commit_batch would call journal_under(),
    // and we don't allow multiple owned removals.
    jdb.commit_batch(1, &blake2b(b"1"), None).unwrap();
}

#[test]
fn complex() {
    // history is 1
    let mut jdb = ArchiveDB::new(
        Arc::new(MockDbRepository::init(vec!["test".into()])),
        "test",
    );

    let foo = jdb.insert(b"foo");
    let bar = jdb.insert(b"bar");
    jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
    assert!(jdb.contains(&foo));
    assert!(jdb.contains(&bar));

    jdb.remove(&foo);
    jdb.remove(&bar);
    let baz = jdb.insert(b"baz");
    jdb.commit_batch(1, &blake2b(b"1"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.contains(&foo));
    assert!(jdb.contains(&bar));
    assert!(jdb.contains(&baz));

    let foo = jdb.insert(b"foo");
    jdb.remove(&baz);
    jdb.commit_batch(2, &blake2b(b"2"), Some((1, blake2b(b"1"))))
        .unwrap();
    assert!(jdb.contains(&foo));
    assert!(jdb.contains(&baz));

    jdb.remove(&foo);
    jdb.commit_batch(3, &blake2b(b"3"), Some((2, blake2b(b"2"))))
        .unwrap();
    assert!(jdb.contains(&foo));

    jdb.commit_batch(4, &blake2b(b"4"), Some((3, blake2b(b"3"))))
        .unwrap();
}

#[test]
fn fork() {
    // history is 1
    let mut jdb = ArchiveDB::new(
        Arc::new(MockDbRepository::init(vec!["test".into()])),
        "test",
    );

    let foo = jdb.insert(b"foo");
    let bar = jdb.insert(b"bar");
    jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
    assert!(jdb.contains(&foo));
    assert!(jdb.contains(&bar));

    jdb.remove(&foo);
    let baz = jdb.insert(b"baz");
    jdb.commit_batch(1, &blake2b(b"1a"), Some((0, blake2b(b"0"))))
        .unwrap();

    jdb.remove(&bar);
    jdb.commit_batch(1, &blake2b(b"1b"), Some((0, blake2b(b"0"))))
        .unwrap();

    assert!(jdb.contains(&foo));
    assert!(jdb.contains(&bar));
    assert!(jdb.contains(&baz));

    jdb.commit_batch(2, &blake2b(b"2b"), Some((1, blake2b(b"1b"))))
        .unwrap();
    assert!(jdb.contains(&foo));
}

#[test]
fn overwrite() {
    // history is 1
    let mut jdb = ArchiveDB::new(
        Arc::new(MockDbRepository::init(vec!["test".into()])),
        "test",
    );

    let foo = jdb.insert(b"foo");
    jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
    assert!(jdb.contains(&foo));

    jdb.remove(&foo);
    jdb.commit_batch(1, &blake2b(b"1"), Some((0, blake2b(b"0"))))
        .unwrap();
    jdb.insert(b"foo");
    assert!(jdb.contains(&foo));
    jdb.commit_batch(2, &blake2b(b"2"), Some((1, blake2b(b"1"))))
        .unwrap();
    assert!(jdb.contains(&foo));
    jdb.commit_batch(3, &blake2b(b"2"), Some((0, blake2b(b"2"))))
        .unwrap();
    assert!(jdb.contains(&foo));
}

#[test]
fn fork_same_key() {
    // history is 1
    let mut jdb = ArchiveDB::new(
        Arc::new(MockDbRepository::init(vec!["test".into()])),
        "test",
    );
    jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();

    let foo = jdb.insert(b"foo");
    jdb.commit_batch(1, &blake2b(b"1a"), Some((0, blake2b(b"0"))))
        .unwrap();

    jdb.insert(b"foo");
    jdb.commit_batch(1, &blake2b(b"1b"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.contains(&foo));

    jdb.commit_batch(2, &blake2b(b"2a"), Some((1, blake2b(b"1a"))))
        .unwrap();
    assert!(jdb.contains(&foo));
}

#[test]
fn reopen() {
    let shared_db = Arc::new(MockDbRepository::init(vec!["test".into()]));
    let bar = H256::random();

    let foo = {
        let mut jdb = ArchiveDB::new(shared_db.clone(), "test");
        // history is 1
        let foo = jdb.insert(b"foo");
        jdb.emplace(bar.clone(), DBValue::from_slice(b"bar"));
        jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
        foo
    };

    {
        let mut jdb = ArchiveDB::new(shared_db.clone(), "test");
        jdb.remove(&foo);
        jdb.commit_batch(1, &blake2b(b"1"), Some((0, blake2b(b"0"))))
            .unwrap();
    }

    {
        let mut jdb = ArchiveDB::new(shared_db, "test");
        assert!(jdb.contains(&foo));
        assert!(jdb.contains(&bar));
        jdb.commit_batch(2, &blake2b(b"2"), Some((1, blake2b(b"1"))))
            .unwrap();
    }
}

#[test]
fn reopen_remove() {
    let shared_db = Arc::new(MockDbRepository::init(vec!["test".into()]));

    let foo = {
        let mut jdb = ArchiveDB::new(shared_db.clone(), "test");
        // history is 1
        let foo = jdb.insert(b"foo");
        jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
        jdb.commit_batch(1, &blake2b(b"1"), Some((0, blake2b(b"0"))))
            .unwrap();

        // foo is ancient history.

        jdb.insert(b"foo");
        jdb.commit_batch(2, &blake2b(b"2"), Some((1, blake2b(b"1"))))
            .unwrap();
        foo
    };

    {
        let mut jdb = ArchiveDB::new(shared_db, "test");
        jdb.remove(&foo);
        jdb.commit_batch(3, &blake2b(b"3"), Some((2, blake2b(b"2"))))
            .unwrap();
        assert!(jdb.contains(&foo));
        jdb.remove(&foo);
        jdb.commit_batch(4, &blake2b(b"4"), Some((3, blake2b(b"3"))))
            .unwrap();
        jdb.commit_batch(5, &blake2b(b"5"), Some((4, blake2b(b"4"))))
            .unwrap();
    }
}

#[test]
fn reopen_fork() {
    let shared_db = Arc::new(MockDbRepository::init(vec!["test".into()]));
    let (foo, _, _) = {
        let mut jdb = ArchiveDB::new(shared_db.clone(), "test");
        // history is 1
        let foo = jdb.insert(b"foo");
        let bar = jdb.insert(b"bar");
        jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
        jdb.remove(&foo);
        let baz = jdb.insert(b"baz");
        jdb.commit_batch(1, &blake2b(b"1a"), Some((0, blake2b(b"0"))))
            .unwrap();

        jdb.remove(&bar);
        jdb.commit_batch(1, &blake2b(b"1b"), Some((0, blake2b(b"0"))))
            .unwrap();
        (foo, bar, baz)
    };

    {
        let mut jdb = ArchiveDB::new(shared_db, "test");
        jdb.commit_batch(2, &blake2b(b"2b"), Some((1, blake2b(b"1b"))))
            .unwrap();
        assert!(jdb.contains(&foo));
    }
}

#[test]
fn returns_state() {
    let shared_db = Arc::new(MockDbRepository::init(vec!["test".into()]));

    let key = {
        let mut jdb = ArchiveDB::new(shared_db.clone(), "test");
        let key = jdb.insert(b"foo");
        jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
        key
    };

    {
        let jdb = ArchiveDB::new(shared_db, "test");
        let state = jdb.state(&key);
        assert!(state.is_some());
    }
}

#[test]
fn inject() {
    let mut jdb = ArchiveDB::new(
        Arc::new(MockDbRepository::init(vec!["test".into()])),
        "test",
    );
    let key = jdb.insert(b"dog");
    jdb.inject_batch().unwrap();

    assert_eq!(jdb.get(&key).unwrap(), DBValue::from_slice(b"dog"));
    jdb.remove(&key);
    jdb.inject_batch().unwrap();

    assert!(jdb.get(&key).is_none());
}
