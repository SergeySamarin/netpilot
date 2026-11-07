//! EN: Strategy host for the deep pick. Runs with administrator rights for the whole pick (one UAC prompt):
//!     stops the zapret service and, on the window's command, runs one strategy after another as a plain
//!     winws.exe. The window meanwhile checks video with each strategy. At the end the service is
//!     restored. Talks to the window via files: the window writes a command (a strategy or stop), the host
//!     answers with the status. If the window process is gone, the host restores the service by itself.
//! RU: Хост стратегий для глубокого подбора. Работает с правами администратора весь подбор (одно окно
//!     UAC): останавливает службу zapret и по команде окна запускает стратегии одну за другой как обычный
//!     winws.exe. Окно тем временем проверяет видео с каждой стратегией. В конце служба возвращается.
//!     Связь с окном — через файлы: окно пишет команду (стратегию или остановку), хост отвечает статусом.
//!     Если процесса окна не стало, хост сам возвращает службу.

use super::control::{self, ZapretAction};
use super::pick;
use super::service::{self, ServiceState};
use crate::i18n::t;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const POLL: Duration = Duration::from_millis(300);
/// EN: No command for this long — the window is stuck, the host stops. RU: Нет команды столько времени — окно зависло, хост останавливается.
const IDLE_LIMIT: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostFiles {
    pub command: PathBuf,
    pub status: PathBuf,
}

/// EN: A command of the window. `seq` grows with each command. RU: Команда окна. `seq` растёт с каждой командой.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostCommand {
    pub seq: u32,
    /// EN: Strategy to run; None — stop and restore the service. RU: Стратегия для запуска; None — остановиться и вернуть службу.
    pub strategy: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostStatus {
    /// EN: The last executed command. RU: Последняя выполненная команда.
    pub seq: u32,
    /// EN: The strategy of the command runs. RU: Стратегия команды запущена.
    pub ready: bool,
    pub error: Option<String>,
    /// EN: The host has finished and restored the service. RU: Хост закончил и вернул службу.
    pub finished: bool,
    pub restore_error: Option<String>,
}

pub fn run(zapret_dir: &Path, parent_pid: u32, files: &HostFiles) -> Result<(), String> {
    control::prepare(zapret_dir)?;
    let was_running = matches!(service::query().state, ServiceState::Running);
    if was_running {
        control::execute(&ZapretAction::Stop)?;
    }
    control::reset_driver();
    if service::is_winws_running() {
        return Err(t("winws.exe запущен вручную (из bat-файла). Закройте его и повторите подбор"));
    }
    crate::info!("strategy host: started, was_running={was_running}");
    write_status(&files.status, &HostStatus::default());

    let mut child: Option<Child> = None;
    let mut last_seq = 0;
    let mut last_command_at = Instant::now();
    loop {
        if !win::is_process_alive(parent_pid) {
            crate::warn!("strategy host: the app window process is gone");
            break;
        }
        if last_command_at.elapsed() > IDLE_LIMIT {
            crate::warn!("strategy host: no commands for {IDLE_LIMIT:?}");
            break;
        }
        let command = fs::read_to_string(&files.command)
            .ok()
            .and_then(|text| serde_json::from_str::<HostCommand>(&text).ok())
            .filter(|command| command.seq > last_seq);
        let Some(command) = command else {
            sleep(POLL);
            continue;
        };
        last_seq = command.seq;
        last_command_at = Instant::now();
        if let Some(previous) = child.take() {
            pick::stop_winws(previous);
        }
        let Some(strategy) = command.strategy else {
            break;
        };
        crate::debug!("strategy host: start {strategy}");
        let status = match pick::start_winws(zapret_dir, &strategy) {
            Ok(started) => {
                child = Some(started);
                flush_dns();
                HostStatus { seq: command.seq, ready: true, ..HostStatus::default() }
            }
            Err(error) => {
                crate::warn!("strategy host: {strategy}: {error}");
                HostStatus { seq: command.seq, error: Some(error), ..HostStatus::default() }
            }
        };
        write_status(&files.status, &status);
    }

    if let Some(previous) = child.take() {
        pick::stop_winws(previous);
    }
    let restore_error = if was_running { control::execute(&ZapretAction::Start).err() } else { None };
    crate::info!("strategy host: finished, restore_error={restore_error:?}");
    write_status(
        &files.status,
        &HostStatus { seq: last_seq, finished: true, restore_error, ..HostStatus::default() },
    );
    Ok(())
}

/// EN: Clears the Windows DNS cache: otherwise the first strategy resolves the video servers itself and the
///     next ones get the addresses ready, and the first one looks slower.
/// RU: Сбрасывает кэш DNS Windows: иначе первая стратегия сама узнаёт адреса серверов видео, а следующим
///     они достаются готовыми, и первая выглядит медленнее.
fn flush_dns() {
    let mut command = std::process::Command::new("ipconfig");
    command.arg("/flushdns").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    if let Err(e) = command.status() {
        crate::warn!("strategy host: ipconfig /flushdns failed: {e}");
    }
}

fn write_status(path: &Path, status: &HostStatus) {
    if let Ok(text) = serde_json::to_string(status) {
        let _ = fs::write(path, text);
    }
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject};

    pub fn is_process_alive(pid: u32) -> bool {
        // EN: SAFETY: the handle is checked for null and closed; waiting with zero timeout only reads the state.
        // RU: SAFETY: дескриптор проверяется на null и закрывается; ожидание с нулевым таймаутом только читает состояние.
        unsafe {
            let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
            if handle.is_null() {
                return false;
            }
            let alive = WaitForSingleObject(handle, 0) == WAIT_TIMEOUT;
            CloseHandle(handle);
            alive
        }
    }
}

#[cfg(not(windows))]
mod win {
    pub fn is_process_alive(_pid: u32) -> bool {
        true
    }
}
