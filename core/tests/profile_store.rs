mod support;
use powderbot_core::profile_store::{ProfileStore, StoreError};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "powderbot-store-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> ProfileStore {
        ProfileStore::open(&self.0).unwrap()
    }
    fn put(&self, name: &str, bytes: &[u8]) {
        fs::write(self.0.join(name), bytes).unwrap();
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn crud_and_selection_survive_reopen() {
    let dir = Directory::new();
    let mut store = dir.store();
    let mut p = support::profile();
    store.create("one", &p).unwrap();
    assert!(matches!(
        store.create("one", &p),
        Err(StoreError::AlreadyExists)
    ));
    assert!(matches!(
        store.update("missing", &p),
        Err(StoreError::NotFound)
    ));
    assert!(matches!(store.remove("missing"), Err(StoreError::NotFound)));
    store.select(Some("one")).unwrap();
    drop(store);
    let mut store = dir.store();
    assert_eq!(store.active().unwrap().unwrap().storage_name, "one");
    p.name = "Updated".into();
    store.update("one", &p).unwrap();
    assert_eq!(store.read("one").unwrap(), p);
    assert_eq!(store.list().unwrap().profiles.len(), 1);
    store.remove("one").unwrap();
    assert!(store.active().unwrap().is_none());
    assert!(store.list().unwrap().profiles.is_empty());
}

#[test]
fn rejects_paths_reserved_names_and_bad_selection() {
    let dir = Directory::new();
    let mut store = dir.store();
    for name in ["../escape", "", "a/b", "active_profile", "storage_probe"] {
        assert!(store.create(name, &support::profile()).is_err());
        assert!(store.read(name).is_err());
        assert!(store.remove(name).is_err());
    }
    assert!(store.select(Some("missing")).is_err());
    dir.put("active_profile.json", br#"{"storageName":"missing"}"#);
    assert!(store.active().is_err()); // caller reports diagnostic and uses no selection
    dir.put("active_profile.json", b"broken");
    assert!(store.active().is_err());
}

#[test]
fn recovery_prefers_primary_then_valid_backup_never_temporary() {
    for primary in [None, Some(b"broken".as_slice())] {
        let dir = Directory::new();
        if let Some(bytes) = primary {
            dir.put("one.json", bytes);
        }
        dir.put("one.bak", &support::profile().to_json().unwrap());
        dir.put("one.tmp", b"unfinished");
        let mut store = dir.store();
        assert_eq!(store.list().unwrap().profiles.len(), 1);
        assert_eq!(store.read("one").unwrap(), support::profile());
    }
    let dir = Directory::new();
    let mut newer = support::profile();
    newer.name = "New".into();
    dir.put("one.json", &newer.to_json().unwrap());
    dir.put("one.bak", &support::profile().to_json().unwrap());
    let mut store = dir.store();
    assert_eq!(store.read("one").unwrap(), newer);
    store.remove("one").unwrap();
    assert!(matches!(store.read("one"), Err(StoreError::NotFound)));
    dir.put("two.tmp", &newer.to_json().unwrap());
    assert!(matches!(store.read("two"), Err(StoreError::NotFound)));
    assert!(store.list().unwrap().profiles.is_empty());
}

#[test]
fn isolates_corruption_and_bounds_resources() {
    let dir = Directory::new();
    let mut store = dir.store();
    store.create("good", &support::profile()).unwrap();
    dir.put("bad.json", b"broken");
    dir.put("bad.bak", b"also broken");
    dir.put("big.json", &vec![b' '; 4097]);
    let list = store.list().unwrap();
    assert_eq!(list.profiles.len(), 1);
    assert_eq!(list.diagnostics.len(), 2);
    assert_eq!(fs::read(dir.0.join("bad.json")).unwrap(), b"broken");
    assert_eq!(fs::read(dir.0.join("bad.bak")).unwrap(), b"also broken");
    for i in 3..64 {
        dir.put(&format!("p{i}.json"), b"broken");
    }
    assert!(matches!(
        store.create("overflow", &support::profile()),
        Err(StoreError::Capacity)
    ));
    store.update("good", &support::profile()).unwrap();
    store.remove("bad").unwrap();
    store.create("replacement", &support::profile()).unwrap();
}

#[test]
fn failed_temporary_write_preserves_primary() {
    let dir = Directory::new();
    let mut store = dir.store();
    store.create("one", &support::profile()).unwrap();
    fs::create_dir(dir.0.join("one.tmp")).unwrap();
    assert!(store.update("one", &support::profile()).is_err());
    assert_eq!(store.read("one").unwrap(), support::profile());
}

#[test]
fn accepts_exact_file_limit_and_recovers_active_metadata() {
    let dir = Directory::new();
    let mut bytes = support::profile().to_json().unwrap();
    bytes.resize(4096, b' ');
    dir.put("one.json", &bytes);
    dir.put("active_profile.bak", br#"{"storageName":"one"}"#);
    let mut store = dir.store();
    assert_eq!(store.read("one").unwrap(), support::profile());
    assert_eq!(store.active().unwrap().unwrap().storage_name, "one");
    dir.put("one.json", b"broken");
    assert!(store.active().is_err());
}

#[test]
fn over_capacity_store_can_be_listed_and_reduced() {
    let dir = Directory::new();
    for i in 0..65 {
        dir.put(
            &format!("p{i}.json"),
            &support::profile().to_json().unwrap(),
        );
    }
    let mut store = dir.store();
    let listing = store.list().unwrap();
    assert_eq!(listing.profiles.len(), 64);
    assert!(
        listing
            .diagnostics
            .iter()
            .any(|message| message.contains("truncated"))
    );
    store.remove("p64").unwrap();
    store.remove("p63").unwrap();
    store.create("new", &support::profile()).unwrap();
}
