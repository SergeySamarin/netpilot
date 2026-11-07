//! EN: Commands called by the UI via invoke().
//! RU: Команды, которые вызывает интерфейс через invoke().

use crate::checks::{self, ServiceCheck};
use crate::config::{self, AppConfig};
use crate::autostart::{self, AutostartMode, AutostartStatus};
use crate::browsers::{self, BrowsersQuic};
use crate::geo::{Country, CountryState};
use crate::diag::deep::DeepPickResult;
use crate::diag::{self, DiagState, Ending, Quality, Scenario, VideoCheckResult};
use crate::engines::zapret::control::{self, SwitchMode, ZapretAction};
use crate::engines::zapret::files::{self, Strategy};
use crate::engines::zapret::fsutil;
use crate::engines::zapret::update::{self, MigrationReport};
use crate::engines::zapret::pick::{PickFiles, PickProgress, PickResult};
use crate::engines::zapret::release::{self, ReleaseInfo};
use crate::engines::zapret::service::{self, ServiceInfo, ServiceState, StartType};
use crate::engines::zapret::services::{self as zapret_services, AddResult, ServicesState};
use crate::i18n::{self, t, tf, Lang};
use crate::privileged;
use crate::recording::{self, RecordingResult, RecordingState, RecordingStatus, StopReason};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZapretOverview {
    /// EN: zapret folder from the settings. None — first run, the folder choice screen is shown.
    /// RU: Папка zapret из настроек. None — первый запуск, показываем экран выбора.
    pub zapret_dir: Option<PathBuf>,
    /// EN: The folder was created by the app (true) or chosen by the user (false).
    /// RU: Папку создало приложение (true) или указал пользователь (false).
    pub managed: bool,
    /// EN: Where the app downloads zapret (without the version folder).
    /// RU: Куда приложение скачивает zapret (без папки версии).
    pub download_base_dir: Option<PathBuf>,
    /// EN: Folder of the installed zapret service, if it differs from the chosen one.
    /// RU: Папка установленной службы zapret, если она отличается от выбранной.
    pub detected_dir: Option<PathBuf>,
    /// EN: Problem with the chosen folder: deleted, has no zapret files, etc.
    /// RU: Ошибка выбранной папки: удалена, не содержит zapret и т. п.
    pub dir_error: Option<String>,
    pub version: Option<String>,
    pub strategies: Vec<Strategy>,
    pub service: ServiceInfo,
    /// EN: The service is installed, but runs from a different folder than the chosen one.
    /// RU: Служба установлена, но работает из другой папки, чем выбрана в приложении.
    pub service_from_other_dir: bool,
    /// EN: winws.exe was started manually from a bat file, not by the service — they conflict.
    /// RU: winws.exe запущен вручную из bat-файла, а не службой — они конфликтуют.
    pub standalone_winws: bool,
    /// EN: Latest version on GitHub at the last check. RU: Последняя версия на GitHub при последней проверке.
    pub latest_version: Option<String>,
    pub has_update: bool,
    /// EN: Version available for rollback and its folder. RU: Версия, на которую можно откатиться, и её папка.
    pub rollback_version: Option<String>,
    pub rollback_dir: Option<PathBuf>,
    pub update_checked_at: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    pub overview: ZapretOverview,
    pub version: String,
    pub report: MigrationReport,
    /// EN: The service strategy is missing in the new version — the service was left as it was.
    /// RU: Стратегии службы нет в новой версии — служба оставлена как была.
    pub strategy_missing: Option<String>,
}

const UPDATE_STAGE_EVENT: &str = "zapret-update-stage";

/// EN: C:\ProgramData\NetPilot\zapret — the zapret service runs as the system, and the path has no user
///     name (Cyrillic in the path breaks zapret).
/// RU: C:\ProgramData\NetPilot\zapret — служба zapret работает от имени системы, а в пути нет имени
///     пользователя (кириллица в пути ломает zapret).
fn download_base_dir() -> Option<PathBuf> {
    fsutil::managed_root().ok()
}

fn same_dir(a: &PathBuf, b: &PathBuf) -> bool {
    fsutil::same_dir(a, b)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

fn build_overview(config: &AppConfig) -> ZapretOverview {
    let service = service::query();

    let detected_dir = service.zapret_dir.clone().filter(|dir| {
        files::validate_dir(dir).is_ok() && config.zapret_dir.as_ref().is_none_or(|chosen| !same_dir(chosen, dir))
    });

    let (dir_error, version, strategies) = match &config.zapret_dir {
        Some(dir) => match files::validate_dir(dir) {
            Ok(()) => (None, files::read_version(dir), files::list_strategies(dir)),
            Err(e) => (Some(e), None, Vec::new()),
        },
        None => (None, None, Vec::new()),
    };

    let service_from_other_dir = match (&config.zapret_dir, &service.zapret_dir) {
        (Some(chosen), Some(running)) => !same_dir(chosen, running),
        _ => false,
    };
    let standalone_winws = !matches!(service.state, ServiceState::Running | ServiceState::Starting | ServiceState::Stopping)
        && service::is_winws_running();

    let has_update = match (&version, &config.latest_version) {
        (Some(current), Some(latest)) => update::compare_versions(latest, current) == std::cmp::Ordering::Greater,
        _ => false,
    };
    let rollback_version = config.rollback_dir.as_deref().filter(|dir| dir.is_dir()).and_then(files::read_version);

    ZapretOverview {
        zapret_dir: config.zapret_dir.clone(),
        managed: config.zapret_managed,
        download_base_dir: download_base_dir(),
        detected_dir,
        dir_error,
        version,
        strategies,
        service,
        service_from_other_dir,
        standalone_winws,
        latest_version: config.latest_version.clone(),
        has_update,
        rollback_dir: rollback_version.as_ref().and(config.rollback_dir.clone()),
        rollback_version,
        update_checked_at: config.update_checked_at,
    }
}

/// EN: Saves the zapret folder, keeping the other settings. A new folder has no previous version.
/// RU: Сохраняет папку zapret, не трогая остальные настройки. У новой папки нет предыдущей версии.
fn save_zapret_dir(app: &AppHandle, dir: Option<PathBuf>, managed: bool) -> Result<AppConfig, String> {
    let mut config = config::load(app);
    config.zapret_dir = dir;
    config.zapret_managed = managed;
    config.rollback_dir = None;
    config::save(app, &config)?;
    Ok(config)
}

/// EN: The service strategy, if the service runs from this folder; otherwise the service is not ours.
/// RU: Стратегия службы, если служба работает из этой папки; иначе служба не наша.
fn own_service_strategy(dir: &PathBuf) -> (Option<String>, bool) {
    let info = service::query();
    let is_ours = info.zapret_dir.as_ref().is_some_and(|running| same_dir(running, dir));
    if is_ours {
        (info.strategy_id, matches!(info.start_type, StartType::Auto))
    } else {
        (None, false)
    }
}

async fn run_privileged(action: ZapretAction) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || privileged::run(&action))
        .await
        .map_err(|e| e.to_string())?
}

/// EN: Runs an action as administrator (via UAC) and returns the fresh state.
/// RU: Выполняет действие с правами администратора (через UAC) и возвращает свежее состояние.
async fn run_action(app: AppHandle, action: ZapretAction) -> Result<ZapretOverview, String> {
    tauri::async_runtime::spawn_blocking(move || privileged::run(&action))
        .await
        .map_err(|e| e.to_string())??;
    Ok(build_overview(&config::load(&app)))
}

#[tauri::command]
pub fn get_language() -> Lang {
    i18n::lang()
}

#[tauri::command]
pub fn set_language(app: AppHandle, language: Lang) -> Result<(), String> {
    i18n::set_lang(language);
    let mut config = config::load(&app);
    config.language = Some(language);
    config::save(&app, &config)
}

#[tauri::command]
pub fn get_zapret_overview(app: AppHandle) -> ZapretOverview {
    build_overview(&config::load(&app))
}

#[tauri::command]
pub async fn get_latest_release() -> Result<ReleaseInfo, String> {
    release::latest().await
}

/// EN: Downloads a release (the latest one, or a given version for testing updates) to
///     C:\ProgramData\NetPilot\zapret\<version> and remembers the folder.
/// RU: Скачивает релиз (последний или указанную версию — для проверки обновлений) в
///     C:\ProgramData\NetPilot\zapret\<версия> и запоминает папку.
#[tauri::command]
pub async fn download_zapret(app: AppHandle, version: Option<String>) -> Result<ZapretOverview, String> {
    let info = match version {
        Some(version) => release::by_version(version.trim()).await?,
        None => release::latest().await?,
    };
    let base = download_base_dir().ok_or_else(|| t("Не удалось найти папку ProgramData"))?;
    let staging = fsutil::staging_dir(&info.version)?;
    release::download(&app, &info, &staging).await?;

    let target = base.join(&info.version);
    let move_from = place_or_defer(&staging, &target)?;
    crate::info!("download_zapret: version={} target={} deferred_move={move_from:?}", info.version, target.display());
    if move_from.is_some() {
        // EN: The old folder of this version can't be deleted by the window process — it is replaced with
        //     administrator rights (the service is not reinstalled).
        // RU: Старую папку этой версии процесс окна удалить не может — её заменяет процесс с правами
        //     администратора (служба не переустанавливается).
        let mode = SwitchMode::UseDir { dir: target.clone(), move_from, remove: Vec::new() };
        run_privileged(ZapretAction::SwitchVersion { mode, strategy_id: None, autostart: false }).await?;
    }
    files::validate_dir(&target).map_err(|e| {
        tf(
            "Архив скачан, но файлы не прошли проверку: {error}. Возможно, их удалил антивирус",
            &[("error", &e)],
        )
    })?;
    update::save_original(&target, &info.version);

    let config = save_zapret_dir(&app, Some(target), true)?;
    Ok(build_overview(&config))
}

/// EN: Asks GitHub for the latest version and remembers it.
/// RU: Узнаёт на GitHub последнюю версию и запоминает её.
#[tauri::command]
pub async fn check_zapret_update(app: AppHandle) -> Result<ZapretOverview, String> {
    let latest = release::latest().await?;
    let mut config = config::load(&app);
    config.latest_version = Some(latest.version);
    config.update_checked_at = Some(now_secs());
    config::save(&app, &config)?;
    Ok(build_overview(&config))
}

/// EN: Updates zapret to the latest version, keeping the user's changes (see update.rs). The app folder
///     gets a new version folder next to the old one; a user folder is updated in place with a backup copy.
/// RU: Обновляет zapret до последней версии с переносом изменений пользователя (см. update.rs). Для папки
///     приложения рядом появляется папка новой версии; папка пользователя обновляется на месте с резервной копией.
#[tauri::command]
pub async fn zapret_update(app: AppHandle) -> Result<UpdateResult, String> {
    let mut config = config::load(&app);
    let current_dir = config.zapret_dir.clone().ok_or_else(config::no_zapret_dir)?;
    files::validate_dir(&current_dir)?;
    let current_version = files::read_version(&current_dir).ok_or_else(|| t("Не удалось определить версию zapret"))?;

    let latest = release::latest().await?;
    if update::compare_versions(&latest.version, &current_version) != std::cmp::Ordering::Greater {
        return Err(t("Установлена последняя версия"));
    }

    crate::info!(
        "zapret_update: {current_version} -> {} dir={} managed={}",
        latest.version,
        current_dir.display(),
        config.zapret_managed
    );
    let _ = app.emit(UPDATE_STAGE_EVENT, "original");
    let original = update::ensure_original(&app, &current_version).await?;
    crate::info!("zapret_update: original={}", original.display());

    let _ = app.emit(UPDATE_STAGE_EVENT, "download");
    let new_dir = fsutil::staging_dir(&latest.version)?;
    release::download(&app, &latest, &new_dir).await?;
    files::validate_dir(&new_dir).map_err(|e| {
        tf(
            "Архив скачан, но файлы не прошли проверку: {error}. Возможно, их удалил антивирус",
            &[("error", &e)],
        )
    })?;
    update::save_original(&new_dir, &latest.version);

    let _ = app.emit(UPDATE_STAGE_EVENT, "merge");
    let report = update::migrate(&current_dir, &original, &new_dir)?;

    let (strategy_id, autostart) = own_service_strategy(&current_dir);
    let strategy_missing = strategy_id.clone().filter(|id| !new_dir.join(format!("{id}.bat")).is_file());
    let strategy_id = strategy_id.filter(|_| strategy_missing.is_none());
    crate::info!(
        "zapret_update: report={report:?} strategy={strategy_id:?} missing={strategy_missing:?} autostart={autostart}"
    );

    let _ = app.emit(UPDATE_STAGE_EVENT, "apply");
    control::ensure_user_lists(&new_dir)?;
    if config.zapret_managed {
        let target = fsutil::managed_root()?.join(&latest.version);
        let move_from = place_or_defer(&new_dir, &target)?;
        let remove = cleanup_managed_versions(&[&target, &current_dir]);
        crate::info!(
            "zapret_update: target={} deferred_move={move_from:?} remove_as_admin={remove:?}",
            target.display()
        );
        if strategy_id.is_some() || move_from.is_some() || !remove.is_empty() {
            let mode = SwitchMode::UseDir { dir: target.clone(), move_from, remove };
            run_privileged(ZapretAction::SwitchVersion { mode, strategy_id, autostart }).await?;
        }
        config.rollback_dir = Some(current_dir.clone());
        config.zapret_dir = Some(target.clone());
    } else {
        let backup = fsutil::backup_dir()?;
        let mode = SwitchMode::ReplaceContents { target: current_dir.clone(), source: new_dir.clone(), backup: backup.clone() };
        run_privileged(ZapretAction::SwitchVersion { mode, strategy_id, autostart }).await?;
        config.rollback_dir = Some(backup);
    }
    config.latest_version = Some(latest.version.clone());
    config::save(&app, &config)?;
    update::cleanup_originals(&[&latest.version, &current_version]);

    Ok(UpdateResult { overview: build_overview(&config), version: latest.version, report, strategy_missing })
}

/// EN: Returns to the previous version and back (rollback is a swap of the two versions).
/// RU: Возвращает предыдущую версию и обратно (откат — это обмен двух версий местами).
#[tauri::command]
pub async fn zapret_rollback(app: AppHandle) -> Result<ZapretOverview, String> {
    let mut config = config::load(&app);
    let current_dir = config.zapret_dir.clone().ok_or_else(config::no_zapret_dir)?;
    let rollback_dir = config
        .rollback_dir
        .clone()
        .filter(|dir| dir.is_dir())
        .ok_or_else(|| t("Нет предыдущей версии для отката"))?;

    let (strategy_id, autostart) = own_service_strategy(&current_dir);
    let target_dir = if config.zapret_managed { rollback_dir.clone() } else { current_dir.clone() };
    let strategy_id = strategy_id.filter(|id| rollback_dir.join(format!("{id}.bat")).is_file());
    crate::info!(
        "zapret_rollback: current={} rollback={} managed={} strategy={strategy_id:?}",
        current_dir.display(),
        rollback_dir.display(),
        config.zapret_managed
    );

    if config.zapret_managed {
        if strategy_id.is_some() {
            control::ensure_user_lists(&target_dir)?;
            let mode = SwitchMode::UseDir { dir: target_dir.clone(), move_from: None, remove: Vec::new() };
            run_privileged(ZapretAction::SwitchVersion { mode, strategy_id, autostart }).await?;
        }
        config.zapret_dir = Some(rollback_dir);
        config.rollback_dir = Some(current_dir);
    } else {
        let mode = SwitchMode::ReplaceContents { target: current_dir, source: rollback_dir.clone(), backup: rollback_dir };
        run_privileged(ZapretAction::SwitchVersion { mode, strategy_id, autostart }).await?;
    }
    config::save(&app, &config)?;
    Ok(build_overview(&config))
}

/// EN: Moves a downloaded version from staging to `target`. If an old `target` can't be deleted (its
///     WinDivert64.sys is held by the driver), returns the staging path: the move is done with
///     administrator rights after the driver is stopped.
/// RU: Переносит скачанную версию из staging в `target`. Если старую `target` удалить нельзя (её
///     WinDivert64.sys держит драйвер), возвращает путь staging: перенос делается с правами
///     администратора после остановки драйвера.
fn place_or_defer(staging: &PathBuf, target: &PathBuf) -> Result<Option<PathBuf>, String> {
    if target.exists() && fs::remove_dir_all(target).is_err() {
        return Ok(Some(staging.clone()));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| tf("Не удалось создать папку: {error}", &[("error", &e)]))?;
    }
    fs::rename(staging, target).map_err(|e| tf("Не удалось создать папку: {error}", &[("error", &e)]))?;
    Ok(None)
}

/// EN: In the app folder only the current and the previous versions are kept. Returns folders the window
///     process couldn't delete — they are deleted with administrator rights.
/// RU: В папке приложения хранятся только текущая и предыдущая версии. Возвращает папки, которые процесс
///     окна удалить не смог, — их удаляет процесс с правами администратора.
fn cleanup_managed_versions(keep: &[&PathBuf]) -> Vec<PathBuf> {
    let Ok(root) = fsutil::managed_root() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && !keep.iter().any(|dir| same_dir(dir, path)))
        .filter(|path| fs::remove_dir_all(path).is_err())
        .collect()
}

/// EN: The user chose their own zapret folder.
/// RU: Пользователь указал свою папку zapret.
#[tauri::command]
pub fn set_zapret_dir(app: AppHandle, path: PathBuf) -> Result<ZapretOverview, String> {
    files::validate_dir(&path)?;
    let config = save_zapret_dir(&app, Some(path), false)?;
    Ok(build_overview(&config))
}

/// EN: Forget the folder: the first run screen is shown again. Files are not deleted.
/// RU: Забыть папку: снова будет экран первого запуска. Файлы не удаляются.
#[tauri::command]
pub fn forget_zapret_dir(app: AppHandle) -> Result<ZapretOverview, String> {
    let config = save_zapret_dir(&app, None, false)?;
    Ok(build_overview(&config))
}

/// EN: Installs (or reinstalls) the zapret service with the strategy and starts it. Autostart is not
///     turned on by itself: on a strategy change the user's choice is kept, on the first install it is off.
/// RU: Устанавливает (или переустанавливает) службу zapret со стратегией и запускает её. Автозапуск сам
///     не включается: при смене стратегии сохраняется выбор пользователя, при первой установке — выключен.
#[tauri::command]
pub async fn zapret_install(app: AppHandle, strategy_id: String) -> Result<ZapretOverview, String> {
    let zapret_dir = config::load(&app).zapret_dir.ok_or_else(config::no_zapret_dir)?;
    files::validate_dir(&zapret_dir)?;
    let autostart = matches!(service::query().start_type, StartType::Auto);
    crate::info!("zapret_install: dir={} strategy={strategy_id} autostart={autostart}", zapret_dir.display());
    control::ensure_user_lists(&zapret_dir)?;
    run_action(app, ZapretAction::Install { zapret_dir, strategy_id, autostart }).await
}

#[tauri::command]
pub async fn zapret_start(app: AppHandle) -> Result<ZapretOverview, String> {
    run_action(app, ZapretAction::Start).await
}

#[tauri::command]
pub async fn zapret_stop(app: AppHandle) -> Result<ZapretOverview, String> {
    run_action(app, ZapretAction::Stop).await
}

#[tauri::command]
pub async fn zapret_remove(app: AppHandle) -> Result<ZapretOverview, String> {
    run_action(app, ZapretAction::Remove).await
}

#[tauri::command]
pub fn get_autostart() -> AutostartStatus {
    autostart::status()
}

/// EN: Sets the autostart mode (None — off). RU: Устанавливает режим автозапуска (None — выключен).
#[tauri::command]
pub async fn set_autostart_mode(app: AppHandle, mode: Option<AutostartMode>) -> Result<AutostartStatus, String> {
    run_action(app.clone(), ZapretAction::SetAutostartMode { mode }).await?;
    if mode == Some(AutostartMode::App) {
        crate::geo::check_now(&app).await;
    }
    Ok(autostart::status())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppOptions {
    pub close_to_tray: bool,
    pub russia_only: bool,
}

#[tauri::command]
pub fn get_app_options(app: AppHandle) -> AppOptions {
    let config = config::load(&app);
    AppOptions { close_to_tray: config.close_to_tray, russia_only: config.russia_only }
}

#[tauri::command]
pub fn set_app_options(app: AppHandle, close_to_tray: bool, russia_only: bool) -> Result<AppOptions, String> {
    let mut config = config::load(&app);
    config.close_to_tray = close_to_tray;
    config.russia_only = russia_only;
    config::save(&app, &config)?;
    crate::info!("app options: close_to_tray={close_to_tray} russia_only={russia_only}");
    Ok(AppOptions { close_to_tray, russia_only })
}

/// EN: The last detected country, None — not checked yet. RU: Последняя определённая страна, None — ещё не проверялась.
#[tauri::command]
pub fn get_country(state: State<'_, CountryState>) -> Option<Country> {
    state.get()
}

/// EN: "Check now": whether the services open with the current bypass state. No admin rights needed.
/// RU: «Проверить сейчас»: открываются ли сервисы с текущим состоянием обхода. Права не нужны.
#[tauri::command]
pub async fn check_services(ids: Option<Vec<String>>) -> Result<Vec<ServiceCheck>, String> {
    let result = checks::run_only(ids.as_deref()).await;
    if let Ok(services) = &result {
        let summary: Vec<String> = services
            .iter()
            .map(|s| format!("{} {:?} {}/{} flaky={}", s.name, s.status, s.passed, s.total, s.flaky))
            .collect();
        crate::info!("check_services: {} | {}", summary.join(", "), service::describe_for_log());
    }
    result
}

/// EN: Path to the cancel flag of the running pick. None — no pick is running.
/// RU: Путь к флагу отмены идущего подбора. None — подбор не идёт.
#[derive(Default)]
pub struct PickState(Mutex<Option<PathBuf>>);

const PICK_PROGRESS_EVENT: &str = "zapret-pick-progress";
const PICK_POLL_INTERVAL: Duration = Duration::from_millis(300);

fn last_pick_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join("last_pick.json"))
}

/// EN: Result of the last pick, to show it after the app restarts.
/// RU: Результат последнего подбора, чтобы показать его после перезапуска приложения.
#[tauri::command]
pub fn get_last_pick(app: AppHandle) -> Option<PickResult> {
    let text = fs::read_to_string(last_pick_path(&app)?).ok()?;
    serde_json::from_str(&text).ok()
}

/// EN: Strategies to recommend first among equivalent ones: the current one (if the service runs from this
///     folder), then the previous recommendation (if the last pick was for this folder and version).
/// RU: Стратегии, которые рекомендуются в первую очередь среди равноценных: текущая (если служба работает
///     из этой папки), затем прошлая рекомендация (если последний подбор был для этой папки и версии).
fn preferred_strategies(app: &AppHandle, zapret_dir: &PathBuf) -> Vec<String> {
    let service = service::query();
    let current = service
        .zapret_dir
        .filter(|dir| same_dir(dir, zapret_dir))
        .and(service.strategy_id);
    let version = files::read_version(zapret_dir);
    let previous = get_last_pick(app.clone())
        .filter(|pick| pick.zapret_dir.as_ref().is_some_and(|dir| same_dir(dir, zapret_dir)) && pick.version == version)
        .and_then(|pick| pick.recommended);
    current.into_iter().chain(previous).collect()
}

/// EN: Quick pick: tries all strategies (one UAC prompt), sends progress via the zapret-pick-progress
///     event, saves and returns the result.
/// RU: Быстрый подбор: перебирает все стратегии (одно окно UAC), шлёт прогресс событием
///     zapret-pick-progress, сохраняет и возвращает результат.
#[tauri::command]
pub async fn zapret_quick_pick(app: AppHandle, state: State<'_, PickState>) -> Result<PickResult, String> {
    let zapret_dir = config::load(&app).zapret_dir.ok_or_else(config::no_zapret_dir)?;
    files::validate_dir(&zapret_dir)?;
    let strategy_ids: Vec<String> = files::list_strategies(&zapret_dir).into_iter().map(|s| s.id).collect();
    control::ensure_user_lists(&zapret_dir)?;
    let preferred = preferred_strategies(&app, &zapret_dir);

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let temp = std::env::temp_dir();
    let pick_files = PickFiles {
        progress: temp.join(format!("netpilot-{stamp}-pick-progress.json")),
        result: temp.join(format!("netpilot-{stamp}-pick-result.json")),
        cancel: temp.join(format!("netpilot-{stamp}-pick-cancel")),
    };
    {
        let mut cancel = state.0.lock().map_err(|e| e.to_string())?;
        if cancel.is_some() {
            return Err(t("Подбор уже идёт"));
        }
        *cancel = Some(pick_files.cancel.clone());
    }

    let done = Arc::new(AtomicBool::new(false));
    let done_in_task = done.clone();
    let action = ZapretAction::QuickPick { zapret_dir, strategy_ids, preferred, files: pick_files.clone() };
    let task = tauri::async_runtime::spawn_blocking(move || {
        let result = privileged::run(&action);
        done_in_task.store(true, Ordering::SeqCst);
        result
    });

    while !done.load(Ordering::SeqCst) {
        let progress = fs::read_to_string(&pick_files.progress)
            .ok()
            .and_then(|text| serde_json::from_str::<PickProgress>(&text).ok());
        if let Some(progress) = progress {
            let _ = app.emit(PICK_PROGRESS_EVENT, progress);
        }
        tokio::time::sleep(PICK_POLL_INTERVAL).await;
    }
    let run_result = task.await.map_err(|e| e.to_string());

    if let Ok(mut cancel) = state.0.lock() {
        *cancel = None;
    }
    let _ = fs::remove_file(&pick_files.progress);
    let _ = fs::remove_file(&pick_files.cancel);
    run_result??;

    let text = fs::read_to_string(&pick_files.result).map_err(|_| t("Подбор не вернул результат"))?;
    let _ = fs::remove_file(&pick_files.result);
    let result: PickResult = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if let Some(path) = last_pick_path(&app) {
        let _ = fs::write(path, &text);
    }
    Ok(result)
}

/// EN: "Stop": the pick stops after the current strategy and restores the service.
/// RU: «Прервать»: подбор остановится после текущей стратегии и вернёт службу как было.
#[tauri::command]
pub fn zapret_cancel_pick(state: State<'_, PickState>) {
    if let Ok(cancel) = state.0.lock() {
        if let Some(path) = cancel.as_ref() {
            let _ = fs::write(path, "");
        }
    }
}

#[tauri::command]
pub fn log_recording_status(state: State<'_, RecordingState>) -> RecordingStatus {
    recording::status(&state)
}

/// EN: "Start recording": debug lines are written from now on, a system snapshot is taken.
/// RU: «Начать запись»: с этого момента пишутся строки debug, снимается снимок системы.
#[tauri::command]
pub async fn log_recording_start(app: AppHandle) -> Result<RecordingStatus, String> {
    recording::start(app).await
}

/// EN: "Finish": takes the end snapshot and packs the archive.
/// RU: «Закончить»: снимает снимок в конце и собирает архив.
#[tauri::command]
pub async fn log_recording_stop(app: AppHandle) -> Result<RecordingResult, String> {
    recording::stop(app, StopReason::Manual).await
}

/// EN: A line from the window (user actions, errors shown on screen). Written only during a recording.
/// RU: Строка от окна (действия пользователя, ошибки на экране). Пишется только во время записи.
#[tauri::command]
pub fn log_ui(message: String) {
    crate::debug!("ui: {message}");
}

/// EN: YouTube video check in a separate player window; progress goes via the diag-video-progress event.
///     last_stage — the check is the last stage: after it the video may go on in a viewer window.
/// RU: Проверка видео YouTube в отдельном окне плеера; прогресс идёт событием diag-video-progress.
///     last_stage — проверка последний этап: после неё видео может продолжиться в окне просмотра.
#[tauri::command]
pub async fn diag_video_check(
    app: AppHandle,
    state: State<'_, DiagState>,
    scenario: Scenario,
    quality: Quality,
    quic: bool,
    last_stage: bool,
) -> Result<VideoCheckResult, String> {
    let ending = if last_stage { Ending::MayKeepWatching } else { Ending::Close };
    diag::run_video_check(&app, &state, scenario, quality, quic, ending).await
}

/// EN: Stops the video check or the deep pick. RU: Останавливает проверку видео или глубокий подбор.
#[tauri::command]
pub fn diag_video_cancel(state: State<'_, DiagState>) {
    diag::cancel(&state);
}

/// EN: The "keep watching after the check" option of the player window. RU: Галка «досмотреть после проверки» окна плеера.
#[tauri::command]
pub fn diag_keep_watching(state: State<'_, DiagState>, enabled: bool) {
    diag::set_keep_watching(&state, enabled);
}

/// EN: How many leaders of the quick pick the deep pick checks. RU: Сколько лидеров быстрого подбора проверяет глубокий подбор.
const DEEP_PICK_LEADERS: usize = 6;

fn last_deep_pick_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join("last_deep_pick.json"))
}

#[tauri::command]
pub fn get_last_deep_pick(app: AppHandle) -> Option<DeepPickResult> {
    let text = fs::read_to_string(last_deep_pick_path(&app)?).ok()?;
    serde_json::from_str(&text).ok()
}

/// EN: Deep pick: the video check with the leaders of the last quick pick and the current strategy.
///     Progress goes via diag-deep-progress, the check of each strategy via diag-video-progress.
/// RU: Глубокий подбор: проверка видео с лидерами последнего быстрого подбора и текущей стратегией.
///     Прогресс идёт событием diag-deep-progress, проверка каждой стратегии — diag-video-progress.
#[tauri::command]
pub async fn diag_deep_pick(
    app: AppHandle,
    state: State<'_, DiagState>,
    quality: Quality,
    all_strategies: bool,
) -> Result<DeepPickResult, String> {
    let zapret_dir = config::load(&app).zapret_dir.ok_or_else(config::no_zapret_dir)?;
    files::validate_dir(&zapret_dir)?;
    control::ensure_user_lists(&zapret_dir)?;
    let version = files::read_version(&zapret_dir);
    let pick = get_last_pick(app.clone())
        .filter(|pick| pick.zapret_dir.as_ref().is_some_and(|dir| same_dir(dir, &zapret_dir)) && pick.version == version);
    let service = service::query();
    let current = service.zapret_dir.filter(|dir| same_dir(dir, &zapret_dir)).and(service.strategy_id);
    let mut candidates: Vec<String> = match pick {
        Some(pick) if !all_strategies => pick
            .results
            .iter()
            .filter(|result| result.error.is_none() && (result.equivalent || result.passed == result.total))
            .take(DEEP_PICK_LEADERS)
            .map(|result| result.strategy_id.clone())
            .collect(),
        None if !all_strategies => {
            return Err(t("Сначала подберите стратегию на вкладке «Россия · zapret»: глубокий подбор проверяет её лидеров"));
        }
        _ => files::list_strategies(&zapret_dir).into_iter().map(|strategy| strategy.id).collect(),
    };
    if let Some(current) = &current
        && !candidates.contains(current)
    {
        candidates.push(current.clone());
    }
    if candidates.is_empty() {
        return Err(t("В быстром подборе нет стратегий, при которых открывается YouTube"));
    }
    let result = diag::deep::run(&app, &state, zapret_dir, candidates, current, quality).await?;
    if let (Some(path), Ok(text)) = (last_deep_pick_path(&app), serde_json::to_string(&result)) {
        let _ = fs::write(path, text);
    }
    Ok(result)
}

#[tauri::command]
pub fn get_browsers_quic() -> BrowsersQuic {
    browsers::quic_status()
}

/// EN: Allows or forbids QUIC in browsers (administrator rights). RU: Разрешает или запрещает QUIC в браузерах (права администратора).
#[tauri::command]
pub async fn set_browsers_quic(enabled: bool) -> Result<BrowsersQuic, String> {
    tauri::async_runtime::spawn_blocking(move || privileged::run(&ZapretAction::SetBrowserQuic { enabled }))
        .await
        .map_err(|e| e.to_string())??;
    Ok(browsers::quic_status())
}

fn services_dir(app: &AppHandle) -> Result<(PathBuf, AppConfig), String> {
    let config = config::load(app);
    let dir = config.zapret_dir.clone().ok_or_else(config::no_zapret_dir)?;
    files::validate_dir(&dir)?;
    control::ensure_user_lists(&dir)?;
    Ok((dir, config))
}

/// EN: zapret lists as services. Also brings the block of turned-off services up to date: after a zapret
///     update the author lists are new. RU: Списки zapret как сервисы. Заодно обновляет блок выключенных
///     сервисов: после обновления zapret списки авторов новые.
#[tauri::command]
pub fn get_zapret_services(app: AppHandle) -> Result<ServicesState, String> {
    let (dir, mut config) = services_dir(&app)?;
    zapret_services::sync_exclusions(&dir, &config.disabled_services)?;
    let current = zapret_services::author_domains(&dir);
    if current != config.known_author_domains {
        let fresh = zapret_services::new_domains(&config.known_author_domains, &current);
        if !fresh.is_empty() {
            crate::info!("services: new author domains {fresh:?}");
            config.fresh_domains = fresh;
        }
        config.known_author_domains = current;
        config::save(&app, &config)?;
    }
    Ok(services_state(&dir, &config))
}

fn services_state(dir: &std::path::Path, config: &AppConfig) -> ServicesState {
    let mut state = zapret_services::state(dir, &config.disabled_services);
    state.fresh = config.fresh_domains.clone();
    state
}

/// EN: "Got it": new author domains are no longer highlighted. RU: «Понятно»: новые домены авторов больше не подсвечиваются.
#[tauri::command]
pub fn dismiss_fresh_domains(app: AppHandle) -> Result<ServicesState, String> {
    let (dir, mut config) = services_dir(&app)?;
    config.fresh_domains.clear();
    config::save(&app, &config)?;
    Ok(services_state(&dir, &config))
}

/// EN: Turns a service on or off. winws re-reads the lists by itself, no restart is needed.
/// RU: Включает или выключает сервис. winws сам перечитывает списки, перезапуск не нужен.
#[tauri::command]
pub fn set_zapret_service(app: AppHandle, id: String, enabled: bool) -> Result<ServicesState, String> {
    let (dir, mut config) = services_dir(&app)?;
    config.disabled_services.retain(|disabled| *disabled != id);
    if !enabled {
        config.disabled_services.push(id.clone());
    }
    zapret_services::sync_exclusions(&dir, &config.disabled_services)?;
    config::save(&app, &config)?;
    crate::info!("services: {id} enabled={enabled}");
    Ok(services_state(&dir, &config))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServicesChange {
    pub state: ServicesState,
    pub added: Option<AddResult>,
}

#[tauri::command]
pub fn add_custom_domain(app: AppHandle, domain: String) -> Result<ServicesChange, String> {
    let (dir, config) = services_dir(&app)?;
    let added = zapret_services::add_custom(&dir, &domain)?;
    crate::info!("services: add custom {added:?}");
    Ok(ServicesChange { state: services_state(&dir, &config), added: Some(added) })
}

#[tauri::command]
pub fn remove_custom_domain(app: AppHandle, domain: String) -> Result<ServicesState, String> {
    let (dir, config) = services_dir(&app)?;
    zapret_services::remove_custom(&dir, &domain)?;
    crate::info!("services: remove custom {domain}");
    Ok(services_state(&dir, &config))
}

#[tauri::command]
pub fn add_user_exclusion(app: AppHandle, domain: String) -> Result<ServicesState, String> {
    let (dir, config) = services_dir(&app)?;
    let domain = zapret_services::add_exclusion(&dir, &domain)?;
    crate::info!("services: add exclusion {domain}");
    Ok(services_state(&dir, &config))
}

#[tauri::command]
pub fn remove_user_exclusion(app: AppHandle, domain: String) -> Result<ServicesState, String> {
    let (dir, config) = services_dir(&app)?;
    zapret_services::remove_exclusion(&dir, &domain)?;
    crate::info!("services: remove exclusion {domain}");
    Ok(services_state(&dir, &config))
}
