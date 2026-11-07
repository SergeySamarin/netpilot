//! EN: Autostart. One switch, two modes:
//!     - "as in zapret" — the zapret service starts with Windows by itself (start type auto), the app is not
//!       needed;
//!     - "through NetPilot" — at logon Windows starts NetPilot in the tray (a Task Scheduler task with the
//!       highest rights, so there is no UAC prompt), and NetPilot turns zapret on itself, only in Russia.
//!       The service is set to manual start so it does not start past the country check.
//!     The mode is read from the system, not from the settings: the task exists — "through NetPilot",
//!     the service starts automatically — "as in zapret", otherwise autostart is off.
//!     Changes need administrator rights (called from privileged.rs).
//! RU: Автозапуск. Один переключатель, два режима:
//!     - «как в zapret» — служба zapret стартует с Windows сама (тип запуска auto), приложение не нужно;
//!     - «через NetPilot» — при входе Windows запускает NetPilot в трее (задача Планировщика с наивысшими
//!       правами, поэтому без окна UAC), и NetPilot сам включает zapret, только в России. Служба переводится
//!       на ручной запуск, чтобы не стартовать в обход проверки страны.
//!     Режим читается из системы, а не из настроек: есть задача — «через NetPilot», служба стартует
//!     автоматически — «как в zapret», иначе автозапуск выключен.
//!     Изменения требуют прав администратора (вызывается из privileged.rs).

use crate::engines::zapret::service::{self, ServiceState, StartType};
use serde::{Deserialize, Serialize};

/// EN: The flag of a launch from the Task Scheduler task: start in the tray, without the window.
/// RU: Ключ запуска из задачи Планировщика: стартовать в трее, без окна.
pub const TRAY_FLAG: &str = "--tray";
const TASK_NAME: &str = "NetPilot";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AutostartMode {
    /// EN: The service starts with Windows. RU: Служба стартует с Windows.
    Zapret,
    /// EN: NetPilot starts at logon and turns zapret on. RU: NetPilot стартует при входе и включает zapret.
    App,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutostartStatus {
    /// EN: None — autostart is off. RU: None — автозапуск выключен.
    pub mode: Option<AutostartMode>,
    /// EN: The zapret service is installed: without it there is nothing to start. RU: Служба zapret установлена: без неё запускать нечего.
    pub service_installed: bool,
}

pub fn status() -> AutostartStatus {
    let info = service::query();
    let service_installed = !matches!(info.state, ServiceState::NotInstalled);
    let mode = if win::task_exists() {
        Some(AutostartMode::App)
    } else if service_installed && info.start_type == StartType::Auto {
        Some(AutostartMode::Zapret)
    } else {
        None
    };
    AutostartStatus { mode, service_installed }
}

/// EN: Sets the mode; None turns autostart off. RU: Устанавливает режим; None выключает автозапуск.
pub fn set_mode(mode: Option<AutostartMode>) -> Result<(), String> {
    let installed = !matches!(service::query().state, ServiceState::NotInstalled);
    match mode {
        Some(AutostartMode::App) => {
            win::create_task()?;
            if installed {
                win::set_service_start(false)?;
            }
        }
        Some(AutostartMode::Zapret) => {
            win::delete_task()?;
            if installed {
                win::set_service_start(true)?;
            }
        }
        None => {
            win::delete_task()?;
            if installed {
                win::set_service_start(false)?;
            }
        }
    }
    crate::info!("autostart: mode={mode:?}");
    Ok(())
}

/// EN: The task XML. Unlike `schtasks /Create /SC ONLOGON`, it lets the task run on battery and without
///     a time limit (by default Windows kills a task after 3 days and does not start it on battery).
/// RU: XML задачи. В отличие от `schtasks /Create /SC ONLOGON`, позволяет запускаться от батареи и без
///     ограничения времени (по умолчанию Windows снимает задачу через 3 дня и не запускает её от батареи).
fn task_xml(exe: &str, user: &str) -> String {
    let escape = |text: &str| text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>NetPilot: start in the tray at logon</Description></RegistrationInfo>
  <Triggers><LogonTrigger><Enabled>true</Enabled><UserId>{user}</UserId></LogonTrigger></Triggers>
  <Principals><Principal id="Author"><UserId>{user}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>HighestAvailable</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author"><Exec><Command>{exe}</Command><Arguments>{TRAY_FLAG}</Arguments></Exec></Actions>
</Task>"#,
        exe = escape(exe),
        user = escape(user),
    )
}

#[cfg(windows)]
mod win {
    use super::{task_xml, TASK_NAME};
    use crate::engines::zapret::service::SERVICE_NAME;
    use crate::i18n::tf;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Output};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn run(program: &str, args: &[&str]) -> Result<Output, String> {
        Command::new(program)
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| tf("Не удалось запустить {program}: {error}", &[("program", &program), ("error", &e)]))
    }

    pub fn task_exists() -> bool {
        run("schtasks.exe", &["/Query", "/TN", TASK_NAME]).is_ok_and(|output| output.status.success())
    }

    pub fn create_task() -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let user = match (std::env::var("USERDOMAIN"), std::env::var("USERNAME")) {
            (Ok(domain), Ok(name)) => format!("{domain}\\{name}"),
            (_, Ok(name)) => name,
            _ => return Err(tf("Не удалось создать задачу автозапуска: {error}", &[("error", &"USERNAME")])),
        };
        let xml = task_xml(&exe.to_string_lossy(), &user);
        let path = std::env::temp_dir().join(format!("netpilot-task-{}.xml", std::process::id()));
        // EN: schtasks expects the XML in UTF-16 with a BOM. RU: schtasks ждёт XML в UTF-16 с BOM.
        let bytes: Vec<u8> = std::iter::once(0xFEFF_u16)
            .chain(xml.encode_utf16())
            .flat_map(u16::to_le_bytes)
            .collect();
        std::fs::write(&path, bytes)
            .map_err(|e| tf("Не удалось создать задачу автозапуска: {error}", &[("error", &e)]))?;
        let path_text = path.to_string_lossy().to_string();
        let output = run("schtasks.exe", &["/Create", "/TN", TASK_NAME, "/XML", &path_text, "/F"]);
        let _ = std::fs::remove_file(&path);
        let output = output?;
        if output.status.success() {
            Ok(())
        } else {
            let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(tf("Не удалось создать задачу автозапуска: {error}", &[("error", &error)]))
        }
    }

    pub fn delete_task() -> Result<(), String> {
        if !task_exists() {
            return Ok(());
        }
        let output = run("schtasks.exe", &["/Delete", "/TN", TASK_NAME, "/F"])?;
        if output.status.success() {
            Ok(())
        } else {
            let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(tf("Не удалось удалить задачу автозапуска: {error}", &[("error", &error)]))
        }
    }

    /// EN: auto — the service starts with Windows, demand — only on request. Done via sc.exe, so only the
    ///     start type changes and the winws.exe command line stays untouched.
    /// RU: auto — служба стартует с Windows, demand — только по запросу. Через sc.exe — так меняется только
    ///     тип запуска, командная строка winws.exe не трогается.
    pub fn set_service_start(auto: bool) -> Result<(), String> {
        let start = if auto { "auto" } else { "demand" };
        let output = run("sc.exe", &["config", SERVICE_NAME, "start=", start])?;
        if output.status.success() {
            Ok(())
        } else {
            let code = output.status.code().unwrap_or(-1);
            Err(tf("Не удалось изменить автозапуск службы (код {code})", &[("code", &code)]))
        }
    }
}

#[cfg(not(windows))]
mod win {
    pub fn task_exists() -> bool {
        false
    }
    pub fn create_task() -> Result<(), String> {
        Ok(())
    }
    pub fn delete_task() -> Result<(), String> {
        Ok(())
    }
    pub fn set_service_start(_auto: bool) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_xml_escapes_and_has_no_limits() {
        let xml = task_xml(r"C:\Program Files\A&B\netpilot.exe", "PC\\User");
        assert!(xml.contains(r"C:\Program Files\A&amp;B\netpilot.exe"));
        assert!(xml.contains("<Arguments>--tray</Arguments>"));
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(xml.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
        assert!(xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
    }
}
