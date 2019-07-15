use super::*;
use super::archivedb::ArchiveDB;
use super::overlayrecentdb::OverlayRecentDB;

extern crate aion_types;

mod archievedb;
mod overlayrecentdb;

#[test]
fn test_journal_algorithm_parsing() {
    assert_eq!(Algorithm::Archive, "archive".parse().unwrap());
    assert_eq!(Algorithm::OverlayRecent, "fast".parse().unwrap());
}

#[test]
fn test_journal_algorithm_printing() {
    assert_eq!(Algorithm::Archive.to_string(), "archive".to_owned());
    assert_eq!(Algorithm::OverlayRecent.to_string(), "fast".to_owned());
}

#[test]
fn test_journal_algorithm_is_stable() {
    assert!(Algorithm::Archive.is_stable());
    assert!(Algorithm::OverlayRecent.is_stable());
}

#[test]
fn test_journal_algorithm_default() {
    assert_eq!(Algorithm::default(), Algorithm::OverlayRecent);
}

#[test]
fn test_journal_algorithm_all_types() {
    // compiling should fail if some cases are not covered
    let mut archive = 0;
    let mut overlayrecent = 0;

    for a in &Algorithm::all_types() {
        match *a {
            Algorithm::Archive => archive += 1,
            Algorithm::OverlayRecent => overlayrecent += 1,
        }
    }

    assert_eq!(archive, 1);
    assert_eq!(overlayrecent, 1);
}
