//! EN: App log: %LOCALAPPDATA%\com.netpilot.app\logs\netpilot.log. The app window and the administrator
//!     processes write to the same file; each line has the level, the process role and id.
//!     Error, warn and info lines are always written, debug lines only while a log recording is on: then
//!     every line also goes to recording.log (see recording.rs), which stops growing at 2 MB.
//!     A file over 1 MB is renamed to netpilot.old.log (the previous .old is replaced), so the log takes
//!     at most about 2 MB.
//! RU: Журнал приложения: %LOCALAPPDATA%\com.netpilot.app\logs\netpilot.log. Окно приложения и процессы с
//!     правами администратора пишут в один файл; в каждой строке — уровень, роль и номер процесса.
//!     Строки error, warn и info пишутся всегда, debug — только пока идёт запись журнала: тогда каждая
//!     строка пишется ещё и в recording.log (см. recording.rs), который перестаёт расти на 2 МБ.
//!     Файл больше 1 МБ переименовывается в netpilot.old.log (прежний .old заменяется), поэтому журнал
//!     занимает не больше ~2 МБ.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const MS_IN_SECOND: u128 = 1000;
const SECONDS_IN_DAY: u128 = 86400;
const SECONDS_IN_HOUR: u128 = 3600;
const SECONDS_IN_MINUTE: u128 = 60;
const MAX_SIZE: u64 = 1024 * 1024;
pub const MAX_RECORDING_SIZE: u64 = 2 * 1024 * 1024;
const LOG_FILE: &str = "netpilot.log";
const RECORDING_FILE: &str = "recording.log";

static IS_RECORDING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN ",
            Self::Info => "INFO ",
            Self::Debug => "DEBUG",
        }
    }
}

pub fn dir() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(local).join("com.netpilot.app").join("logs"))
}

pub fn recording_path() -> Option<PathBuf> {
    Some(dir()?.join(RECORDING_FILE))
}

pub fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

pub fn is_recording() -> bool {
    IS_RECORDING.load(Ordering::Relaxed)
}

pub fn set_recording(is_on: bool) {
    IS_RECORDING.store(is_on, Ordering::Relaxed);
}

/// EN: UTC time of day with milliseconds — enough to order events within a session.
/// RU: Время суток UTC с миллисекундами — достаточно, чтобы упорядочить события сессии.
fn timestamp() -> String {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or_default();
    let secs = ms / MS_IN_SECOND % SECONDS_IN_DAY;
    format!(
        "{:02}:{:02}:{:02}.{:03}Z",
        secs / SECONDS_IN_HOUR,
        secs % SECONDS_IN_HOUR / SECONDS_IN_MINUTE,
        secs % SECONDS_IN_MINUTE,
        ms % MS_IN_SECOND
    )
}

fn rotate_if_large(path: &Path) {
    if file_size(path) > MAX_SIZE {
        let _ = fs::rename(path, path.with_extension("old.log"));
    }
}

fn append(path: &Path, line: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}

pub fn write(level: Level, message: &str) {
    let is_recording = is_recording();
    if level == Level::Debug && !is_recording {
        return;
    }
    let Some(dir) = dir() else {
        return;
    };
    let _ = fs::create_dir_all(&dir);
    let role = if std::env::args().nth(1).as_deref() == Some("--privileged") { "admin" } else { "main" };
    let line = format!("{} {} [{role} {}] {message}\n", timestamp(), level.label(), std::process::id());

    if is_recording {
        let recording = dir.join(RECORDING_FILE);
        if file_size(&recording) < MAX_RECORDING_SIZE {
            append(&recording, &line);
        }
    }
    if level != Level::Debug {
        let path = dir.join(LOG_FILE);
        rotate_if_large(&path);
        append(&path, &line);
    }
}

/// EN: `error!("text {}", value)` — an error: the action failed.
/// RU: `error!("текст {}", value)` — ошибка: действие не выполнено.
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::log::write($crate::log::Level::Error, &format!($($arg)*))
    };
}

/// EN: `warn!(…)` — something went wrong, but the action went on.
/// RU: `warn!(…)` — что-то пошло не так, но действие продолжилось.
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::log::write($crate::log::Level::Warn, &format!($($arg)*))
    };
}

/// EN: `info!(…)` — a main event: an action, its result, the state of the service.
/// RU: `info!(…)` — главное событие: действие, его итог, состояние службы.
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::log::write($crate::log::Level::Info, &format!($($arg)*))
    };
}

/// EN: `debug!(…)` — details, written only while a log recording is on. Without a recording the text is
///     not even formatted.
/// RU: `debug!(…)` — подробности, пишутся только пока идёт запись журнала. Без записи текст даже не
///     собирается.
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if $crate::log::is_recording() {
            $crate::log::write($crate::log::Level::Debug, &format!($($arg)*))
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_file_moves_to_old() {
        let dir = std::env::temp_dir().join(format!("netpilot-log-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("netpilot.log");
        let old = dir.join("netpilot.old.log");

        fs::write(&path, vec![b'x'; 10]).expect("write small");
        rotate_if_large(&path);
        assert!(path.exists() && !old.exists());

        fs::write(&path, vec![b'x'; usize::try_from(MAX_SIZE).expect("size") + 1]).expect("write large");
        rotate_if_large(&path);
        assert!(!path.exists() && old.exists());

        let _ = fs::remove_dir_all(&dir);
    }
}
