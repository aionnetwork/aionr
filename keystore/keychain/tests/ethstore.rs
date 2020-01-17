extern crate tempdir;
extern crate key;
extern crate keychain;

use keychain::accounts_dir::{KeyDirectory, MemoryDirectory, RootDiskDirectory};
use key::{generate_keypair, Ed25519KeyPair};
use keychain::secret_store::{SimpleSecretStore, SecretStore};
use keychain::EthStore;
use tempdir::TempDir;

fn keypair() -> Ed25519KeyPair { generate_keypair() }

fn store() -> EthStore {
    EthStore::open(Box::new(MemoryDirectory::default()))
        .expect("MemoryDirectory always load successfuly; qed")
}

struct RootDiskDirectoryGuard {
    pub key_dir: Option<Box<dyn KeyDirectory>>,
    _path: TempDir,
}

impl RootDiskDirectoryGuard {
    pub fn new() -> Self {
        let temp_path = TempDir::new("").unwrap();
        let disk_dir = Box::new(RootDiskDirectory::create(temp_path.path()).unwrap());

        RootDiskDirectoryGuard {
            key_dir: Some(disk_dir),
            _path: temp_path,
        }
    }
}

#[test]
fn should_insert_account_successfully() {
    // given
    let store = store();
    let keypair = keypair();

    // when
    let address = store
        .insert_account_ed25519(keypair.secret().clone(), "test")
        .unwrap();

    // then
    //assert_eq!(address, StoreAccountRef::root(keypair.address()));
    assert!(store.get(&address).is_ok(), "Should contain account.");
    assert_eq!(
        store.accounts().unwrap().len(),
        1,
        "Should have one account."
    );
}

#[test]
fn should_update_meta_and_name() {
    // given
    let store = store();
    let keypair = keypair();
    let address = store
        .insert_account_ed25519(keypair.secret().clone(), "test")
        .unwrap();
    assert_eq!(&store.meta(&address).unwrap(), "{}");
    assert_eq!(&store.name(&address).unwrap(), "");

    // when
    store.set_meta(&address, "meta".into()).unwrap();
    store.set_name(&address, "name".into()).unwrap();

    // then
    assert_eq!(&store.meta(&address).unwrap(), "meta");
    assert_eq!(&store.name(&address).unwrap(), "name");
    assert_eq!(store.accounts().unwrap().len(), 1);
}

#[test]
fn should_remove_account() {
    // given
    let store = store();
    let keypair = keypair();
    let address = store
        .insert_account_ed25519(keypair.secret().clone(), "test")
        .unwrap();

    // when
    store.remove_account(&address, "test").unwrap();

    // then
    assert_eq!(store.accounts().unwrap().len(), 0, "Should remove account.");
}

#[test]
fn should_return_true_if_password_is_correct() {
    // given
    let store = store();
    let keypair = keypair();
    let address = store
        .insert_account_ed25519(keypair.secret().clone(), "test")
        .unwrap();

    // when
    let res1 = store.test_password(&address, "x").unwrap();
    let res2 = store.test_password(&address, "test").unwrap();

    assert!(!res1, "First password should be invalid.");
    assert!(res2, "Second password should be correct.");
}

#[test]
fn should_not_remove_account_when_moving_to_self() {
    // given
    let mut dir = RootDiskDirectoryGuard::new();
    let store = EthStore::open(dir.key_dir.take().unwrap()).unwrap();
    let password1 = "password1";
    let keypair1 = keypair();

    // when
    let account1 = store
        .insert_account_ed25519(keypair1.secret().clone(), password1)
        .unwrap();
    store.change_account_vault(account1).unwrap();

    // then
    let accounts = store.accounts().unwrap();
    assert_eq!(accounts.len(), 1);
}

#[test]
fn should_list_opened_vaults() {
    // given
    let mut dir = RootDiskDirectoryGuard::new();
    let store = EthStore::open(dir.key_dir.take().unwrap()).unwrap();
    let name1 = "vault1";
    let password1 = "password1";
    let name2 = "vault2";
    let password2 = "password2";
    let name3 = "vault3";
    let password3 = "password3";

    // when
    store.create_vault(name1, password1).unwrap();
    store.create_vault(name2, password2).unwrap();
    store.create_vault(name3, password3).unwrap();
    store.close_vault(name2).unwrap();

    // then
    let opened_vaults = store.list_opened_vaults().unwrap();
    assert_eq!(opened_vaults.len(), 2);
    assert!(opened_vaults.iter().any(|v| &*v == name1));
    assert!(opened_vaults.iter().any(|v| &*v == name3));
}

#[test]
fn should_manage_vaults_meta() {
    // given
    let mut dir = RootDiskDirectoryGuard::new();
    let store = EthStore::open(dir.key_dir.take().unwrap()).unwrap();
    let name1 = "vault1";
    let password1 = "password1";

    // when
    store.create_vault(name1, password1).unwrap();

    // then
    assert_eq!(store.get_vault_meta(name1).unwrap(), "{}".to_owned());
    assert!(store.set_vault_meta(name1, "Hello, world!!!").is_ok());
    assert_eq!(
        store.get_vault_meta(name1).unwrap(),
        "Hello, world!!!".to_owned()
    );

    // and when
    store.close_vault(name1).unwrap();
    store.open_vault(name1, password1).unwrap();

    // then
    assert_eq!(
        store.get_vault_meta(name1).unwrap(),
        "Hello, world!!!".to_owned()
    );

    // and when
    store.close_vault(name1).unwrap();

    // then
    assert_eq!(
        store.get_vault_meta(name1).unwrap(),
        "Hello, world!!!".to_owned()
    );
    assert!(store.get_vault_meta("vault2").is_err());
}

#[test]
fn should_save_meta_when_setting_before_password() {
    // given
    let mut dir = RootDiskDirectoryGuard::new();
    let store = EthStore::open(dir.key_dir.take().unwrap()).unwrap();
    let name = "vault";
    let password = "password1";
    let new_password = "password2";

    // when
    store.create_vault(name, password).unwrap();
    store.set_vault_meta(name, "OldMeta").unwrap();
    store.change_vault_password(name, new_password).unwrap();

    // then
    assert_eq!(store.get_vault_meta(name).unwrap(), "OldMeta".to_owned());
}

#[test]
fn should_export_account() {
    // given
    let store = store();
    let keypair = generate_keypair();
    let address = store
        .insert_account_ed25519(keypair.secret().clone(), "test")
        .unwrap();

    // when
    let exported = store.export_account(&address, "test");

    // then
    assert!(
        exported.is_ok(),
        "Should export single account: {:?}",
        exported
    );
}
