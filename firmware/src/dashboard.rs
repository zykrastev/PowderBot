use crate::{
    api::SharedProfiles,
    control::{Command, Control},
};
use embedded_svc::{http::Method, io::Write};
use esp_idf_svc::http::server::EspHttpServer;
use powderbot_core::profile_api::Reply;
use std::time::Instant;
fn result_reply(result: crate::control::Result<()>) -> Reply {
    match result {
        Ok(()) => Reply {
            status: 200,
            body: br#"{"success":true}"#.to_vec(),
            diagnostics: Vec::new(),
        },
        Err((code, error)) => Reply::error(code, &error),
    }
}
pub fn register(
    server: &mut EspHttpServer<'static>,
    control: Control,
    profiles: Option<SharedProfiles>,
) -> anyhow::Result<()> {
    let history = control.clone();
    server.fn_handler::<anyhow::Error, _>("/api/loads", Method::Get, move |request| {
        let (code, body) = match history.snapshot() {
            Ok(state) => match state.load_history(Instant::now()) {
                Ok(body) => (200, body),
                Err(error) => (500, Reply::error(500, &error.to_string()).body),
            },
            Err((code, error)) => (code, Reply::error(code, &error).body),
        };
        request
            .into_response(
                code,
                None,
                &[
                    ("Content-Type", "application/json"),
                    ("Cache-Control", "no-store"),
                ],
            )?
            .write_all(&body)?;
        Ok(())
    })?;
    let clear = control.clone();
    server.fn_handler::<anyhow::Error, _>("/api/loads", Method::Delete, move |request| {
        let reply = match clear.gate.lock() {
            Ok(_guard) => result_reply(clear.submit(Command::ClearLoads)),
            Err(_) => Reply::error(503, "Controller gate unavailable"),
        };
        request
            .into_response(
                reply.status,
                None,
                &[
                    ("Content-Type", "application/json"),
                    ("Cache-Control", "no-store"),
                ],
            )?
            .write_all(&reply.body)?;
        Ok(())
    })?;
    let snapshot = control.clone();
    server.fn_handler::<anyhow::Error, _>("/api/status", Method::Get, move |request| {
        let (code, body) = match snapshot.snapshot() {
            Ok(state) => (200, state.snapshot(Instant::now()).to_string().into_bytes()),
            Err((code, error)) => (code, Reply::error(code, &error).body),
        };
        request
            .into_response(
                code,
                None,
                &[
                    ("Content-Type", "application/json"),
                    ("Cache-Control", "no-store"),
                ],
            )?
            .write_all(&body)?;
        Ok(())
    })?;
    let settings = control.clone();
    server.fn_handler::<anyhow::Error, _>("/api/settings", Method::Put, move |mut request| {
        let reply = match crate::api::read_body(&mut request, true) {
            Err(reply) => reply,
            Ok(bytes) => match settings.gate.lock() {
                Ok(_guard) => result_reply(settings.submit(Command::Settings(bytes))),
                Err(_) => Reply::error(503, "Controller gate unavailable"),
            },
        };
        request
            .into_response(
                reply.status,
                None,
                &[
                    ("Content-Type", "application/json"),
                    ("Connection", "close"),
                    ("Cache-Control", "no-store"),
                ],
            )?
            .write_all(&reply.body)?;
        Ok(())
    })?;
    for path in ["/api/start", "/api/stop", "/api/tare", "/api/reset"] {
        let control = control.clone();
        let profiles = profiles.clone();
        server.fn_handler::<anyhow::Error, _>(path, Method::Post, move |request| {
            let result = if path == "/api/stop" {
                control.submit(Command::Stop)
            } else {
                match control.gate.lock() {
                    Err(_) => Err((503, "Controller gate unavailable".into())),
                    Ok(_guard) => match path {
                        "/api/start" => {
                            if control.busy() {
                                Err((409, "Already dispensing".into()))
                            } else {
                                match &profiles {
                                    None => Err((503, "Profile storage unavailable".into())),
                                    Some(profiles) => match profiles.lock() {
                                        Err(_) => {
                                            Err((503, "Profile storage lock unavailable".into()))
                                        }
                                        Ok(mut profiles) => match profiles.active() {
                                            Ok(profile) => control.submit(Command::Start(profile)),
                                            Err(error) => Err((500, error.to_string())),
                                        },
                                    },
                                }
                            }
                        }
                        "/api/tare" => control.submit(Command::Tare),
                        _ => control.submit(Command::Reset),
                    },
                }
            };
            let reply = result_reply(result);
            request
                .into_response(
                    reply.status,
                    None,
                    &[
                        ("Content-Type", "application/json"),
                        ("Connection", "close"),
                        ("Cache-Control", "no-store"),
                    ],
                )?
                .write_all(&reply.body)?;
            Ok(())
        })?;
    }
    Ok(())
}

pub fn stop_server(control: Control) -> anyhow::Result<EspHttpServer<'static>> {
    let mut server = EspHttpServer::new(&esp_idf_svc::http::server::Configuration {
        http_port: 81,
        ctrl_port: 32769,
        stack_size: 8192,
        max_open_sockets: 2,
        max_uri_handlers: 2,
        ..Default::default()
    })?;
    server.fn_handler::<anyhow::Error, _>("/api/stop", Method::Post, move |request| {
        let (status, body): (u16, &[u8]) = match control.submit(Command::Stop) {
            Ok(()) => (200, br#"{"success":true}"#),
            Err((status, _)) => (
                status,
                br#"{"error":"Stop not acknowledged; check controller status"}"#,
            ),
        };
        request
            .into_response(
                status,
                None,
                &[
                    ("Content-Type", "application/json"),
                    ("Access-Control-Allow-Origin", "*"),
                    ("Connection", "close"),
                    ("Cache-Control", "no-store"),
                ],
            )?
            .write_all(body)?;
        Ok(())
    })?;
    server.fn_handler::<anyhow::Error, _>("/api/stop", Method::Options, |request| {
        request.into_response(
            204,
            None,
            &[
                ("Access-Control-Allow-Origin", "*"),
                ("Access-Control-Allow-Methods", "POST, OPTIONS"),
                ("Access-Control-Allow-Headers", "Content-Type"),
            ],
        )?;
        Ok(())
    })?;
    Ok(server)
}
