
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
include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));

pub struct WebConsole {
    _stop_server: Option<EspHttpServer<'static>>,
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
            stack_size: 32768,
            max_uri_handlers: 64,
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
        for &(path, mime, bytes) in ASSETS {
            server.fn_handler::<anyhow::Error, _>(path, Method::Get, move |request| {
                request
                    .into_response(
                        200,
                        None,
                        &[("Content-Type", mime), ("Cache-Control", "no-cache")],
                    )?
                    .write_all(bytes)?;
                Ok(())
            })?;
        }
        log::info!("Dashboard ready: Wi-Fi {SSID}, http://{address}/; logs at /console");
        Ok(Self {
            _stop_server: None,
            _server: server,
            _wifi: wifi,
            address,
        })
    }

    pub fn register_profiles(
        &mut self,
        profiles: Option<crate::api::SharedProfiles>,
        control: crate::control::Control,
    ) -> anyhow::Result<()> {
        crate::api::register(&mut self._server, profiles, control)
    }
    pub fn register_dashboard(
        &mut self,
        control: crate::control::Control,
        profiles: Option<crate::api::SharedProfiles>,
    ) -> anyhow::Result<()> {
        self._stop_server = Some(crate::dashboard::stop_server(control.clone())?);
        crate::dashboard::register(&mut self._server, control, profiles)
    }
    pub fn has_client(&self) -> Result<bool, esp_idf_svc::sys::EspError> {
        let mut stations = esp_idf_svc::sys::wifi_sta_list_t::default();
        // SAFETY: Wi-Fi is initialized and stations is a valid output buffer.
        esp_idf_svc::sys::esp!(unsafe {
            esp_idf_svc::sys::esp_wifi_ap_get_sta_list(&mut stations)
        })?;
        Ok(stations.num > 0)
    }
}
