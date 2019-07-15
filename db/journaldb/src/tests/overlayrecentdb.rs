use blake2b::blake2b;
use super::*;
use kvdb::{HashStore, DBValue};
use logger::init_log;
use {kvdb::MockDbRepository, JournalDB, kvdb::DBTransaction};

use aion_types::H256;

fn new_db() -> OverlayRecentDB {
    let backing = Arc::new(MockDbRepository::init(vec!["test".into()]));
    OverlayRecentDB::new(backing, "test")
}

#[test]
fn insert_same_in_fork() {
    // history is 1
    let mut jdb = new_db();

    let x = jdb.insert(b"X");
    jdb.commit_batch(1, &blake2b(b"1"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());
    jdb.commit_batch(2, &blake2b(b"2"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());
    jdb.commit_batch(3, &blake2b(b"1002a"), Some((1, blake2b(b"1"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    jdb.commit_batch(4, &blake2b(b"1003a"), Some((2, blake2b(b"2"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.remove(&x);
    jdb.commit_batch(3, &blake2b(b"1002b"), Some((1, blake2b(b"1"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    let x = jdb.insert(b"X");
    jdb.commit_batch(4, &blake2b(b"1003b"), Some((2, blake2b(b"2"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.commit_batch(5, &blake2b(b"1004a"), Some((3, blake2b(b"1002a"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    jdb.commit_batch(6, &blake2b(b"1005a"), Some((4, blake2b(b"1003a"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    assert!(jdb.contains(&x));
}

#[test]
fn long_history() {
    // history is 3
    let mut jdb = new_db();
    let h = jdb.insert(b"foo");
    jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(jdb.contains(&h));
    jdb.remove(&h);
    jdb.commit_batch(1, &blake2b(b"1"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(jdb.contains(&h));
    jdb.commit_batch(2, &blake2b(b"2"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(jdb.contains(&h));
    jdb.commit_batch(3, &blake2b(b"3"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(jdb.contains(&h));
    jdb.commit_batch(4, &blake2b(b"4"), Some((1, blake2b(b"1"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(!jdb.contains(&h));
}

#[test]
fn complex() {
    // history is 1
    let mut jdb = new_db();

    let foo = jdb.insert(b"foo");
    let bar = jdb.insert(b"bar");
    jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(jdb.contains(&foo));
    assert!(jdb.contains(&bar));

    jdb.remove(&foo);
    jdb.remove(&bar);
    let baz = jdb.insert(b"baz");
    jdb.commit_batch(1, &blake2b(b"1"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(jdb.contains(&foo));
    assert!(jdb.contains(&bar));
    assert!(jdb.contains(&baz));

    let foo = jdb.insert(b"foo");
    jdb.remove(&baz);
    jdb.commit_batch(2, &blake2b(b"2"), Some((1, blake2b(b"1"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(jdb.contains(&foo));
    assert!(!jdb.contains(&bar));
    assert!(jdb.contains(&baz));

    jdb.remove(&foo);
    jdb.commit_batch(3, &blake2b(b"3"), Some((2, blake2b(b"2"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(jdb.contains(&foo));
    assert!(!jdb.contains(&bar));
    assert!(!jdb.contains(&baz));

    jdb.commit_batch(4, &blake2b(b"4"), Some((3, blake2b(b"3"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(!jdb.contains(&foo));
    assert!(!jdb.contains(&bar));
    assert!(!jdb.contains(&baz));
}

#[test]
fn fork() {
    // history is 1
    let mut jdb = new_db();

    let foo = jdb.insert(b"foo");
    let bar = jdb.insert(b"bar");
    jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(jdb.contains(&foo));
    assert!(jdb.contains(&bar));

    jdb.remove(&foo);
    let baz = jdb.insert(b"baz");
    jdb.commit_batch(1, &blake2b(b"1a"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.remove(&bar);
    jdb.commit_batch(1, &blake2b(b"1b"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    assert!(jdb.contains(&foo));
    assert!(jdb.contains(&bar));
    assert!(jdb.contains(&baz));

    jdb.commit_batch(2, &blake2b(b"2b"), Some((1, blake2b(b"1b"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(jdb.contains(&foo));
    assert!(!jdb.contains(&baz));
    assert!(!jdb.contains(&bar));
}

#[test]
fn overwrite() {
    // history is 1
    let mut jdb = new_db();

    let foo = jdb.insert(b"foo");
    jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(jdb.contains(&foo));

    jdb.remove(&foo);
    jdb.commit_batch(1, &blake2b(b"1"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    jdb.insert(b"foo");
    assert!(jdb.contains(&foo));
    jdb.commit_batch(2, &blake2b(b"2"), Some((1, blake2b(b"1"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(jdb.contains(&foo));
    jdb.commit_batch(3, &blake2b(b"2"), Some((0, blake2b(b"2"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(jdb.contains(&foo));
}

#[test]
fn fork_same_key_one() {
    let mut jdb = new_db();
    jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());

    let foo = jdb.insert(b"foo");
    jdb.commit_batch(1, &blake2b(b"1a"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.insert(b"foo");
    jdb.commit_batch(1, &blake2b(b"1b"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.insert(b"foo");
    jdb.commit_batch(1, &blake2b(b"1c"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    assert!(jdb.contains(&foo));

    jdb.commit_batch(2, &blake2b(b"2a"), Some((1, blake2b(b"1a"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(jdb.contains(&foo));
}

#[test]
fn fork_same_key_other() {
    let mut jdb = new_db();

    jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());

    let foo = jdb.insert(b"foo");
    jdb.commit_batch(1, &blake2b(b"1a"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.insert(b"foo");
    jdb.commit_batch(1, &blake2b(b"1b"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.insert(b"foo");
    jdb.commit_batch(1, &blake2b(b"1c"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    assert!(jdb.contains(&foo));

    jdb.commit_batch(2, &blake2b(b"2b"), Some((1, blake2b(b"1b"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(jdb.contains(&foo));
}

#[test]
fn fork_ins_del_ins() {
    let mut jdb = new_db();

    jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());

    let foo = jdb.insert(b"foo");
    jdb.commit_batch(1, &blake2b(b"1"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.remove(&foo);
    jdb.commit_batch(2, &blake2b(b"2a"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.remove(&foo);
    jdb.commit_batch(2, &blake2b(b"2b"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.insert(b"foo");
    jdb.commit_batch(3, &blake2b(b"3a"), Some((1, blake2b(b"1"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.insert(b"foo");
    jdb.commit_batch(3, &blake2b(b"3b"), Some((1, blake2b(b"1"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.commit_batch(4, &blake2b(b"4a"), Some((2, blake2b(b"2a"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.commit_batch(5, &blake2b(b"5a"), Some((3, blake2b(b"3a"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
}

#[test]
fn reopen() {
    let shared_db = Arc::new(MockDbRepository::init(vec!["test".into()]));
    let bar = H256::random();

    let foo = {
        let mut jdb = OverlayRecentDB::new(shared_db.clone(), "test");
        // history is 1
        let foo = jdb.insert(b"foo");
        jdb.emplace(bar.clone(), DBValue::from_slice(b"bar"));
        jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
        assert!(jdb.can_reconstruct_refs());
        foo
    };

    {
        let mut jdb = OverlayRecentDB::new(shared_db.clone(), "test");
        jdb.remove(&foo);
        jdb.commit_batch(1, &blake2b(b"1"), Some((0, blake2b(b"0"))))
            .unwrap();
        assert!(jdb.can_reconstruct_refs());
    }

    {
        let mut jdb = OverlayRecentDB::new(shared_db.clone(), "test");
        assert!(jdb.contains(&foo));
        assert!(jdb.contains(&bar));
        jdb.commit_batch(2, &blake2b(b"2"), Some((1, blake2b(b"1"))))
            .unwrap();
        assert!(jdb.can_reconstruct_refs());
        assert!(!jdb.contains(&foo));
    }
}

#[test]
fn insert_delete_insert_delete_insert_expunge() {
    init_log();
    let mut jdb = new_db();

    // history is 4
    let foo = jdb.insert(b"foo");
    jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());
    jdb.remove(&foo);
    jdb.commit_batch(1, &blake2b(b"1"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());
    jdb.insert(b"foo");
    jdb.commit_batch(2, &blake2b(b"2"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());
    jdb.remove(&foo);
    jdb.commit_batch(3, &blake2b(b"3"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());
    jdb.insert(b"foo");
    jdb.commit_batch(4, &blake2b(b"4"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    // expunge foo
    jdb.commit_batch(5, &blake2b(b"5"), Some((1, blake2b(b"1"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
}

#[test]
fn forked_insert_delete_insert_delete_insert_expunge() {
    init_log();
    let mut jdb = new_db();

    // history is 4
    let foo = jdb.insert(b"foo");
    jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.remove(&foo);
    jdb.commit_batch(1, &blake2b(b"1a"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.remove(&foo);
    jdb.commit_batch(1, &blake2b(b"1b"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.insert(b"foo");
    jdb.commit_batch(2, &blake2b(b"2a"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.insert(b"foo");
    jdb.commit_batch(2, &blake2b(b"2b"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.remove(&foo);
    jdb.commit_batch(3, &blake2b(b"3a"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.remove(&foo);
    jdb.commit_batch(3, &blake2b(b"3b"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.insert(b"foo");
    jdb.commit_batch(4, &blake2b(b"4a"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.insert(b"foo");
    jdb.commit_batch(4, &blake2b(b"4b"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    // expunge foo
    jdb.commit_batch(5, &blake2b(b"5"), Some((1, blake2b(b"1a"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
}

#[test]
fn broken_assert() {
    let mut jdb = new_db();

    let foo = jdb.insert(b"foo");
    jdb.commit_batch(1, &blake2b(b"1"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    // foo is ancient history.

    jdb.remove(&foo);
    jdb.commit_batch(2, &blake2b(b"2"), Some((1, blake2b(b"1"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.insert(b"foo");
    jdb.commit_batch(3, &blake2b(b"3"), Some((2, blake2b(b"2"))))
        .unwrap(); // BROKEN
    assert!(jdb.can_reconstruct_refs());
    assert!(jdb.contains(&foo));

    jdb.remove(&foo);
    jdb.commit_batch(4, &blake2b(b"4"), Some((3, blake2b(b"3"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.commit_batch(5, &blake2b(b"5"), Some((4, blake2b(b"4"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    assert!(!jdb.contains(&foo));
}

#[test]
fn reopen_test() {
    let mut jdb = new_db();
    // history is 4
    let foo = jdb.insert(b"foo");
    jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());
    jdb.commit_batch(1, &blake2b(b"1"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());
    jdb.commit_batch(2, &blake2b(b"2"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());
    jdb.commit_batch(3, &blake2b(b"3"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());
    jdb.commit_batch(4, &blake2b(b"4"), Some((0, blake2b(b"0"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    // foo is ancient history.

    jdb.insert(b"foo");
    let bar = jdb.insert(b"bar");
    jdb.commit_batch(5, &blake2b(b"5"), Some((1, blake2b(b"1"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    jdb.remove(&foo);
    jdb.remove(&bar);
    jdb.commit_batch(6, &blake2b(b"6"), Some((2, blake2b(b"2"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
    jdb.insert(b"foo");
    jdb.insert(b"bar");
    jdb.commit_batch(7, &blake2b(b"7"), Some((3, blake2b(b"3"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());
}

#[test]
fn reopen_remove_three() {
    init_log();

    let shared_db = Arc::new(MockDbRepository::init(vec!["test".into()]));
    let foo = blake2b(b"foo");

    {
        let mut jdb = OverlayRecentDB::new(shared_db.clone(), "test");
        // history is 1
        jdb.insert(b"foo");
        jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
        assert!(jdb.can_reconstruct_refs());
        jdb.commit_batch(1, &blake2b(b"1"), None).unwrap();
        assert!(jdb.can_reconstruct_refs());

        // foo is ancient history.

        jdb.remove(&foo);
        jdb.commit_batch(2, &blake2b(b"2"), Some((0, blake2b(b"0"))))
            .unwrap();
        assert!(jdb.can_reconstruct_refs());
        assert!(jdb.contains(&foo));

        jdb.insert(b"foo");
        jdb.commit_batch(3, &blake2b(b"3"), Some((1, blake2b(b"1"))))
            .unwrap();
        assert!(jdb.can_reconstruct_refs());
        assert!(jdb.contains(&foo));

        // incantation to reopen the db
    };
    {
        let mut jdb = OverlayRecentDB::new(shared_db.clone(), "test");

        jdb.remove(&foo);
        jdb.commit_batch(4, &blake2b(b"4"), Some((2, blake2b(b"2"))))
            .unwrap();
        assert!(jdb.can_reconstruct_refs());
        assert!(jdb.contains(&foo));

        // incantation to reopen the db
    };
    {
        let mut jdb = OverlayRecentDB::new(shared_db.clone(), "test");

        jdb.commit_batch(5, &blake2b(b"5"), Some((3, blake2b(b"3"))))
            .unwrap();
        assert!(jdb.can_reconstruct_refs());
        assert!(jdb.contains(&foo));

        // incantation to reopen the db
    };
    {
        let mut jdb = OverlayRecentDB::new(shared_db, "test");

        jdb.commit_batch(6, &blake2b(b"6"), Some((4, blake2b(b"4"))))
            .unwrap();
        assert!(jdb.can_reconstruct_refs());
        assert!(!jdb.contains(&foo));
    }
}

#[test]
fn reopen_fork() {
    let shared_db = Arc::new(MockDbRepository::init(vec!["test".into()]));

    let (foo, bar, baz) = {
        let mut jdb = OverlayRecentDB::new(shared_db.clone(), "test");
        // history is 1
        let foo = jdb.insert(b"foo");
        let bar = jdb.insert(b"bar");
        jdb.commit_batch(0, &blake2b(b"0"), None).unwrap();
        assert!(jdb.can_reconstruct_refs());
        jdb.remove(&foo);
        let baz = jdb.insert(b"baz");
        jdb.commit_batch(1, &blake2b(b"1a"), Some((0, blake2b(b"0"))))
            .unwrap();
        assert!(jdb.can_reconstruct_refs());

        jdb.remove(&bar);
        jdb.commit_batch(1, &blake2b(b"1b"), Some((0, blake2b(b"0"))))
            .unwrap();
        assert!(jdb.can_reconstruct_refs());
        (foo, bar, baz)
    };

    {
        let mut jdb = OverlayRecentDB::new(shared_db, "test");
        jdb.commit_batch(2, &blake2b(b"2b"), Some((1, blake2b(b"1b"))))
            .unwrap();
        assert!(jdb.can_reconstruct_refs());
        assert!(jdb.contains(&foo));
        assert!(!jdb.contains(&baz));
        assert!(!jdb.contains(&bar));
    }
}

#[test]
fn insert_older_era() {
    let mut jdb = new_db();
    let foo = jdb.insert(b"foo");
    jdb.commit_batch(0, &blake2b(b"0a"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());

    let bar = jdb.insert(b"bar");
    jdb.commit_batch(1, &blake2b(b"1"), Some((0, blake2b(b"0a"))))
        .unwrap();
    assert!(jdb.can_reconstruct_refs());

    jdb.remove(&bar);
    jdb.commit_batch(0, &blake2b(b"0b"), None).unwrap();
    assert!(jdb.can_reconstruct_refs());
    jdb.commit_batch(2, &blake2b(b"2"), Some((1, blake2b(b"1"))))
        .unwrap();

    assert!(jdb.contains(&foo));
    assert!(jdb.contains(&bar));
}

#[test]
fn inject() {
    let mut jdb = new_db();
    let key = jdb.insert(b"dog");
    jdb.inject_batch().unwrap();

    assert_eq!(jdb.get(&key).unwrap(), DBValue::from_slice(b"dog"));
    jdb.remove(&key);
    jdb.inject_batch().unwrap();

    assert!(jdb.get(&key).is_none());
}

#[test]
fn earliest_era() {
    let shared_db = Arc::new(MockDbRepository::init(vec!["test".into()]));

    // empty DB
    let mut jdb = OverlayRecentDB::new(shared_db.clone(), "test");
    assert!(jdb.earliest_era().is_none());

    // single journalled era.
    let _key = jdb.insert(b"hello!");
    let mut batch = DBTransaction::new();
    jdb.journal_under(&mut batch, 0, &blake2b(b"0")).unwrap();
    jdb.backing().write_buffered(batch);

    assert_eq!(jdb.earliest_era(), Some(0));

    // second journalled era.
    let mut batch = DBTransaction::new();
    jdb.journal_under(&mut batch, 1, &blake2b(b"1")).unwrap();
    jdb.backing().write_buffered(batch);

    assert_eq!(jdb.earliest_era(), Some(0));

    // single journalled era.
    let mut batch = DBTransaction::new();
    jdb.mark_canonical(&mut batch, 0, &blake2b(b"0")).unwrap();
    jdb.backing().write_buffered(batch);

    assert_eq!(jdb.earliest_era(), Some(1));

    // no journalled eras.
    let mut batch = DBTransaction::new();
    jdb.mark_canonical(&mut batch, 1, &blake2b(b"1")).unwrap();
    jdb.backing().write_buffered(batch);

    assert_eq!(jdb.earliest_era(), Some(1));

    // reconstructed: no journal entries.
    drop(jdb);
    let jdb = OverlayRecentDB::new(shared_db, "test");
    assert_eq!(jdb.earliest_era(), None);
}
