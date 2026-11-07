//! EN: Running actions as administrator.
//!     If the app already runs as administrator, the action runs right away. Otherwise the app starts
//!     itself via UAC with `--privileged <request> <result>`: that process performs one action, writes
//!     the result to a file and exits without opening a window.
//! RU: Выполнение действий с правами администратора.
//!     Если приложение уже запущено от администратора — действие выполняется сразу. Иначе приложение
//!     запускает само себя через UAC с ключом `--privileged <запрос> <результат>`: этот процесс выполняет
//!     одно действие, пишет результат в файл и завершается, окно не открывается.

use crate::engines::zapret::control::{self, ZapretAction};
use crate::i18n::{self, t, tf, Lang};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const PRIVILEGED_FLAG: &str = "--privileged";

#[derive(Serialize, Deserialize)]
struct ActionRequest {
    /// EN: UI language, so that error messages from the separate process are in the same language.
    /// RU: Язык интерфейса, чтобы ошибки из отдельного процесса были на том же языке.
    lang: Lang,
    /// EN: A log recording is on: the separate process writes debug lines too.
    /// RU: Идёт запись журнала: отдельный процесс тоже пишет строки debug.
    #[serde(default)]
    is_recording: bool,
    action: ZapretAction,
}

#[derive(Serialize, Deserialize)]
struct ActionResult {
    error: Option<String>,
}

/// EN: The app runs as administrator (started by the autostart task): actions need no UAC.
/// RU: Приложение запущено с правами администратора (задачей автозапуска): действия идут без UAC.
pub fn is_elevated() -> bool {
    win::is_elevated()
}

/// EN: Runs an action as administrator. Blocks the thread until it finishes (waits for UAC).
/// RU: Выполняет действие с правами администратора. Блокирует поток до завершения (ждёт UAC).
pub fn run(action: &ZapretAction) -> Result<(), String> {
    let result = run_inner(action);
    match &result {
        Ok(()) => crate::info!("privileged result: ok"),
        Err(e) => crate::error!("privileged result: {e}"),
    }
    result
}

fn run_inner(action: &ZapretAction) -> Result<(), String> {
    let is_elevated = win::is_elevated();
    crate::info!("privileged run (elevated={is_elevated}): {action:?}");
    if is_elevated {
        return control::execute(action);
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let temp = std::env::temp_dir();
    let request_path = temp.join(format!("netpilot-{stamp}-request.json"));
    let result_path = temp.join(format!("netpilot-{stamp}-result.json"));

    let request = ActionRequest { lang: i18n::lang(), is_recording: crate::log::is_recording(), action: action.clone() };
    let text = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    fs::write(&request_path, text).map_err(|e| tf("Не удалось записать запрос: {error}", &[("error", &e)]))?;

    let launched = win::run_elevated_self(&request_path, &result_path);
    let _ = fs::remove_file(&request_path);
    launched?;

    let result = fs::read_to_string(&result_path)
        .map_err(|_| t("Процесс с правами администратора не вернул результат"))?;
    let _ = fs::remove_file(&result_path);
    let result: ActionResult = serde_json::from_str(&result).map_err(|e| e.to_string())?;
    match result.error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// EN: Called from main(): if the process was started with `--privileged`, performs the action and
///     returns the exit code. For a normal launch returns None.
/// RU: Вызывается из main(): если процесс запущен с `--privileged`, выполняет действие и возвращает
///     код выхода. Для обычного запуска возвращает None.
pub fn handle_args() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some(PRIVILEGED_FLAG) {
        return None;
    }
    let (Some(request_path), Some(result_path)) = (args.get(2), args.get(3)) else {
        return Some(2);
    };

    let error = fs::read_to_string(request_path)
        .map_err(|e| tf("Не удалось прочитать запрос: {error}", &[("error", &e)]))
        .and_then(|text| serde_json::from_str::<ActionRequest>(&text).map_err(|e| e.to_string()))
        .and_then(|request| {
            i18n::set_lang(request.lang);
            crate::log::set_recording(request.is_recording);
            crate::info!("admin process: {:?}", request.action);
            control::execute(&request.action)
        })
        .err();
    match &error {
        None => crate::info!("admin process end: ok"),
        Some(e) => crate::error!("admin process end: {e}"),
    }

    let code = i32::from(error.is_some());
    let text = serde_json::to_string(&ActionResult { error }).unwrap_or_default();
    let _ = fs::write(PathBuf::from(result_path), text);
    Some(code)
}

#[cfg(windows)]
mod win {
    use super::PRIVILEGED_FLAG;
    use crate::i18n::{t, tf};
    use std::path::Path;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
    use windows_sys::Win32::System::Threading::{WaitForSingleObject, INFINITE};
    use windows_sys::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    const ERROR_CANCELLED: u32 = 1223;

    pub fn is_elevated() -> bool {
        // EN: SAFETY: no arguments, only reads the token of the current process.
        // RU: SAFETY: функция без аргументов, только читает токен текущего процесса.
        unsafe { IsUserAnAdmin() != 0 }
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn run_elevated_self(request_path: &Path, result_path: &Path) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let verb = wide("runas");
        let file = wide(&exe.to_string_lossy());
        let params = wide(&format!("{PRIVILEGED_FLAG} \"{}\" \"{}\"", request_path.display(), result_path.display()));

        // EN: SAFETY: the struct is zeroed and filled with the needed fields, the strings live until the
        //     end of the call, the process handle is closed after waiting.
        // RU: SAFETY: структура заполняется нулями и нужными полями, строки живут до конца вызова,
        //     дескриптор процесса закрывается после ожидания.
        unsafe {
            let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
            info.fMask = SEE_MASK_NOCLOSEPROCESS;
            info.lpVerb = verb.as_ptr();
            info.lpFile = file.as_ptr();
            info.lpParameters = params.as_ptr();
            info.nShow = SW_HIDE;

            if ShellExecuteExW(&mut info) == 0 {
                return Err(match GetLastError() {
                    ERROR_CANCELLED => t("Действие отменено в окне Windows (UAC)"),
                    code => tf("Не удалось запустить с правами администратора (код {code})", &[("code", &code)]),
                });
            }
            if !info.hProcess.is_null() {
                WaitForSingleObject(info.hProcess, INFINITE);
                CloseHandle(info.hProcess);
            }
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod win {
    use crate::i18n::t;
    use std::path::Path;

    pub fn is_elevated() -> bool {
        false
    }

    pub fn run_elevated_self(_request_path: &Path, _result_path: &Path) -> Result<(), String> {
        Err(t("Запуск с правами администратора поддерживается только в Windows"))
    }
}
