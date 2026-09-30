//! Thin HTTP adapter. Pure endpoint behavior lives in powderbot-core.
use embedded_svc::{http::Method, io::Write};
use esp_idf_svc::http::server::EspHttpServer;
use powderbot_core::{
    profile_api::{self, Operation, Reply},
    profile_store::{ProfileStore, MAX_FILE_BYTES},
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub type SharedProfiles = Arc<Mutex<ProfileStore>>;

pub fn register(
    server: &mut EspHttpServer<'static>,
    profiles: Option<SharedProfiles>,
) -> anyhow::Result<()> {
    server.fn_handler::<anyhow::Error, _>("/api/system", Method::Get, |request| {
        request.into_response(200, None, &[("Content-Type", "application/json"), ("Cache-Control", "no-store")])?
            .write_all(br#"{"name":"PowderBot","version":"0.1.0","author":"Zhivko Krastev","capabilities":{"profiles":true,"dispensing":false}}"#)?;
        Ok(())
    })?;
    for (path, method, operation) in [
        ("/api/powders", Method::Get, Operation::List),
        ("/api/powders", Method::Post, Operation::Create),
        ("/api/powders", Method::Put, Operation::Update),
        ("/api/powders", Method::Delete, Operation::Delete),
        ("/api/active-powder", Method::Get, Operation::Active),
        ("/api/active-powder", Method::Put, Operation::Select),
    ] {
        let profiles = profiles.clone();
        server.fn_handler::<anyhow::Error, _>(path, method, move |mut request| {
            let query = request
                .uri()
                .split_once('?')
                .map(|(_, query)| query)
                .unwrap_or("")
                .to_owned();
            let needs_body = matches!(method, Method::Post | Method::Put);
            let (body, rejection) = match read_body(&mut request, needs_body) {
                Ok(body) => (body, None),
                Err(reply) => (Vec::new(), Some(reply)),
            };
            let reply = rejection.unwrap_or_else(|| match &profiles {
                Some(profiles) => match profiles.lock() {
                    Ok(mut store) => {
                        profile_api::handle(Some(&mut store), operation, &query, &body)
                    }
                    Err(_) => Reply::error(503, "Profile storage lock unavailable"),
                },
                None => profile_api::handle(None, operation, &query, &body),
            });
            // File access is finished and the lock released before sending to a slow client.
            for diagnostic in &reply.diagnostics {
                log::warn!("Profile API: {diagnostic}");
            }
            let warnings = reply.diagnostics.len().to_string();
            request
                .into_response(
                    reply.status,
                    None,
                    &[
                        ("Content-Type", "application/json"),
                        ("Cache-Control", "no-store"),
                        ("Connection", "close"),
                        ("X-Profile-Warnings", &warnings),
                    ],
                )?
                .write_all(&reply.body)?;
            Ok(())
        })?;
    }
    log::info!("Profile API ready");
    Ok(())
}

pub(crate) fn read_body(
    request: &mut embedded_svc::http::server::Request<
        &mut esp_idf_svc::http::server::EspHttpConnection<'_>,
    >,
    needs_body: bool,
) -> Result<Vec<u8>, Reply> {
    let mut body = Vec::new();
    let mut rejection = None;
    if request.header("Transfer-Encoding").is_some() {
        rejection = Some(Reply::error(
            400,
            "Use Content-Length; chunked requests are unsupported",
        ));
    } else if needs_body {
        let content_type = request
            .header("Content-Type")
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim();
        let length = request
            .header("Content-Length")
            .and_then(|value| value.parse::<usize>().ok());
        if !content_type.eq_ignore_ascii_case("application/json") {
            rejection = Some(Reply::error(415, "Content-Type must be application/json"));
        } else if let Some(length) = length {
            if length > MAX_FILE_BYTES {
                rejection = Some(Reply::error(413, "Request body exceeds 4096 bytes"));
            } else {
                body.resize(length, 0);
                let start = Instant::now();
                let mut offset = 0;
                while offset < length {
                    if start.elapsed() >= Duration::from_secs(5) {
                        rejection = Some(Reply::error(408, "Request body timed out"));
                        break;
                    }
                    match request.read(&mut body[offset..]) {
                        Ok(0) => {
                            rejection = Some(Reply::error(400, "Incomplete request body"));
                            break;
                        }
                        Ok(count) => offset += count,
                        Err(_) => {
                            rejection = Some(Reply::error(408, "Request body could not be read"));
                            break;
                        }
                    }
                }
            }
        } else {
            rejection = Some(Reply::error(411, "Content-Length is required"));
        }
    }
    match rejection {
        Some(reply) => Err(reply),
        None => Ok(body),
    }
}
