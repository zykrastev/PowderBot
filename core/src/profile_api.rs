use crate::{
    profile::{Profile, validate_storage_name},
    profile_store::{MAX_FILE_BYTES, ProfileStore, StoreError, StoredProfile},
};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub enum Operation {
    List,
    Create,
    Update,
    Delete,
    Active,
    Select,
}

pub struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
    pub diagnostics: Vec<String>,
}
impl Reply {
    pub fn error(status: u16, message: &str) -> Self {
        Self::json(status, json!({"error":message}))
    }
    fn json(status: u16, value: Value) -> Self {
        Self {
            status,
            body: value.to_string().into_bytes(),
            diagnostics: Vec::new(),
        }
    }
}

pub fn handle(
    store: Option<&mut ProfileStore>,
    operation: Operation,
    query: &str,
    body: &[u8],
) -> Reply {
    if body.len() > MAX_FILE_BYTES {
        return Reply::error(413, "Request body exceeds 4096 bytes");
    }
    let Some(store) = store else {
        return Reply::error(503, "Profile storage unavailable");
    };
    match execute(store, operation, query, body) {
        Ok(reply) => reply,
        Err(reply) => reply,
    }
}

fn execute(
    store: &mut ProfileStore,
    operation: Operation,
    query: &str,
    body: &[u8],
) -> Result<Reply, Reply> {
    match operation {
        Operation::List => {
            let listing = store.list().map_err(storage_error)?;
            let profiles = listing
                .profiles
                .into_iter()
                .map(profile_value)
                .collect::<Result<Vec<_>, _>>()?;
            let mut reply = Reply::json(200, Value::Array(profiles));
            reply.diagnostics = listing.diagnostics;
            Ok(reply)
        }
        Operation::Active => {
            let active = store.active().map_err(storage_error)?;
            Ok(Reply::json(
                200,
                match active {
                    Some(profile) => profile_value(profile)?,
                    None => Value::Null,
                },
            ))
        }
        Operation::Create | Operation::Update => {
            let value: Value =
                serde_json::from_slice(body).map_err(|_| Reply::error(400, "Invalid JSON"))?;
            let name = storage_name(&value)?;
            let profile =
                Profile::from_json(body).map_err(|e| Reply::error(400, &e.to_string()))?;
            if matches!(operation, Operation::Create) {
                store.create(name, &profile).map_err(storage_error)?;
                Ok(Reply::json(201, json!({"success":true})))
            } else {
                store.update(name, &profile).map_err(storage_error)?;
                Ok(Reply::json(200, json!({"success":true})))
            }
        }
        Operation::Select => {
            let value: Value =
                serde_json::from_slice(body).map_err(|_| Reply::error(400, "Invalid JSON"))?;
            let name = storage_name(&value)?;
            store.select(Some(name)).map_err(storage_error)?;
            Ok(Reply::json(200, json!({"success":true})))
        }
        Operation::Delete => {
            let name = query_name(query)?;
            store.remove(&name).map_err(storage_error)?;
            Ok(Reply::json(200, json!({"success":true})))
        }
    }
}
fn storage_name(value: &Value) -> Result<&str, Reply> {
    let name = value
        .get("storageName")
        .and_then(Value::as_str)
        .ok_or_else(|| Reply::error(400, "storageName is required"))?;
    validate_storage_name(name).map_err(|e| Reply::error(400, &e.to_string()))?;
    Ok(name)
}
fn profile_value(stored: StoredProfile) -> Result<Value, Reply> {
    let mut value = serde_json::to_value(stored.profile)
        .map_err(|_| Reply::error(500, "Could not serialize profile"))?;
    value["storageName"] = stored.storage_name.into();
    Ok(value)
}
fn storage_error(error: StoreError) -> Reply {
    let status = match &error {
        StoreError::NotFound => 404,
        StoreError::AlreadyExists => 409,
        StoreError::Capacity => 409,
        _ => 500,
    };
    Reply::error(status, &error.to_string())
}
fn query_name(query: &str) -> Result<String, Reply> {
    let bad = || Reply::error(400, "Exactly one valid name query parameter is required");
    let mut value = None;
    for part in query.split('&') {
        let Some((key, encoded)) = part.split_once('=') else {
            return Err(bad());
        };
        if key != "name" || value.is_some() {
            return Err(bad());
        }
        let mut decoded = Vec::new();
        let mut bytes = encoded.bytes();
        while let Some(b) = bytes.next() {
            decoded.push(if b == b'%' {
                let hi = (bytes.next().ok_or_else(bad)? as char)
                    .to_digit(16)
                    .ok_or_else(bad)?;
                let lo = (bytes.next().ok_or_else(bad)? as char)
                    .to_digit(16)
                    .ok_or_else(bad)?;
                (hi * 16 + lo) as u8
            } else if b == b'+' {
                b' '
            } else {
                b
            });
        }
        let name = String::from_utf8(decoded).map_err(|_| bad())?;
        validate_storage_name(&name).map_err(|_| bad())?;
        value = Some(name);
    }
    value.ok_or_else(bad)
}
