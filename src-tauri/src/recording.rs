//! EN: Log recording for support: the user starts it, repeats the problem and stops it. While it is on,
//!     all log lines, debug included, go to recording.log (see log.rs). At the end the app packs one zip:
//!     the recording, netpilot.log (what happened before) and a system snapshot at start and end.
//!     Limits, so nothing piles up: a recording stops by itself after 30 minutes or at 2 MB of text,
//!     only the last 3 archives are kept, a recording left by a closed app is deleted at the next start.
//! RU: Запись журнала для поддержки: пользователь включает её, повторяет проблему и останавливает. Пока она
//!     идёт, все строки журнала, включая debug, пишутся в recording.log (см. log.rs). В конце приложение
//!     собирает один zip: запись, netpilot.log (что было до неё) и снимок системы в начале и в конце.
//!     Ограничения, чтобы ничего не копилось: запись сама останавливается через 30 минут или на 2 МБ текста,
//!     хранятся только 3 последних архива, запись, оставшаяся от закрытого приложения, удаляется при запуске.

use crate::i18n::{t, tf};
use crate::log;
use crate::snapshot;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

const MAX_DURATION: Duration = Duration::from_secs(30 * 60);
const KEEP_ARCHIVES: usize = 3;
const ARCHIVE_PREFIX: &str = "netpilot-log-";
const MONITOR_INTERVAL: Duration = Duration::from_secs(2);
const FINISHED_EVENT: &str = "log-recording-finished";

/// EN: The current recording. None — no recording.
/// RU: Текущая запись. None — запись не идёт.
#[derive(Default)]
pub struct RecordingState(Mutex<Option<Session>>);

struct Session {
    /// EN: Start time, Unix ms: also tells one recording from another. RU: Время начала, мс Unix: заодно отличает одну запись от другой.
    started_at_ms: u64,
    start_snapshot: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStatus {
    /// EN: Start time, Unix ms. None — no recording. RU: Время начала, мс Unix. None — запись не идёт.
    pub started_at_ms: Option<u64>,
    pub max_seconds: u64,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    Manual,
    TimeLimit,
    SizeLimit,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingResult {
    pub archive_path: PathBuf,
    pub reason: StopReason,
    pub seconds: u64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

/// EN: Called at app start: a recording cannot outlive the app, its leftover file is deleted.
/// RU: Вызывается при запуске: запись не переживает закрытие приложения, оставшийся файл удаляется.
pub fn clean_up_on_start() {
    let Some(path) = log::recording_path().filter(|path| path.exists()) else {
        return;
    };
    let _ = fs::remove_file(&path);
    crate::info!("log recording: removed a leftover recording.log");
}

pub fn status(state: &RecordingState) -> RecordingStatus {
    let started_at_ms = state.0.lock().ok().and_then(|session| session.as_ref().map(|s| s.started_at_ms));
    RecordingStatus { started_at_ms, max_seconds: MAX_DURATION.as_secs() }
}

pub async fn start(app: AppHandle) -> Result<RecordingStatus, String> {
    let state = app.state::<RecordingState>();
    if status(&state).started_at_ms.is_some() {
        return Ok(status(&state));
    }
    let path = log::recording_path().ok_or_else(|| t("Не удалось найти папку журнала"))?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| tf("Не удалось создать папку журнала: {error}", &[("error", &e)]))?;
    }
    fs::write(&path, "").map_err(|e| tf("Не удалось начать запись: {error}", &[("error", &e)]))?;

    let started_at_ms = now_ms();
    log::set_recording(true);
    crate::info!("log recording started");
    let start_snapshot = snapshot::collect(&app, "start").await;
    if let Ok(mut session) = state.0.lock() {
        *session = Some(Session { started_at_ms, start_snapshot });
    }

    tauri::async_runtime::spawn(monitor(app.clone(), started_at_ms));
    Ok(status(&state))
}

/// EN: Stops the recording by itself when a limit is reached and tells the window with an event.
/// RU: Сама останавливает запись при достижении предела и сообщает окну событием.
async fn monitor(app: AppHandle, started_at_ms: u64) {
    loop {
        tokio::time::sleep(MONITOR_INTERVAL).await;
        let state = app.state::<RecordingState>();
        if status(&state).started_at_ms != Some(started_at_ms) {
            return;
        }
        let size = log::recording_path().map(|path| log::file_size(&path)).unwrap_or(0);
        let reason = if size >= log::MAX_RECORDING_SIZE {
            StopReason::SizeLimit
        } else if now_ms().saturating_sub(started_at_ms) >= u64::try_from(MAX_DURATION.as_millis()).unwrap_or(u64::MAX) {
            StopReason::TimeLimit
        } else {
            continue;
        };
        match stop(app.clone(), reason).await {
            Ok(result) => {
                let _ = app.emit(FINISHED_EVENT, result);
            }
            Err(e) => crate::error!("log recording auto stop: {e}"),
        }
        return;
    }
}

pub async fn stop(app: AppHandle, reason: StopReason) -> Result<RecordingResult, String> {
    let session = app
        .state::<RecordingState>()
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or_else(|| t("Запись журнала не идёт"))?;
    let seconds = now_ms().saturating_sub(session.started_at_ms) / 1000;
    crate::info!("log recording stopping: reason={reason:?} seconds={seconds}");

    // EN: The end snapshot is taken while the recording is still on: its checks get into the recording.
    // RU: Снимок в конце снимается, пока запись ещё идёт: его проверки попадают в запись.
    let end_snapshot = snapshot::collect(&app, "end").await;
    log::set_recording(false);
    crate::info!("log recording stopped");

    let dir = log::dir().ok_or_else(|| t("Не удалось найти папку журнала"))?;
    let system = format!("{}\n\n{}", session.start_snapshot, end_snapshot);
    let archive_path = dir.join(archive_name());
    let packed = pack(&dir, &archive_path, &system);
    if let Some(path) = log::recording_path() {
        let _ = fs::remove_file(path);
    }
    packed.map_err(|e| tf("Не удалось собрать архив журнала: {error}", &[("error", &e)]))?;
    remove_old_archives(&dir);
    crate::info!("log recording archive: {}", archive_path.display());

    Ok(RecordingResult { archive_path, reason, seconds })
}

fn archive_name() -> String {
    let (year, month, day, hour, minute, second) = snapshot::local_time();
    format!("{ARCHIVE_PREFIX}{year}-{month:02}-{day:02}_{hour:02}-{minute:02}-{second:02}.zip")
}

fn pack(dir: &Path, archive_path: &Path, system: &str) -> Result<(), String> {
    let file = fs::File::create(archive_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("system.txt", options).map_err(|e| e.to_string())?;
    zip.write_all(system.as_bytes()).map_err(|e| e.to_string())?;
    for name in ["recording.log", "netpilot.log"] {
        if let Ok(content) = fs::read(dir.join(name)) {
            zip.start_file(name, options).map_err(|e| e.to_string())?;
            zip.write_all(&content).map_err(|e| e.to_string())?;
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// EN: Keeps only the newest archives: the name has the date, so sorting by name sorts by time.
/// RU: Оставляет только самые новые архивы: в имени дата, поэтому сортировка по имени — это сортировка по времени.
fn remove_old_archives(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut archives: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy())
                .is_some_and(|name| name.starts_with(ARCHIVE_PREFIX) && name.ends_with(".zip"))
        })
        .collect();
    archives.sort();
    let extra = archives.len().saturating_sub(KEEP_ARCHIVES);
    for path in archives.into_iter().take(extra) {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_newest_archives() {
        let dir = std::env::temp_dir().join(format!("netpilot-archives-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        for day in 1..=5 {
            fs::write(dir.join(format!("{ARCHIVE_PREFIX}2026-11-0{day}_10-00-00.zip")), "").expect("write");
        }
        fs::write(dir.join("netpilot.log"), "").expect("write");
        remove_old_archives(&dir);
        let mut left: Vec<String> = fs::read_dir(&dir)
            .expect("read")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                "netpilot-log-2026-11-03_10-00-00.zip",
                "netpilot-log-2026-11-04_10-00-00.zip",
                "netpilot-log-2026-11-05_10-00-00.zip",
                "netpilot.log"
            ]
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
