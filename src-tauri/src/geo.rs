//! EN: Country by the public IP, for the "through NetPilot" autostart mode: outside Russia zapret is not
//!     needed. Two public services, the second one if the first does not answer. Requests go without a proxy:
//!     a system proxy would show the proxy country. The country is checked again when the network changes —
//!     it is noticed by the local address the system would use for the internet (nothing is sent for that).
//! RU: Страна по внешнему IP — для режима автозапуска «через NetPilot»: вне России zapret не нужен. Два
//!     открытых сервиса, второй — если первый не ответил. Запросы идут без прокси: через системный прокси
//!     определилась бы страна прокси. Страна проверяется заново при смене сети — её видно по локальному
//!     адресу, который система использовала бы для выхода в интернет (для этого ничего не отправляется).

use crate::autostart::{self, AutostartMode};
use crate::engines::zapret::control::{self, ZapretAction};
use crate::engines::zapret::service::{self, ServiceState};
use crate::i18n::t;
use crate::{config, privileged};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, UdpSocket};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(6);
/// EN: How often to look at the network. RU: Как часто смотреть на сеть.
const WATCH_INTERVAL: Duration = Duration::from_secs(20);
/// EN: After a failed check try again not earlier than this. RU: После неудачной проверки повторять не раньше чем через это время.
const RETRY_INTERVAL: Duration = Duration::from_secs(60);
pub const RUSSIA: &str = "RU";
const COUNTRY_EVENT: &str = "country-changed";

/// EN: The last detected country. RU: Последняя определённая страна.
#[derive(Default)]
pub struct CountryState(Mutex<Option<Country>>);

impl CountryState {
    pub fn get(&self) -> Option<Country> {
        self.0.lock().ok().and_then(|country| country.clone())
    }

    fn set(&self, country: Country) {
        if let Ok(mut current) = self.0.lock() {
            *current = Some(country);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Country {
    /// EN: ISO 3166 code, e.g. RU, DE. RU: Код ISO 3166, например RU, DE.
    pub code: String,
    /// EN: Unix seconds. RU: Секунды Unix.
    pub checked_at: u64,
}

#[derive(Deserialize)]
struct CountryIs {
    country: String,
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().no_proxy().timeout(REQUEST_TIMEOUT).build().map_err(|e| e.to_string())
}

fn valid(code: &str) -> Option<String> {
    let code = code.trim().to_uppercase();
    (code.len() == 2 && code.chars().all(|c| c.is_ascii_uppercase())).then_some(code)
}

/// EN: The country code of the public IP. RU: Код страны внешнего IP.
pub async fn detect() -> Result<String, String> {
    let client = client()?;
    let first = async {
        let answer: CountryIs = client.get("https://api.country.is/").send().await.ok()?.json().await.ok()?;
        valid(&answer.country)
    };
    if let Some(code) = first.await {
        return Ok(code);
    }
    let text = client
        .get("https://ipinfo.io/country")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    valid(&text).ok_or_else(|| format!("unexpected answer: {}", text.trim()))
}

/// EN: The local address of the route to the internet; changes with the network. UDP "connect" only picks
///     the route, no packet is sent. RU: Локальный адрес маршрута в интернет; меняется вместе с сетью.
///     «Соединение» UDP только выбирает маршрут, пакеты не отправляются.
pub fn network_fingerprint() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:53").ok()?;
    Some(socket.local_addr().ok()?.ip())
}

fn now_seconds() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default()
}

/// EN: Watches the network in the "through NetPilot" mode: the country is checked at start and when the
///     network changes; zapret is turned on in Russia and off outside it — only when the country changes, so a
///     manual "off" is not overridden. Turning the service on and off needs rights: the app started by the
///     autostart task has them, a manually opened app only shows the country.
/// RU: Следит за сетью в режиме «через NetPilot»: страна проверяется при запуске и при смене сети; zapret
///     включается в России и выключается вне её — только при смене страны, чтобы не перебивать ручное
///     «выключить». Включение и выключение службы требует прав: приложение, запущенное задачей автозапуска,
///     их имеет, открытое вручную — только показывает страну.
pub fn spawn_watcher(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_network: Option<IpAddr> = None;
        let mut failed_at: Option<std::time::Instant> = None;
        loop {
            let network = network_fingerprint();
            let retry_due = failed_at.is_some_and(|at| at.elapsed() >= RETRY_INTERVAL);
            if network.is_some() && (network != last_network || retry_due) {
                last_network = network;
                if autostart::status().mode == Some(AutostartMode::App) {
                    failed_at = if check_now(&app).await { None } else { Some(std::time::Instant::now()) };
                }
            }
            tokio::time::sleep(WATCH_INTERVAL).await;
        }
    });
}

/// EN: Checks the country now and acts on a change; false — the check failed.
///     Also called right after the "through NetPilot" mode is chosen.
/// RU: Проверяет страну сейчас и реагирует на смену; false — проверка не удалась.
///     Вызывается и сразу после выбора режима «через NetPilot».
pub async fn check_now(app: &AppHandle) -> bool {
    match detect().await {
        Ok(code) => {
            on_country(app, &code);
            true
        }
        Err(e) => {
            crate::warn!("country: check failed: {e}");
            false
        }
    }
}

fn on_country(app: &AppHandle, code: &str) {
    let country = Country { code: code.to_string(), checked_at: now_seconds() };
    let previous = app.try_state::<CountryState>().and_then(|state| state.get()).map(|known| known.code);
    let previous = previous.as_deref();
    if let Some(state) = app.try_state::<CountryState>() {
        state.set(country.clone());
    }
    let _ = app.emit(COUNTRY_EVENT, country);
    if previous == Some(code) {
        return;
    }
    crate::info!("country: {code} (was {previous:?})");
    let in_russia = code == RUSSIA || !config::load(app).russia_only;
    let running = matches!(service::query().state, ServiceState::Running);
    let action = match (in_russia, running) {
        (true, false) => Some(ZapretAction::Start),
        (false, true) => Some(ZapretAction::Stop),
        _ => None,
    };
    if let Some(action) = action {
        if privileged::is_elevated() {
            if let Err(e) = control::execute(&action) {
                crate::error!("country: {action:?} failed: {e}");
            }
            let _ = app.emit(crate::tray::STATE_EVENT, ());
        } else {
            crate::info!("country: not elevated, {action:?} is left to the user");
        }
    }
    let text = if code == RUSSIA {
        t("Вы в России — обход включён.")
    } else if in_russia {
        t("Вы не в России, но обход включён: так задано в настройках.")
    } else {
        t("Вы не в России — обход выключен. Вернётесь — включится сам.")
    };
    // EN: The first check in Russia is the usual case, no need to notify. RU: Первая проверка в России — обычный случай, уведомлять незачем.
    if previous.is_some() || code != RUSSIA {
        let _ = app.notification().builder().title("NetPilot").body(text).show();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_country_codes() {
        assert_eq!(valid("ru\n").as_deref(), Some("RU"));
        assert_eq!(valid("Russia"), None);
        assert_eq!(valid("R1"), None);
    }

    /// EN: Manual network check: `cargo test print_country -- --ignored --nocapture`.
    /// RU: Ручная проверка по сети: `cargo test print_country -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn print_country() {
        println!("{:?} {:?}", tauri::async_runtime::block_on(detect()), network_fingerprint());
    }
}
