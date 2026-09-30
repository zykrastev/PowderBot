mod support;
use powderbot_core::{
    profile_api::{Operation, handle},
    profile_store::ProfileStore,
};

#[test]
fn profile_http_contract() {
    let root = std::env::temp_dir().join(format!("powderbot-api-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let mut store = ProfileStore::open(&root).unwrap();
    let mut body = serde_json::to_value(support::profile()).unwrap();
    body["storageName"] = "api_test".into();
    let bytes = serde_json::to_vec(&body).unwrap();
    assert_eq!(handle(None, Operation::List, "", b"").status, 503);
    assert_eq!(
        handle(Some(&mut store), Operation::Create, "", b"bad").status,
        400
    );
    assert_eq!(
        handle(Some(&mut store), Operation::Create, "", &vec![b' '; 4097]).status,
        413
    );
    assert_eq!(
        handle(Some(&mut store), Operation::Create, "", &bytes).status,
        201
    );
    assert_eq!(
        handle(Some(&mut store), Operation::Create, "", &bytes).status,
        409
    );
    let response = handle(Some(&mut store), Operation::List, "", b"");
    assert_eq!(response.status, 200);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&response.body).unwrap()[0]["storageName"],
        "api_test"
    );
    assert_eq!(
        handle(
            Some(&mut store),
            Operation::Select,
            "",
            br#"{"storageName":"api_test"}"#
        )
        .status,
        200
    );
    assert_eq!(
        handle(Some(&mut store), Operation::Active, "", b"").status,
        200
    );
    assert_eq!(
        handle(Some(&mut store), Operation::Update, "", &bytes).status,
        200
    );
    assert_eq!(
        handle(Some(&mut store), Operation::Delete, "name=..%2Fx", b"").status,
        400
    );
    assert_eq!(
        handle(
            Some(&mut store),
            Operation::Delete,
            "name=api_test&name=other",
            b""
        )
        .status,
        400
    );
    assert_eq!(
        handle(Some(&mut store), Operation::Delete, "name=api%5Ftest", b"").status,
        200
    );
    assert_eq!(
        handle(Some(&mut store), Operation::Delete, "name=api_test", b"").status,
        404
    );
    let active = handle(Some(&mut store), Operation::Active, "", b"");
    assert_eq!(active.status, 200);
    assert_eq!(active.body, b"null");
    assert_eq!(
        handle(Some(&mut store), Operation::Update, "", &bytes).status,
        404
    );
    std::fs::write(root.join("broken.json"), b"invalid").unwrap();
    let listing = handle(Some(&mut store), Operation::List, "", b"");
    assert_eq!(listing.status, 200);
    assert_eq!(listing.diagnostics.len(), 1);
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}
