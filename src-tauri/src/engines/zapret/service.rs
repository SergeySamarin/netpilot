//! EN: The `zapret` Windows service: state, start type, path to winws.exe and strategy name.
//!     Read only — no administrator rights needed.
//! RU: Служба Windows `zapret`: состояние, тип запуска, путь к winws.exe и название стратегии.
//!     Только чтение — права администратора не нужны.

use serde::Serialize;
use std::path::PathBuf;

pub const SERVICE_NAME: &str = "zapret";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ServiceState {
    NotInstalled,
    Running,
    Stopped,
    Starting,
    Stopping,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StartType {
    Auto,
    Manual,
    Disabled,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub state: ServiceState,
    pub start_type: StartType,
    /// EN: zapret folder the service runs from (one level above bin\winws.exe).
    /// RU: Папка zapret, из которой запускается служба (на уровень выше bin\winws.exe).
    pub zapret_dir: Option<PathBuf>,
    /// EN: Strategy name that service.bat writes to the registry when installing the service.
    /// RU: Название стратегии, которое service.bat записывает в реестр при установке службы.
    pub strategy_id: Option<String>,
}

impl ServiceInfo {
    fn not_installed() -> Self {
        Self {
            state: ServiceState::NotInstalled,
            start_type: StartType::Unknown,
            zapret_dir: None,
            strategy_id: None,
        }
    }
}

#[cfg(windows)]
pub fn query() -> ServiceInfo {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let key_path = format!("SYSTEM\\CurrentControlSet\\Services\\{SERVICE_NAME}");
    let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(&key_path) else {
        return ServiceInfo::not_installed();
    };

    let start_type = match key.get_value::<u32, _>("Start") {
        Ok(2) => StartType::Auto,
        Ok(3) => StartType::Manual,
        Ok(4) => StartType::Disabled,
        _ => StartType::Unknown,
    };
    let zapret_dir = key
        .get_value::<String, _>("ImagePath")
        .ok()
        .and_then(|image_path| zapret_dir_from_image_path(&image_path));
    let strategy_id = key.get_value::<String, _>("zapret-discord-youtube").ok();

    ServiceInfo { state: query_state(), start_type, zapret_dir, strategy_id }
}

/// EN: One line for the log: zapret service state, its exe and strategy, and the WinDivert driver path.
/// RU: Одна строка для журнала: состояние службы zapret, её exe и стратегия, путь драйвера WinDivert.
#[cfg(windows)]
pub fn describe_for_log() -> String {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let read = |service: &str, value: &str| -> String {
        hklm.open_subkey(format!("SYSTEM\\CurrentControlSet\\Services\\{service}"))
            .and_then(|key| key.get_value::<String, _>(value))
            .unwrap_or_else(|_| "-".into())
    };
    let image = read(SERVICE_NAME, "ImagePath");
    let exe = image.split(" --").next().unwrap_or("-").trim().to_string();
    format!(
        "service={:?} exe={exe} strategy={} driver={}",
        query_state(),
        read(SERVICE_NAME, "zapret-discord-youtube"),
        read("WinDivert", "ImagePath")
    )
}

#[cfg(not(windows))]
pub fn describe_for_log() -> String {
    String::new()
}

#[cfg(not(windows))]
pub fn query() -> ServiceInfo {
    ServiceInfo::not_installed()
}

#[cfg(windows)]
fn query_state() -> ServiceState {
    use windows_service::service::{ServiceAccess, ServiceState as WinState};
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    let Ok(manager) = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) else {
        return ServiceState::Unknown;
    };
    let Ok(service) = manager.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS) else {
        return ServiceState::NotInstalled;
    };
    match service.query_status().map(|status| status.current_state) {
        Ok(WinState::Running) => ServiceState::Running,
        Ok(WinState::Stopped) => ServiceState::Stopped,
        Ok(WinState::StartPending) => ServiceState::Starting,
        Ok(WinState::StopPending) => ServiceState::Stopping,
        _ => ServiceState::Unknown,
    }
}

/// EN: Whether winws.exe is running anywhere — by the service or manually from a bat file.
/// RU: Запущен ли где-нибудь winws.exe — службой или вручную из bat-файла.
#[cfg(windows)]
pub fn is_winws_running() -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq winws.exe", "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).to_lowercase().contains("winws.exe"))
        .unwrap_or(false)
}

#[cfg(not(windows))]
pub fn is_winws_running() -> bool {
    false
}

/// EN: `"C:\zapret\bin\winws.exe" --wf-tcp ...` -> `C:\zapret`.
/// RU: `"C:\zapret\bin\winws.exe" --wf-tcp ...` -> `C:\zapret`.
fn zapret_dir_from_image_path(image_path: &str) -> Option<PathBuf> {
    let trimmed = image_path.trim();
    let exe = if let Some(rest) = trimmed.strip_prefix('"') {
        rest.split('"').next()?
    } else {
        trimmed.split_whitespace().next()?
    };
    let exe = PathBuf::from(exe);
    Some(exe.parent()?.parent()?.to_path_buf())
}
