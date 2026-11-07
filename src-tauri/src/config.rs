//! EN: App settings, stored in config.json in the app config folder (%APPDATA%\com.netpilot.app).
//! RU: Настройки приложения, хранятся в config.json в папке настроек (%APPDATA%\com.netpilot.app).

use crate::i18n::{t, tf, Lang};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    /// EN: Folder with zapret files (bin/, lists/, general*.bat). None means first run.
    /// RU: Папка с файлами zapret (bin/, lists/, general*.bat). None — первый запуск.
    pub zapret_dir: Option<PathBuf>,
    /// EN: true — the app created the folder when downloading, false — the user chose it.
    /// RU: true — папку создало приложение при скачивании, false — её указал пользователь.
    pub zapret_managed: bool,
    /// EN: Language chosen by the user. None — follow the system language.
    /// RU: Язык, выбранный пользователем. None — как в системе.
    #[serde(default)]
    pub language: Option<Lang>,
    /// EN: Previous version for rollback: a version folder (app folder) or a backup copy (user folder).
    /// RU: Предыдущая версия для отката: папка версии (папка приложения) или резервная копия (папка пользователя).
    #[serde(default)]
    pub rollback_dir: Option<PathBuf>,
    /// EN: Latest zapret version on GitHub at the last check. RU: Последняя версия zapret на GitHub при последней проверке.
    #[serde(default)]
    pub latest_version: Option<String>,
    /// EN: Time of the last update check, Unix seconds. RU: Время последней проверки обновлений, секунды Unix.
    #[serde(default)]
    pub update_checked_at: Option<u64>,
    /// EN: Services turned off in the "Services" card (see engines/zapret/services.rs).
    /// RU: Сервисы, выключенные в карточке «Сервисы» (см. engines/zapret/services.rs).
    #[serde(default)]
    pub disabled_services: Vec<String>,
    /// EN: Author list entries the user has already seen. RU: Записи списков авторов, которые пользователь уже видел.
    #[serde(default)]
    pub known_author_domains: Vec<String>,
    /// EN: Entries added by the authors in the last update, highlighted until "Got it".
    /// RU: Записи, добавленные авторами в последнем обновлении, подсвечиваются до «Понятно».
    #[serde(default)]
    pub fresh_domains: Vec<String>,
    /// EN: The window close button hides the window to the tray. RU: Крестик окна сворачивает в трей.
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
    /// EN: In the "through NetPilot" autostart mode turn zapret on only in Russia.
    /// RU: В режиме автозапуска «через NetPilot» включать zapret только в России.
    #[serde(default = "default_true")]
    pub russia_only: bool,
}

fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            zapret_dir: None,
            zapret_managed: false,
            language: None,
            rollback_dir: None,
            latest_version: None,
            update_checked_at: None,
            disabled_services: Vec::new(),
            known_author_domains: Vec::new(),
            fresh_domains: Vec::new(),
            close_to_tray: true,
            russia_only: true,
        }
    }
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| tf("Не удалось найти папку настроек: {error}", &[("error", &e)]))?;
    Ok(dir.join("config.json"))
}

pub fn load(app: &AppHandle) -> AppConfig {
    let Ok(path) = config_path(app) else {
        return AppConfig::default();
    };
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    let path = config_path(app)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| tf("Не удалось создать папку настроек: {error}", &[("error", &e)]))?;
    }
    let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| tf("Не удалось сохранить настройки: {error}", &[("error", &e)]))
}

/// EN: Shortcut for commands: an error message when the zapret folder is not chosen.
/// RU: Для команд: сообщение, когда папка zapret не выбрана.
pub fn no_zapret_dir() -> String {
    t("Папка zapret не выбрана")
}
