use super::{AccountProvider, Unlock};
use std::time::Instant;
use keychain::ethkey::{generate_keypair, Address};
use keychain::StoreAccountRef;

#[test]
fn unlock_account_temp() {
    let kp = generate_keypair();
    let ap = AccountProvider::transient_provider();
    assert!(
        ap.insert_account_ed25519(kp.secret().clone(), "test")
            .is_ok()
    );
    assert!(
        ap.unlock_account_temporarily(&kp.address(), "test1".into())
            .is_err()
    );
    assert!(
        ap.unlock_account_temporarily(&kp.address(), "test".into())
            .is_ok()
    );
    assert!(ap.sign(kp.address(), None, Default::default()).is_ok());
    assert!(ap.sign(kp.address(), None, Default::default()).is_err());
}

#[test]
fn unlock_account_perm() {
    let kp = generate_keypair();
    let ap = AccountProvider::transient_provider();
    assert!(
        ap.insert_account_ed25519(kp.secret().clone(), "test")
            .is_ok()
    );
    assert!(
        ap.unlock_account_permanently(&kp.address(), "test1".into())
            .is_err()
    );
    assert!(
        ap.unlock_account_permanently(&kp.address(), "test".into())
            .is_ok()
    );
    assert!(ap.sign(kp.address(), None, Default::default()).is_ok());
    assert!(ap.sign(kp.address(), None, Default::default()).is_ok());
    assert!(
        ap.unlock_account_temporarily(&kp.address(), "test".into())
            .is_ok()
    );
    assert!(ap.sign(kp.address(), None, Default::default()).is_ok());
    assert!(ap.sign(kp.address(), None, Default::default()).is_ok());
}

#[test]
fn unlock_account_timer() {
    let kp = generate_keypair();
    let ap = AccountProvider::transient_provider();
    assert!(
        ap.insert_account_ed25519(kp.secret().clone(), "test")
            .is_ok()
    );
    assert!(
        ap.unlock_account_timed(&kp.address(), "test1".into(), 60000)
            .is_err()
    );
    assert!(
        ap.unlock_account_timed(&kp.address(), "test".into(), 60000)
            .is_ok()
    );
    assert!(ap.sign(kp.address(), None, Default::default()).is_ok());
    ap.unlocked
        .write()
        .get_mut(&StoreAccountRef::new(kp.address()))
        .unwrap()
        .unlock = Unlock::Timed(Instant::now());
    assert!(ap.sign(kp.address(), None, Default::default()).is_err());
}

#[test]
fn should_not_return_blacklisted_account() {
    // given
    let mut ap = AccountProvider::transient_provider();
    let acc = ap.new_account_ed25519("test").unwrap();
    ap.blacklisted_accounts = vec![acc];

    // then
    assert_eq!(
        ap.accounts_info()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<Address>>(),
        vec![]
    );
    assert_eq!(ap.accounts().unwrap(), vec![]);
}
