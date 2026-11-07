//! EN: Removal of NetPilot: the uninstaller runs `netpilot.exe --uninstall` (see windows/hooks.nsh).
//!     NetPilot asks what to do with the zapret service and cleans up after itself with administrator rights:
//!     the autostart task is removed; the service is either removed or left to start with Windows (in the
//!     "through NetPilot" mode it was on manual start, and nobody would start it any more).
//! RU: Удаление NetPilot: деинсталлятор запускает `netpilot.exe --uninstall` (см. windows/hooks.nsh).
//!     NetPilot спрашивает, что делать со службой zapret, и убирает за собой с правами администратора:
//!     задача автозапуска удаляется; служба либо удаляется, либо остаётся и стартует с Windows (в режиме
//!     «через NetPilot» она была на ручном запуске, и запускать её стало бы некому).

use crate::autostart::{self, AutostartMode};
use crate::config::AppConfig;
use crate::engines::zapret::control::{self, ZapretAction};
use crate::engines::zapret::service::{self, ServiceState};
use crate::i18n::{self, t};
use crate::privileged;

const UNINSTALL_FLAG: &str = "--uninstall";

/// EN: Called from main(): for `--uninstall` cleans up and returns the exit code, otherwise None.
/// RU: Вызывается из main(): для `--uninstall` убирает за собой и возвращает код выхода, иначе None.
pub fn handle_args() -> Option<i32> {
    if std::env::args().nth(1).as_deref() != Some(UNINSTALL_FLAG) {
        return None;
    }
    i18n::set_lang(saved_language().unwrap_or_else(i18n::system_lang));
    let installed = !matches!(service::query().state, ServiceState::NotInstalled);
    let remove_service = installed
        && win::ask_yes_no(&t(
            "Удалить и службу zapret?\n\n«Да» — служба и драйвер WinDivert будут удалены, обход выключится.\n«Нет» — служба останется и будет запускаться вместе с Windows, как обычный zapret.",
        ));
    crate::info!("uninstall: remove_service={remove_service}");
    match privileged::run(&ZapretAction::Cleanup { remove_service }) {
        Ok(()) => Some(0),
        Err(_) => Some(1),
    }
}

/// EN: Runs with administrator rights (see control.rs). RU: Выполняется с правами администратора (см. control.rs).
pub fn cleanup(remove_service: bool) -> Result<(), String> {
    if remove_service {
        autostart::set_mode(None)?;
        control::execute(&ZapretAction::Remove)
    } else {
        autostart::set_mode(Some(AutostartMode::Zapret))
    }
}

/// EN: The language from the app settings; the uninstaller has no Tauri app to find the settings folder.
/// RU: Язык из настроек приложения; у деинсталлятора нет приложения Tauri, чтобы найти папку настроек.
fn saved_language() -> Option<i18n::Lang> {
    let path = std::path::PathBuf::from(std::env::var_os("APPDATA")?).join("com.netpilot.app").join("config.json");
    let config: AppConfig = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    config.language
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::UI::WindowsAndMessaging::{IDYES, MB_ICONQUESTION, MB_YESNO, MessageBoxW};

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn ask_yes_no(text: &str) -> bool {
        let text = wide(text);
        let title = wide("NetPilot");
        // EN: SAFETY: both strings are null-terminated and live until the end of the call.
        // RU: SAFETY: обе строки оканчиваются нулём и живут до конца вызова.
        unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_YESNO | MB_ICONQUESTION) == IDYES }
    }
}

#[cfg(not(windows))]
mod win {
    pub fn ask_yes_no(_text: &str) -> bool {
        false
    }
}
