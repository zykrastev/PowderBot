//! A read-only log page on the same access-point network as the C++ firmware.

use embedded_svc::{http::Method, io::Write};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::modem::Modem,
    http::server::{Configuration as ServerConfiguration, EspHttpServer},
    nvs::EspDefaultNvsPartition,
    wifi::{AccessPointConfiguration, AuthMethod, BlockingWifi, Configuration, EspWifi},
};

use crate::web_log::SharedLogs;

const SSID: &str = "PowderBot";
const PASSWORD: &str = "powderbot";
const PAGE: &str = include_str!("../web/index.html");

// Retaining these objects keeps the server and Wi-Fi alive.
pub struct WebConsole {
    _server: EspHttpServer<'static>,
    _wifi: BlockingWifi<EspWifi<'static>>,
    pub address: String,
}

impl WebConsole {
    pub fn new(modem: Modem<'static>, logs: SharedLogs) -> anyhow::Result<Self> {
        let event_loop = EspSystemEventLoop::take()?;
        let nvs = EspDefaultNvsPartition::take()?;
        let mut wifi = BlockingWifi::wrap(
            EspWifi::new(modem, event_loop.clone(), Some(nvs))?,
            event_loop,
        )?;
        wifi.set_configuration(&Configuration::AccessPoint(AccessPointConfiguration {
            ssid: SSID
                .try_into()
                .map_err(|_| anyhow::anyhow!("SSID is too long"))?,
            password: PASSWORD
                .try_into()
                .map_err(|_| anyhow::anyhow!("Password is too long"))?,
            auth_method: AuthMethod::WPA2Personal,
            channel: 1,
            ..Default::default()
        }))?;
        wifi.start()?;
        wifi.wait_netif_up()?;
        let address = wifi.wifi().ap_netif().get_ip_info()?.ip.to_string();

        let mut server = EspHttpServer::new(&ServerConfiguration {
            stack_size: 8192,
            ..Default::default()
        })?;
        server.fn_handler::<anyhow::Error, _>("/", Method::Get, |request| {
            request
                .into_response(
                    200,
                    Some("OK"),
                    &[
                        ("Content-Type", "text/html; charset=utf-8"),
                        ("Cache-Control", "no-store"),
                    ],
                )?
                .write_all(PAGE.as_bytes())?;
            Ok(())
        })?;
        server.fn_handler::<anyhow::Error, _>("/logs", Method::Get, move |request| {
            let snapshot = {
                let history = logs.lock().unwrap_or_else(|error| error.into_inner());
                history.snapshot()
            };
            // Send after releasing the lock, so a slow client doesn't block logs.
            request
                .into_response(
                    200,
                    Some("OK"),
                    &[
                        ("Content-Type", "text/plain; charset=utf-8"),
                        ("Cache-Control", "no-store"),
                    ],
                )?
                .write_all(snapshot.as_bytes())?;
            Ok(())
        })?;
        log::info!("Web logs ready: connect to Wi-Fi {SSID}, open http://{address}/");
        Ok(Self {
            _server: server,
            _wifi: wifi,
            address,
        })
    }
}
