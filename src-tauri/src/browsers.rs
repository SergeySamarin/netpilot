//! EN: QUIC in browsers, set by their policies in HKLM\SOFTWARE\Policies (all users of the PC).
//!     - Chromium browsers read `QuicAllowed`: 0 — QUIC is off, the browser always uses an ordinary TCP
//!       connection; no value — as the browser decides itself (QUIC is on).
//!     - Firefox reads the `Preferences` policy (JSON): `network.http.http3.enable = false` turns HTTP/3
//!       (QUIC) off. Other preferences in the same policy are kept as they are.
//!     Reading needs no rights, writing needs administrator rights (called from privileged.rs). A browser
//!     picks the change up after a restart. Any policy makes it show "Managed by your organization".
//! RU: QUIC в браузерах, задаётся их политиками в HKLM\SOFTWARE\Policies (для всех пользователей ПК).
//!     - Браузеры на Chromium читают `QuicAllowed`: 0 — QUIC выключен, браузер всегда идёт обычным
//!       соединением TCP; значения нет — как решит сам браузер (QUIC включён).
//!     - Firefox читает политику `Preferences` (JSON): `network.http.http3.enable = false` выключает HTTP/3
//!       (QUIC). Остальные настройки в той же политике остаются как были.
//!     Чтение — без прав, запись — с правами администратора (вызывается из privileged.rs). Браузер
//!     подхватывает изменение после перезапуска. С любой политикой он пишет «Управляется организацией».

use crate::i18n::t;
use serde::Serialize;
use serde_json::{json, Map, Value};

const CHROMIUM_VALUE: &str = "QuicAllowed";
const FIREFOX_KEY: &str = r"SOFTWARE\Policies\Mozilla\Firefox";
const FIREFOX_VALUE: &str = "Preferences";
const FIREFOX_HTTP3: &str = "network.http.http3.enable";

enum Policy {
    /// EN: QuicAllowed in this key. RU: QuicAllowed в этом разделе.
    Chromium(&'static str),
    Firefox,
}

struct Browser {
    id: &'static str,
    /// EN: The name in the UI language. RU: Название на языке интерфейса.
    name: fn() -> String,
    /// EN: File name under App Paths, used to tell if the browser is installed.
    /// RU: Имя файла в App Paths, по нему видно, установлен ли браузер.
    exe: &'static str,
    /// EN: Part of the path to the exe: browser.exe is a too common name. RU: Часть пути к exe: browser.exe — слишком общее имя.
    path_hint: Option<&'static str>,
    policy: Policy,
}

const BROWSERS: [Browser; 5] = [
    Browser {
        id: "chrome",
        name: || "Google Chrome".to_string(),
        exe: "chrome.exe",
        path_hint: None,
        policy: Policy::Chromium(r"SOFTWARE\Policies\Google\Chrome"),
    },
    Browser {
        id: "edge",
        name: || "Microsoft Edge".to_string(),
        exe: "msedge.exe",
        path_hint: None,
        policy: Policy::Chromium(r"SOFTWARE\Policies\Microsoft\Edge"),
    },
    Browser {
        id: "yandex",
        name: || t("Яндекс Браузер"),
        exe: "browser.exe",
        path_hint: Some("Yandex"),
        policy: Policy::Chromium(r"SOFTWARE\Policies\YandexBrowser"),
    },
    Browser {
        id: "brave",
        name: || "Brave".to_string(),
        exe: "brave.exe",
        path_hint: None,
        policy: Policy::Chromium(r"SOFTWARE\Policies\BraveSoftware\Brave"),
    },
    Browser {
        id: "firefox",
        name: || "Firefox".to_string(),
        exe: "firefox.exe",
        path_hint: None,
        policy: Policy::Firefox,
    },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserQuic {
    pub id: &'static str,
    pub name: String,
    /// EN: QUIC is turned off by the policy. RU: QUIC выключен политикой.
    pub quic_off: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowsersQuic {
    /// EN: Installed supported browsers. RU: Установленные поддерживаемые браузеры.
    pub browsers: Vec<BrowserQuic>,
}

fn is_quic_off(policy: &Policy) -> bool {
    match policy {
        Policy::Chromium(key) => win::read_dword(key, CHROMIUM_VALUE) == Some(0),
        Policy::Firefox => win::read_string(FIREFOX_KEY, FIREFOX_VALUE).is_some_and(|text| firefox_quic_off(&text)),
    }
}

pub fn quic_status() -> BrowsersQuic {
    let browsers = BROWSERS
        .iter()
        .filter(|browser| win::is_installed(browser.exe, browser.path_hint))
        .map(|browser| BrowserQuic { id: browser.id, name: (browser.name)(), quic_off: is_quic_off(&browser.policy) })
        .collect();
    BrowsersQuic { browsers }
}

/// EN: Turns QUIC on (removes our setting) or off in the installed browsers. Turning on also cleans up
///     browsers removed since then. RU: Включает QUIC (убирает нашу настройку) или выключает в установленных
///     браузерах. Включение убирает настройку и у браузеров, удалённых с тех пор.
pub fn set_quic(enabled: bool) -> Result<(), String> {
    for browser in &BROWSERS {
        if !enabled && !win::is_installed(browser.exe, browser.path_hint) {
            continue;
        }
        match browser.policy {
            Policy::Chromium(key) => {
                if enabled {
                    if win::read_dword(key, CHROMIUM_VALUE).is_some() {
                        win::delete_value(key, CHROMIUM_VALUE)?;
                    }
                } else {
                    win::write_dword(key, CHROMIUM_VALUE, 0)?;
                }
            }
            Policy::Firefox => {
                let current = win::read_string(FIREFOX_KEY, FIREFOX_VALUE);
                if enabled && current.is_none() {
                    continue;
                }
                match firefox_with_quic(current.as_deref(), enabled) {
                    Some(text) => win::write_string(FIREFOX_KEY, FIREFOX_VALUE, &text)?,
                    None => win::delete_value(FIREFOX_KEY, FIREFOX_VALUE)?,
                }
            }
        }
    }
    crate::info!("browsers: QUIC {}", if enabled { "allowed" } else { "forbidden" });
    Ok(())
}

/// EN: The Firefox Preferences policy turns HTTP/3 off. RU: Политика Preferences Firefox выключает HTTP/3.
fn firefox_quic_off(policy: &str) -> bool {
    serde_json::from_str::<Value>(policy)
        .ok()
        .and_then(|value| value.get(FIREFOX_HTTP3)?.get("Value")?.as_bool())
        == Some(false)
}

/// EN: The Preferences policy with HTTP/3 on (our setting removed) or off. None — nothing is left, the value
///     can be deleted. A policy that is not JSON is replaced only when turning off.
/// RU: Политика Preferences с включённым HTTP/3 (наша настройка убрана) или выключенным. None — ничего не
///     осталось, значение можно удалить. Политика не в JSON заменяется только при выключении.
fn firefox_with_quic(current: Option<&str>, enabled: bool) -> Option<String> {
    let mut prefs: Map<String, Value> = current
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    if enabled {
        prefs.remove(FIREFOX_HTTP3);
    } else {
        prefs.insert(FIREFOX_HTTP3.to_string(), json!({ "Value": false, "Status": "locked" }));
    }
    (!prefs.is_empty()).then(|| Value::Object(prefs).to_string())
}

#[cfg(windows)]
mod win {
    use crate::i18n::tf;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_SET_VALUE};
    use winreg::RegKey;

    const APP_PATHS: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths";

    fn write_error(e: &std::io::Error) -> String {
        tf("Не удалось записать политику браузера: {error}", &[("error", e)])
    }

    /// EN: Installed for all users (HKLM) or for the current one (HKCU, Chrome often installs so).
    /// RU: Установлен для всех (HKLM) или для текущего пользователя (HKCU, так часто ставится Chrome).
    pub fn is_installed(exe: &str, path_hint: Option<&str>) -> bool {
        [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER].into_iter().any(|root| {
            let Ok(key) = RegKey::predef(root).open_subkey(format!(r"{APP_PATHS}\{exe}")) else {
                return false;
            };
            let path: String = key.get_value("").unwrap_or_default();
            path_hint.is_none_or(|hint| path.contains(hint))
        })
    }

    pub fn read_dword(key: &str, name: &str) -> Option<u32> {
        RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(key).ok()?.get_value(name).ok()
    }

    /// EN: REG_SZ or REG_MULTI_SZ (lines are joined). RU: REG_SZ или REG_MULTI_SZ (строки склеиваются).
    pub fn read_string(key: &str, name: &str) -> Option<String> {
        let key = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(key).ok()?;
        key.get_value::<String, _>(name)
            .ok()
            .or_else(|| key.get_value::<Vec<String>, _>(name).ok().map(|lines| lines.join("\n")))
    }

    pub fn write_dword(key: &str, name: &str, value: u32) -> Result<(), String> {
        let (key, _) = RegKey::predef(HKEY_LOCAL_MACHINE).create_subkey(key).map_err(|e| write_error(&e))?;
        key.set_value(name, &value).map_err(|e| write_error(&e))
    }

    pub fn write_string(key: &str, name: &str, value: &str) -> Result<(), String> {
        let (key, _) = RegKey::predef(HKEY_LOCAL_MACHINE).create_subkey(key).map_err(|e| write_error(&e))?;
        key.set_value(name, &value).map_err(|e| write_error(&e))
    }

    pub fn delete_value(key: &str, name: &str) -> Result<(), String> {
        let key = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(key, KEY_SET_VALUE)
            .map_err(|e| write_error(&e))?;
        key.delete_value(name).map_err(|e| write_error(&e))
    }
}

#[cfg(not(windows))]
mod win {
    pub fn is_installed(_exe: &str, _path_hint: Option<&str>) -> bool {
        false
    }

    pub fn read_dword(_key: &str, _name: &str) -> Option<u32> {
        None
    }

    pub fn read_string(_key: &str, _name: &str) -> Option<String> {
        None
    }

    pub fn write_dword(_key: &str, _name: &str, _value: u32) -> Result<(), String> {
        Ok(())
    }

    pub fn write_string(_key: &str, _name: &str, _value: &str) -> Result<(), String> {
        Ok(())
    }

    pub fn delete_value(_key: &str, _name: &str) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firefox_policy_keeps_other_preferences() {
        let other = r#"{"browser.startup.homepage":{"Value":"about:blank","Status":"default"}}"#;
        let off = firefox_with_quic(Some(other), false).unwrap();
        assert!(firefox_quic_off(&off));
        assert!(off.contains("browser.startup.homepage"));

        let on = firefox_with_quic(Some(&off), true).unwrap();
        assert!(!firefox_quic_off(&on));
        assert!(on.contains("browser.startup.homepage"));
    }

    #[test]
    fn firefox_policy_disappears_when_only_ours() {
        let off = firefox_with_quic(None, false).unwrap();
        assert!(firefox_quic_off(&off));
        assert_eq!(firefox_with_quic(Some(&off), true), None);
    }
}
