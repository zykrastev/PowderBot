use embedded_svc::{http::Method, io::Write};
use esp_idf_svc::http::server::EspHttpServer;
use powderbot_core::{dashboard::Dashboard, profile_api::Reply};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

pub type SharedDashboard = Arc<Mutex<Dashboard>>;
pub fn register(server: &mut EspHttpServer<'static>, state: SharedDashboard) -> anyhow::Result<()> {
    let snapshot = state.clone();
    server.fn_handler::<anyhow::Error, _>("/api/status", Method::Get, move |request| {
        let (status, body) = match snapshot.lock() {
            Ok(state) => (200, state.snapshot(Instant::now()).to_string().into_bytes()),
            Err(_) => (503, br#"{"error":"Status unavailable"}"#.to_vec()),
        };
        request
            .into_response(
                status,
                None,
                &[
                    ("Content-Type", "application/json"),
                    ("Cache-Control", "no-store"),
                ],
            )?
            .write_all(&body)?;
        Ok(())
    })?;
    server.fn_handler::<anyhow::Error, _>("/api/settings", Method::Put, move |mut request| {
        let reply = match crate::api::read_body(&mut request, true) {
            Err(reply) => reply,
            Ok(body) => match state.lock() {
                Ok(mut state) => match state.update_settings(&body) {
                    Ok(()) => Reply {
                        status: 200,
                        body: br#"{"success":true}"#.to_vec(),
                        diagnostics: Vec::new(),
                    },
                    Err(error) => Reply::error(400, &error),
                },
                Err(_) => Reply::error(503, "Settings unavailable"),
            },
        };
        request
            .into_response(
                reply.status,
                None,
                &[
                    ("Content-Type", "application/json"),
                    ("Cache-Control", "no-store"),
                    ("Connection", "close"),
                ],
            )?
            .write_all(&reply.body)?;
        Ok(())
    })?;
    for path in ["/api/start", "/api/stop", "/api/tare", "/api/reset"] {
        server.fn_handler::<anyhow::Error, _>(path, Method::Post, |request| {
            request.into_response(501, None, &[("Content-Type", "application/json"), ("Connection", "close")])?
                .write_all(br#"{"error":"Hardware commands are not available in this migration checkpoint"}"#)?;
            Ok(())
        })?;
    }
    Ok(())
}
