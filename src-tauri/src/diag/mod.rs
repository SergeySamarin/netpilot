//! EN: "Diagnostics": the YouTube video check. The app opens its own window with the official YouTube
//!     player (diag-player.html), the player plays a planned list of videos, and the window's browser
//!     writes a Chromium net log. From the player events and the log the check builds a timeline: when
//!     videos started and stopped, which video requests hung, whether QUIC works.
//!     The window has its own WebView2 data folder: the net log and other browser arguments need a
//!     separate browser process, and a fresh folder each time means the browser does not remember from
//!     the previous check that QUIC is broken on some server. The folder is deleted after the check.
//! RU: «Диагностика»: проверка видео YouTube. Приложение открывает своё окно с официальным плеером
//!     YouTube (diag-player.html), плеер проигрывает заданный список видео, а браузер окна пишет сетевой
//!     журнал Chromium. По событиям плеера и журналу проверка строит шкалу: когда видео стартовали и
//!     останавливались, какие запросы видео зависли, работает ли QUIC.
//!     У окна своя папка данных WebView2: сетевому журналу и другим аргументам браузера нужен отдельный
//!     процесс браузера, а свежая папка каждый раз — чтобы браузер не помнил с прошлой проверки, что QUIC
//!     на каком-то сервере сломан. После проверки папка удаляется.

pub mod analysis;
pub mod deep;
pub mod netlog;

use crate::i18n::{self, t};
use analysis::{PlayerEvent, RequestKind, VideoTrace};
use netlog::NetLogReader;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Listener, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

const PLAYER_WINDOW: &str = "diag-player";
/// EN: Window to keep watching the video after the check. RU: Окно, чтобы досмотреть видео после проверки.
const VIEWER_WINDOW: &str = "diag-viewer";
const PLAYER_PAGE: &str = "diag-player.html";
/// EN: Events from the player page. RU: События страницы плеера.
const PLAYER_EVENT: &str = "diag-player";
const PROGRESS_EVENT: &str = "diag-video-progress";

const TICK: Duration = Duration::from_millis(500);
const PROGRESS_INTERVAL: Duration = Duration::from_secs(2);
/// EN: The player page loads and reports for this long beyond the plan before the check is stopped.
/// RU: Сколько сверх плана ждём загрузку страницы и отчёт плеера, прежде чем остановить проверку.
const EXTRA_TIME: Duration = Duration::from_secs(60);
/// EN: After the window closes the browser finishes writing the log. RU: После закрытия окна браузер дописывает журнал.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(6);
const MS_IN_SECOND: u64 = 1000;

/// EN: Shown in the window at 640×360: the player is drawn in full size and scaled down, because YouTube
///     picks the quality by the player size. RU: В окне видно 640×360: плеер рисуется в полном размере и
///     уменьшается, потому что YouTube выбирает качество по размеру плеера.
const WINDOW_WIDTH: f64 = 672.0;
const WINDOW_HEIGHT: f64 = 470.0;
const VIEWER_WIDTH: f64 = 960.0;
const VIEWER_HEIGHT: f64 = 600.0;

/// EN: A long public video in 4K that allows embedding (Big Buck Bunny, Blender Foundation, 10.5 min).
/// RU: Длинное публичное видео в 4K, которое разрешено встраивать (Big Buck Bunny, Blender Foundation, 10,5 мин).
const LONG_VIDEO: &str = "aqz-KE-bpKQ";
/// EN: Different popular videos: each new one usually comes from another video server.
/// RU: Разные популярные видео: каждое новое обычно идёт с другого сервера видео.
const PLAYLIST: [&str; 12] = [
    "LXb3EKWsInQ",
    "dQw4w9WgXcQ",
    "9bZkp7q19f0",
    "YQHsXMglC9A",
    "OPf0YbXqDm0",
    "hT_nvWreIhg",
    "RgKAFK5djSk",
    "JGwWNGJdvx8",
    "60ItHLz5WEA",
    "njX2bu-_Vw4",
    "kJQP7kiw5Fk",
    "jNQXAC9IVRw",
];
const VIDEO_SECONDS: u32 = 120;
const LONG_SECONDS: u32 = 300;
const PLAYLIST_ITEM_SECONDS: u32 = 10;
/// EN: Videos of the playlist per strategy in the deep pick. RU: Видео плейлиста на одну стратегию в глубоком подборе.
const PICK_VIDEOS: usize = 6;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Scenario {
    /// EN: One long video. RU: Одно длинное видео.
    Video,
    /// EN: Many videos in a row, as a playlist or Shorts. RU: Много видео подряд, как плейлист или Shorts.
    Playlist,
    /// EN: One video for 5 minutes. RU: Одно видео на 5 минут.
    Long,
    /// EN: A short playlist for one strategy of the deep pick. RU: Короткий плейлист для одной стратегии глубокого подбора.
    Pick,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Quality {
    #[serde(rename = "1080p")]
    P1080,
    #[serde(rename = "1440p")]
    P1440,
    #[serde(rename = "2160p")]
    P2160,
}

impl Quality {
    /// EN: Player size for this quality. RU: Размер плеера для этого качества.
    fn size(self) -> (u32, u32) {
        match self {
            Self::P1080 => (1920, 1080),
            Self::P1440 => (2560, 1440),
            Self::P2160 => (3840, 2160),
        }
    }
}

/// EN: Videos and seconds for each. RU: Видео и секунды на каждое.
fn plan(scenario: Scenario) -> Vec<(&'static str, u32)> {
    match scenario {
        Scenario::Video => vec![(LONG_VIDEO, VIDEO_SECONDS)],
        Scenario::Long => vec![(LONG_VIDEO, LONG_SECONDS)],
        Scenario::Playlist => PLAYLIST.iter().map(|id| (*id, PLAYLIST_ITEM_SECONDS)).collect(),
        Scenario::Pick => PLAYLIST.iter().take(PICK_VIDEOS).map(|id| (*id, PLAYLIST_ITEM_SECONDS)).collect(),
    }
}

#[derive(Default)]
pub struct DiagState {
    /// EN: The running check. None — no check. RU: Идущая проверка. None — проверки нет.
    run: Mutex<Option<Arc<Run>>>,
    /// EN: Keep watching the video in a separate window after the check. RU: Досмотреть видео в отдельном окне после проверки.
    keep_watching: AtomicBool,
    /// EN: Cancel flag of the running deep pick. RU: Флаг отмены идущего глубокого подбора.
    deep_cancel: Mutex<Option<Arc<AtomicBool>>>,
}

#[derive(Default)]
struct Run {
    events: Mutex<Vec<PlayerEvent>>,
    done: AtomicBool,
    cancel: AtomicBool,
    window_closed: AtomicBool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoCheckProgress {
    pub elapsed_ms: u64,
    pub total_ms: u64,
    pub trace: VideoTrace,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoCheckResult {
    pub scenario: Scenario,
    pub quality: Quality,
    pub quic: bool,
    /// EN: Unix ms. RU: мс Unix.
    pub started_at_ms: u64,
    /// EN: Stopped by the user or the window was closed. RU: Остановлена пользователем или окно закрыли.
    pub cancelled: bool,
    pub trace: VideoTrace,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

/// EN: Clears the "check is running" mark even if the check fails. RU: Снимает отметку «идёт проверка», даже если проверка упала.
struct RunGuard<'a>(&'a DiagState);

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut run) = self.0.run.lock() {
            *run = None;
        }
    }
}

fn work_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    Ok(base.join("diag"))
}

/// EN: Leftovers of a check interrupted by closing the app. RU: Остатки проверки, прерванной закрытием приложения.
pub fn clean_up_on_start(app: &AppHandle) {
    if let Ok(dir) = work_dir(app)
        && dir.exists()
    {
        let _ = fs::remove_dir_all(&dir);
    }
}

/// EN: Browser arguments of the player window. RU: Аргументы браузера окна плеера.
fn browser_args(netlog: &Path, quic: bool) -> String {
    // EN: wry passes the first flag by default; setting own arguments replaces it, so it is repeated.
    //     No proxy: the check is about zapret, a system proxy would hide its work.
    // RU: Первый флаг wry передаёт по умолчанию; свои аргументы его заменяют, поэтому он повторён.
    //     Без прокси: проверяется zapret, системный прокси скрыл бы его работу.
    let mut args = format!(
        "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --autoplay-policy=no-user-gesture-required --no-proxy-server --log-net-log=\"{}\" --net-log-capture-mode=Default",
        netlog.display()
    );
    if !quic {
        args.push_str(" --disable-quic");
    }
    args
}

fn lang_code() -> String {
    serde_json::to_value(i18n::lang())
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// EN: `keep`: None — the "keep watching" option is not shown (deep pick), Some — its state.
/// RU: `keep`: None — галка «досмотреть» не показывается (глубокий подбор), Some — её состояние.
fn page_url(scenario: Scenario, quality: Quality, keep: Option<bool>) -> String {
    let (width, height) = quality.size();
    let plan = plan(scenario)
        .iter()
        .map(|(id, seconds)| format!("{id}:{seconds}"))
        .collect::<Vec<_>>()
        .join(",");
    let lang = lang_code();
    let keep = match keep {
        None => "",
        Some(true) => "&keep=1",
        Some(false) => "&keep=0",
    };
    format!("{PLAYER_PAGE}?plan={plan}&width={width}&height={height}&lang={lang}{keep}")
}

/// EN: How the check ends. RU: Чем заканчивается проверка.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// EN: Just closes the window. RU: Просто закрывает окно.
    Close,
    /// EN: The last stage: if the user asked, the video goes on in a viewer window.
    /// RU: Последний этап: если пользователь попросил, видео продолжается в окне просмотра.
    MayKeepWatching,
    /// EN: Part of the deep pick: the "keep watching" option is hidden. RU: Часть глубокого подбора: галки «досмотреть» нет.
    DeepPick,
}

/// EN: Runs the check: opens the player window, sends progress every 2 s, returns the result.
/// RU: Проводит проверку: открывает окно плеера, раз в 2 с шлёт прогресс, возвращает итог.
pub async fn run_video_check(
    app: &AppHandle,
    state: &DiagState,
    scenario: Scenario,
    quality: Quality,
    quic: bool,
    ending: Ending,
) -> Result<VideoCheckResult, String> {
    let run = Arc::new(Run::default());
    {
        let mut current = state.run.lock().map_err(|e| e.to_string())?;
        if current.is_some() {
            return Err(t("Проверка уже идёт"));
        }
        *current = Some(run.clone());
    }
    let _guard = RunGuard(state);

    // EN: Each check has its own folder: the previous one may still be deleted in the background.
    // RU: У каждой проверки своя папка: предыдущая может ещё удаляться в фоне.
    let dir = work_dir(app)?.join(now_ms().to_string());
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let netlog_path = dir.join("netlog.json");
    let total_ms: u64 = plan(scenario).iter().map(|(_, seconds)| u64::from(*seconds) * MS_IN_SECOND).sum();
    crate::info!("video check: start scenario={scenario:?} quality={quality:?} quic={quic}");

    let run_for_events = run.clone();
    let listener = app.listen(PLAYER_EVENT, move |event| {
        match serde_json::from_str::<PlayerEvent>(event.payload()) {
            Ok(player_event) => {
                crate::debug!("video check: player {player_event:?}");
                if matches!(player_event, PlayerEvent::Done { .. }) {
                    run_for_events.done.store(true, Ordering::SeqCst);
                }
                if let Ok(mut events) = run_for_events.events.lock() {
                    events.push(player_event);
                }
            }
            Err(e) => crate::warn!("video check: bad player event {}: {e}", event.payload()),
        }
    });

    let started_at_ms = now_ms();
    let keep = (ending != Ending::DeepPick).then(|| state.keep_watching.load(Ordering::SeqCst));
    // EN: A viewer from the previous check would play along and spoil the measurement.
    // RU: Окно просмотра от прошлой проверки играло бы параллельно и портило замер.
    if let Some(viewer) = app.get_webview_window(VIEWER_WINDOW) {
        let _ = viewer.destroy();
    }
    let url = page_url(scenario, quality, keep);
    let window = WebviewWindowBuilder::new(app, PLAYER_WINDOW, WebviewUrl::App(url.into()))
        .title(t("NetPilot — проверка видео"))
        .inner_size(WINDOW_WIDTH, WINDOW_HEIGHT)
        .resizable(false)
        .maximizable(false)
        .data_directory(dir.join("webview"))
        .additional_browser_args(&browser_args(&netlog_path, quic))
        .build();
    let window = match window {
        Ok(window) => window,
        Err(e) => {
            app.unlisten(listener);
            crate::error!("video check: window failed: {e}");
            return Err(e.to_string());
        }
    };
    let run_for_window = run.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            run_for_window.window_closed.store(true, Ordering::SeqCst);
        }
    });

    let mut reader = NetLogReader::new(&netlog_path);
    let deadline = Duration::from_millis(total_ms) + EXTRA_TIME;
    let mut last_progress = Duration::ZERO;
    let mut elapsed = Duration::ZERO;
    while !run.done.load(Ordering::SeqCst)
        && !run.cancel.load(Ordering::SeqCst)
        && !run.window_closed.load(Ordering::SeqCst)
        && elapsed < deadline
    {
        tokio::time::sleep(TICK).await;
        elapsed += TICK;
        if elapsed - last_progress >= PROGRESS_INTERVAL {
            last_progress = elapsed;
            reader.read();
            let trace = build_trace(&run, &reader, started_at_ms, quic);
            let elapsed_ms = now_ms().saturating_sub(started_at_ms).min(total_ms);
            let _ = app.emit(PROGRESS_EVENT, VideoCheckProgress { elapsed_ms, total_ms, trace });
        }
    }
    let cancelled = !run.done.load(Ordering::SeqCst);
    let finished_at_ms = now_ms();

    if !run.window_closed.load(Ordering::SeqCst) {
        let _ = window.destroy();
    }
    if !cancelled && ending == Ending::MayKeepWatching && state.keep_watching.load(Ordering::SeqCst) {
        let events = run.events.lock().map(|events| events.clone()).unwrap_or_default();
        if let Some(PlayerEvent::Done { video: Some(video), position, .. }) = events.last() {
            open_viewer(app, video, position.unwrap_or(0.0));
        }
    }
    app.unlisten(listener);
    wait_for_log(&netlog_path).await;
    reader.read();
    let mut trace = build_trace(&run, &reader, started_at_ms, quic);
    trace.duration_ms = trace.duration_ms.min(finished_at_ms.saturating_sub(started_at_ms));
    remove_dir_later(dir);

    log_details(&trace);
    crate::info!(
        "video check: end cancelled={cancelled} starts={} median={:?} worst={:?} stalls={} hung={}/{} quic={:?} servers={}",
        trace.stats.starts,
        trace.stats.start_median_ms,
        trace.stats.start_worst_ms,
        trace.stats.stalls,
        trace.stats.hung,
        trace.stats.requests,
        trace.stats.quic,
        trace.servers.len()
    );
    Ok(VideoCheckResult { scenario, quality, quic, started_at_ms, cancelled, trace })
}

/// EN: Opens a plain window with the same video from the same moment: no net log, any size, full screen.
/// RU: Открывает обычное окно с тем же видео с того же места: без сетевого журнала, любого размера, на весь экран.
fn open_viewer(app: &AppHandle, video: &str, position: f64) {
    let start = position.max(0.0).floor();
    let url = format!("{PLAYER_PAGE}?view={video}&start={start}&lang={}", lang_code());
    let result = WebviewWindowBuilder::new(app, VIEWER_WINDOW, WebviewUrl::App(url.into()))
        .title(t("NetPilot — видео"))
        .inner_size(VIEWER_WIDTH, VIEWER_HEIGHT)
        .build();
    if let Err(e) = result {
        crate::warn!("video check: viewer window failed: {e}");
    }
}

/// EN: The "keep watching" option of the player window. RU: Галка «досмотреть» окна плеера.
pub fn set_keep_watching(state: &DiagState, enabled: bool) {
    state.keep_watching.store(enabled, Ordering::SeqCst);
}

/// EN: Debug lines for the log recording: QUIC errors per server and every hung or failed request.
/// RU: Строки debug для записи журнала: ошибки QUIC по серверам и каждый зависший или упавший запрос.
fn log_details(trace: &VideoTrace) {
    if !crate::log::is_recording() {
        return;
    }
    for server in &trace.servers {
        crate::debug!("video check: server {} quic={:?} error={:?}", server.host, server.quic, server.quic_error);
    }
    for request in &trace.requests {
        if matches!(request.kind, RequestKind::Hung | RequestKind::Failed) {
            crate::debug!(
                "video check: {:?} request at {} ms to {}: waited {} ms, quic={}, error={:?}",
                request.kind,
                request.start_ms,
                trace.servers[request.server].host,
                request.wait_ms,
                request.quic,
                request.error
            );
        }
    }
}

fn build_trace(run: &Run, reader: &NetLogReader, started_at_ms: u64, quic: bool) -> VideoTrace {
    let events = run.events.lock().map(|events| events.clone()).unwrap_or_default();
    analysis::build(started_at_ms, now_ms(), &events, &reader.summary(), quic)
}

/// EN: The browser closes the log JSON when its process exits; waits for that, but not forever.
/// RU: Браузер закрывает JSON журнала при выходе своего процесса; ждём этого, но не бесконечно.
async fn wait_for_log(path: &Path) {
    let mut waited = Duration::ZERO;
    while waited < FLUSH_TIMEOUT {
        let finished = fs::read(path)
            .map(|bytes| bytes.trim_ascii_end().ends_with(b"}"))
            .unwrap_or(false);
        if finished {
            return;
        }
        tokio::time::sleep(TICK).await;
        waited += TICK;
    }
    crate::debug!("video check: net log not finished in {FLUSH_TIMEOUT:?}, reading what is written");
}

/// EN: The browser process may hold the folder for a few seconds after closing — retry in the background.
/// RU: Процесс браузера может держать папку несколько секунд после закрытия — повторяем в фоне.
fn remove_dir_later(dir: PathBuf) {
    const ATTEMPTS: u32 = 10;
    tauri::async_runtime::spawn(async move {
        for _ in 0..ATTEMPTS {
            if fs::remove_dir_all(&dir).is_ok() || !dir.exists() {
                return;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        crate::warn!("video check: could not remove {}", dir.display());
    });
}

/// EN: "Stop": the check closes the window and returns what it collected.
/// RU: «Прервать»: проверка закрывает окно и возвращает то, что успела собрать.
pub fn cancel(state: &DiagState) {
    if let Ok(deep) = state.deep_cancel.lock()
        && let Some(deep) = deep.as_ref()
    {
        deep.store(true, Ordering::SeqCst);
    }
    if let Ok(run) = state.run.lock()
        && let Some(run) = run.as_ref()
    {
        run.cancel.store(true, Ordering::SeqCst);
    }
}
