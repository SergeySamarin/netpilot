//! EN: zapret service management: install with a strategy, start, stop, remove.
//!     Everything here needs administrator rights — it is called from privileged.rs.
//! RU: Управление службой zapret: установка со стратегией, запуск, остановка, удаление.
//!     Всё здесь требует прав администратора — вызывается из privileged.rs.

use super::fsutil;
use crate::autostart::AutostartMode;
use super::host::{self, HostFiles};
use super::pick::{self, PickFiles};
use super::service::{self, ServiceState, SERVICE_NAME};
use super::strategy;
use crate::i18n::tf;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// EN: An action that runs with administrator rights.
/// RU: Действие, которое выполняется с правами администратора.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum ZapretAction {
    /// EN: Install the service with a strategy and start it. autostart — start when the PC turns on.
    /// RU: Установить службу со стратегией и запустить. autostart — запускать ли при включении ПК.
    Install { zapret_dir: PathBuf, strategy_id: String, autostart: bool },
    Start,
    Stop,
    Remove,
    /// EN: Autostart mode, None — off (see autostart.rs). RU: Режим автозапуска, None — выключен (см. autostart.rs).
    SetAutostartMode { mode: Option<AutostartMode> },
    /// EN: Quick pick: try the strategies and check the services (see pick.rs). preferred — strategies to
    ///     recommend first among equivalent ones: the current one and the previous recommendation.
    /// RU: Быстрый подбор: перебрать стратегии и проверить сервисы (см. pick.rs). preferred — стратегии,
    ///     которые рекомендуются в первую очередь среди равноценных: текущая и прошлая рекомендация.
    QuickPick {
        zapret_dir: PathBuf,
        strategy_ids: Vec<String>,
        #[serde(default)]
        preferred: Vec<String>,
        files: PickFiles,
    },
    /// EN: Switch zapret to another version (update or rollback) and reinstall the service with the same
    ///     strategy. strategy_id: None — the service is not reinstalled (not ours or the strategy is gone).
    /// RU: Переключить zapret на другую версию (обновление или откат) и переустановить службу с той же
    ///     стратегией. strategy_id: None — служба не переустанавливается (не наша или стратегии нет).
    SwitchVersion { mode: SwitchMode, strategy_id: Option<String>, autostart: bool },
    /// EN: Deep pick: hold strategies on the window's commands (see host.rs). parent_pid — the window
    ///     process: if it is gone, the host restores the service.
    /// RU: Глубокий подбор: держать стратегии по командам окна (см. host.rs). parent_pid — процесс окна:
    ///     если его не стало, хост возвращает службу.
    StrategyHost { zapret_dir: PathBuf, parent_pid: u32, files: HostFiles },
    /// EN: Allow or forbid QUIC in browsers via their policies (see browsers.rs).
    /// RU: Разрешить или запретить QUIC в браузерах через их политики (см. browsers.rs).
    SetBrowserQuic { enabled: bool },
    /// EN: NetPilot is being removed (see uninstall.rs). RU: NetPilot удаляется (см. uninstall.rs).
    Cleanup { remove_service: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SwitchMode {
    /// EN: Folder of the app: the version is in its own folder, the service is switched to it.
    ///     move_from — the downloaded version is still in staging: the old busy `dir` is replaced by it
    ///     after the service and the driver are stopped.
    /// RU: Папка приложения: версия лежит в своей папке, служба переключается на неё.
    ///     move_from — скачанная версия ещё в staging: ею заменяется старая занятая `dir`
    ///     после остановки службы и драйвера.
    ///     remove — old version folders the window process couldn't delete (their files belong to administrators).
    ///     remove — папки старых версий, которые не смог удалить процесс окна (файлы принадлежат администраторам).
    UseDir { dir: PathBuf, move_from: Option<PathBuf>, remove: Vec<PathBuf> },
    /// EN: User-chosen folder: its contents are replaced by `source`, the old contents go to `backup`.
    /// RU: Папка пользователя: содержимое заменяется на `source`, прежнее уходит в `backup`.
    ReplaceContents { target: PathBuf, source: PathBuf, backup: PathBuf },
}

pub fn execute(action: &ZapretAction) -> Result<(), String> {
    crate::info!("execute start | {}", service::describe_for_log());
    let result = execute_inner(action);
    match &result {
        Ok(()) => crate::info!("execute end: ok | {}", service::describe_for_log()),
        Err(e) => crate::error!("execute end: {e} | {}", service::describe_for_log()),
    }
    result
}

fn execute_inner(action: &ZapretAction) -> Result<(), String> {
    match action {
        ZapretAction::Install { zapret_dir, strategy_id, autostart } => install(zapret_dir, strategy_id, *autostart),
        ZapretAction::Start => win::start(),
        ZapretAction::Stop => win::stop(),
        ZapretAction::Remove => win::remove(),
        ZapretAction::SetAutostartMode { mode } => crate::autostart::set_mode(*mode),
        ZapretAction::QuickPick { zapret_dir, strategy_ids, preferred, files } => {
            pick::run(zapret_dir, strategy_ids, preferred, files)
        }
        ZapretAction::SwitchVersion { mode, strategy_id, autostart } => {
            switch_version(mode, strategy_id.as_deref(), *autostart)
        }
        ZapretAction::StrategyHost { zapret_dir, parent_pid, files } => host::run(zapret_dir, *parent_pid, files),
        ZapretAction::SetBrowserQuic { enabled } => crate::browsers::set_quic(*enabled),
        ZapretAction::Cleanup { remove_service } => crate::uninstall::cleanup(*remove_service),
    }
}

fn switch_version(mode: &SwitchMode, strategy_id: Option<&str>, autostart: bool) -> Result<(), String> {
    let state = service::query().state;
    let is_installed = !matches!(state, ServiceState::NotInstalled);
    let was_running = matches!(state, ServiceState::Running);
    let moves_files =
        matches!(mode, SwitchMode::ReplaceContents { .. } | SwitchMode::UseDir { move_from: Some(_), .. });
    crate::info!(
        "switch_version: installed={is_installed} was_running={was_running} moves_files={moves_files} strategy={strategy_id:?}"
    );

    // EN: winws.exe and the WinDivert driver hold the files — stop them before replacing files or reinstalling.
    // RU: winws.exe и драйвер WinDivert держат файлы — останавливаем их перед заменой файлов или переустановкой.
    if is_installed && (moves_files || strategy_id.is_some()) {
        win::stop()?;
    }
    if moves_files {
        win::reset_driver();
    }

    let dir = match mode {
        SwitchMode::UseDir { dir, move_from, .. } => {
            if let Some(from) = move_from {
                if dir.exists() {
                    fs::remove_dir_all(dir)
                        .map_err(|e| tf("Не удалось удалить {path}: {error}", &[("path", &dir.display()), ("error", &e)]))?;
                }
                fs::rename(from, dir).map_err(|e| tf("Не удалось создать папку: {error}", &[("error", &e)]))?;
            }
            dir.clone()
        }
        SwitchMode::ReplaceContents { target, source, backup } => {
            fsutil::replace_contents(target, source, backup)?;
            target.clone()
        }
    };

    match strategy_id {
        Some(id) if is_installed => {
            install(&dir, id, autostart)?;
            if !was_running {
                win::stop()?;
            }
        }
        // EN: The service stays as it was; start it again if it was running before.
        // RU: Служба остаётся как была; если работала — запускаем снова.
        _ if is_installed && was_running && moves_files => {
            win::start()?;
        }
        _ => {}
    }

    if let SwitchMode::UseDir { remove, .. } = mode {
        for old in remove {
            match fs::remove_dir_all(old) {
                Ok(()) => crate::info!("remove old version {}: ok", old.display()),
                Err(e) => crate::warn!("remove old version {}: {e}", old.display()),
            }
        }
    }
    Ok(())
}

/// EN: Resets the WinDivert driver service (see win::reset_driver).
/// RU: Сбрасывает службу драйвера WinDivert (см. win::reset_driver).
pub fn reset_driver() {
    win::reset_driver();
}

/// EN: Prepares the folder and the system before starting winws.exe: user lists and TCP timestamps.
/// RU: Подготовка папки и системы перед запуском winws.exe: списки пользователя и TCP timestamps.
pub fn prepare(zapret_dir: &Path) -> Result<(), String> {
    ensure_user_lists(zapret_dir)?;
    win::enable_tcp_timestamps();
    Ok(())
}

fn install(zapret_dir: &Path, strategy_id: &str, autostart: bool) -> Result<(), String> {
    prepare(zapret_dir)?;
    let args = strategy::build_args(zapret_dir, strategy_id)?;
    let exe = zapret_dir.join("bin").join("winws.exe");
    win::install(&exe, &args, strategy_id, autostart)
}

/// EN: Template contents of the user lists (the same as service.bat creates).
/// RU: Содержимое-заготовка пользовательских списков (такое же создаёт service.bat).
pub const USER_LIST_DEFAULTS: [(&str, &str); 3] = [
    ("ipset-exclude-user.txt", "203.0.113.113/32\r\n"),
    ("list-general-user.txt", "# Never leave this file empty\r\ndomain.example.abc\r\n"),
    ("list-exclude-user.txt", "domain.example.abc\r\n"),
];

/// EN: User lists that all strategies refer to. The release has none, and winws.exe does not start
///     without them. The window process creates them before an administrator action: files created by an
///     administrator process can't be deleted by the user later.
/// RU: Пользовательские списки, на которые ссылаются все стратегии. В релизе их нет, а без них
///     winws.exe не запустится. Процесс окна создаёт их до действия с правами администратора: файлы,
///     созданные процессом администратора, пользователь потом не сможет удалить.
pub fn ensure_user_lists(zapret_dir: &Path) -> Result<(), String> {
    let lists = zapret_dir.join("lists");
    for (name, content) in USER_LIST_DEFAULTS {
        let path = lists.join(name);
        if !path.exists() {
            fs::write(&path, content)
                .map_err(|e| tf("Не удалось создать {name}: {error}", &[("name", &name), ("error", &e)]))?;
        }
    }
    Ok(())
}

#[cfg(windows)]
mod win {
    use super::SERVICE_NAME;
    use crate::i18n::{t, tf};
    use std::ffi::OsString;
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::process::Command;
    use std::thread::sleep;
    use std::time::{Duration, Instant};
    use windows_service::service::{
        ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceState, ServiceType,
    };
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const ERROR_SERVICE_DOES_NOT_EXIST: i32 = 1060;
    const ERROR_SERVICE_NOT_ACTIVE: i32 = 1062;
    const ERROR_SERVICE_MARKED_FOR_DELETE: i32 = 1072;
    const WAIT_TIMEOUT: Duration = Duration::from_secs(10);
    const POLL_INTERVAL: Duration = Duration::from_millis(200);
    /// EN: How long to wait after the start to make sure winws.exe did not crash right away.
    /// RU: Сколько ждать после запуска, чтобы убедиться, что winws.exe не упал на старте.
    const STABLE_CHECK: Duration = Duration::from_millis(1500);

    /// EN: Driver service installed by winws.exe. Removed together with our service so nothing is left behind.
    /// RU: Служба драйвера, которую ставит winws.exe. Удаляем вместе с нашей службой, чтобы ничего не осталось.
    const DRIVER_SERVICES: [&str; 2] = ["WinDivert", "WinDivert14"];

    fn os_error(e: &windows_service::Error) -> Option<i32> {
        match e {
            windows_service::Error::Winapi(io) => io.raw_os_error(),
            _ => None,
        }
    }

    fn describe(e: windows_service::Error) -> String {
        match e {
            windows_service::Error::Winapi(io) => io.to_string(),
            other => other.to_string(),
        }
    }

    fn manager() -> Result<ServiceManager, String> {
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE)
            .map_err(|e| tf("Нет доступа к службам Windows: {error}", &[("error", &describe(e))]))
    }

    pub fn install(exe: &Path, args: &[String], strategy_id: &str, autostart: bool) -> Result<(), String> {
        crate::info!("install: exe={} strategy={strategy_id} autostart={autostart} args={}", exe.display(), args.len());
        crate::debug!("install args: {}", args.join(" "));
        let manager = manager()?;
        stop_and_delete(&manager, SERVICE_NAME)?;
        reset_driver_with(&manager);

        let info = ServiceInfo {
            name: OsString::from(SERVICE_NAME),
            display_name: OsString::from(SERVICE_NAME),
            service_type: ServiceType::OWN_PROCESS,
            start_type: if autostart { ServiceStartType::AutoStart } else { ServiceStartType::OnDemand },
            error_control: ServiceErrorControl::Normal,
            executable_path: exe.to_path_buf(),
            launch_arguments: args.iter().map(OsString::from).collect(),
            dependencies: vec![],
            account_name: None,
            account_password: None,
        };
        let service = manager
            .create_service(&info, ServiceAccess::START | ServiceAccess::QUERY_STATUS | ServiceAccess::CHANGE_CONFIG)
            .map_err(|e| tf("Не удалось создать службу: {error}", &[("error", &describe(e))]))?;
        let _ = service.set_description("Zapret DPI bypass software");
        save_strategy_name(strategy_id)?;

        service
            .start::<&str>(&[])
            .map_err(|e| tf("Служба создана, но не запустилась: {error}", &[("error", &describe(e))]))?;
        wait_running(&service)
    }

    pub fn start() -> Result<(), String> {
        let manager = manager()?;
        let service = manager
            .open_service(SERVICE_NAME, ServiceAccess::START | ServiceAccess::QUERY_STATUS)
            .map_err(|e| tf("Не удалось открыть службу: {error}", &[("error", &describe(e))]))?;
        service
            .start::<&str>(&[])
            .map_err(|e| tf("Не удалось запустить службу: {error}", &[("error", &describe(e))]))?;
        wait_running(&service)
    }

    pub fn stop() -> Result<(), String> {
        let manager = manager()?;
        let service = manager
            .open_service(SERVICE_NAME, ServiceAccess::STOP | ServiceAccess::QUERY_STATUS)
            .map_err(|e| tf("Не удалось открыть службу: {error}", &[("error", &describe(e))]))?;
        stop_service(&service)
    }

    pub fn remove() -> Result<(), String> {
        let manager = manager()?;
        stop_and_delete(&manager, SERVICE_NAME)?;
        reset_driver_with(&manager);
        Ok(())
    }

    /// EN: Stops and deletes the WinDivert driver service. It remembers the path of the first loaded
    ///     WinDivert64.sys and keeps that file locked; winws.exe then reuses the old path even from
    ///     another zapret folder. After a reset winws.exe creates the driver again from its own folder.
    /// RU: Останавливает и удаляет службу драйвера WinDivert. Она запоминает путь к первому загруженному
    ///     WinDivert64.sys и держит этот файл; winws.exe потом берёт старый путь даже из другой папки zapret.
    ///     После сброса winws.exe создаёт драйвер заново из своей папки.
    pub fn reset_driver() {
        if let Ok(manager) = manager() {
            reset_driver_with(&manager);
        }
    }

    fn reset_driver_with(manager: &ServiceManager) {
        for driver in DRIVER_SERVICES {
            // EN: The driver may be held by another bypass program — it doesn't block our actions.
            // RU: Драйвер может быть занят другой программой обхода — это не мешает нашим действиям.
            let result = stop_and_delete(manager, driver);
            crate::info!("reset driver {driver}: {result:?}");
        }
    }

    /// EN: Waits until the service starts and keeps running for a moment: winws.exe with wrong
    ///     arguments starts and exits right away.
    /// RU: Ждёт, пока служба запустится и немного проработает: winws.exe с неверными аргументами
    ///     запускается и сразу завершается.
    fn wait_running(service: &windows_service::service::Service) -> Result<(), String> {
        let deadline = Instant::now() + WAIT_TIMEOUT;
        loop {
            let status = service.query_status().map_err(describe)?;
            match status.current_state {
                ServiceState::Running => break,
                ServiceState::Stopped => {
                    return Err(tf(
                        "Служба запустилась и сразу остановилась (код {code}). Возможно, стратегия не подходит или антивирус блокирует winws.exe",
                        &[("code", &format!("{:?}", status.exit_code))],
                    ));
                }
                _ if Instant::now() > deadline => return Err(t("Служба слишком долго запускается")),
                _ => sleep(POLL_INTERVAL),
            }
        }
        sleep(STABLE_CHECK);
        let status = service.query_status().map_err(describe)?;
        if status.current_state != ServiceState::Running {
            return Err(t("Служба запустилась, но через секунду остановилась. Проверьте стратегию или антивирус"));
        }
        Ok(())
    }

    fn stop_service(service: &windows_service::service::Service) -> Result<(), String> {
        match service.stop() {
            Ok(_) => {}
            Err(e) if os_error(&e) == Some(ERROR_SERVICE_NOT_ACTIVE) => return Ok(()),
            Err(e) => return Err(tf("Не удалось остановить службу: {error}", &[("error", &describe(e))])),
        }
        let deadline = Instant::now() + WAIT_TIMEOUT;
        while Instant::now() < deadline {
            if service.query_status().map_err(describe)?.current_state == ServiceState::Stopped {
                return Ok(());
            }
            sleep(POLL_INTERVAL);
        }
        Err(t("Служба не остановилась за 10 секунд"))
    }

    /// EN: Stops and deletes a service. Does nothing if it does not exist.
    /// RU: Останавливает и удаляет службу. Если её нет — ничего не делает.
    fn stop_and_delete(manager: &ServiceManager, name: &str) -> Result<(), String> {
        let access = ServiceAccess::STOP | ServiceAccess::QUERY_STATUS | ServiceAccess::DELETE;
        let service = match manager.open_service(name, access) {
            Ok(service) => service,
            Err(e) if os_error(&e) == Some(ERROR_SERVICE_DOES_NOT_EXIST) => return Ok(()),
            Err(e) => {
                return Err(tf("Не удалось открыть службу {name}: {error}", &[("name", &name), ("error", &describe(e))]));
            }
        };
        stop_service(&service)?;
        match service.delete() {
            Ok(()) => {}
            Err(e) if os_error(&e) == Some(ERROR_SERVICE_MARKED_FOR_DELETE) => {}
            Err(e) => {
                return Err(tf("Не удалось удалить службу {name}: {error}", &[("name", &name), ("error", &describe(e))]));
            }
        }
        drop(service);

        // EN: Windows deletes the service when all its handles are closed — wait for it.
        // RU: Windows удаляет службу, когда закрыты все её дескрипторы — ждём.
        let deadline = Instant::now() + WAIT_TIMEOUT;
        while Instant::now() < deadline {
            match manager.open_service(name, ServiceAccess::QUERY_STATUS) {
                Err(e) if os_error(&e) == Some(ERROR_SERVICE_DOES_NOT_EXIST) => return Ok(()),
                _ => sleep(POLL_INTERVAL),
            }
        }
        Err(tf(
            "Служба {name} помечена на удаление, но ещё не удалена. Закройте окно «Службы» (services.msc), если оно открыто, и повторите",
            &[("name", &name)],
        ))
    }

    /// EN: Strategy name in the same registry value service.bat uses, so both show the current strategy.
    /// RU: Название стратегии в том же параметре реестра, что у service.bat, — оба видят текущую стратегию.
    fn save_strategy_name(strategy_id: &str) -> Result<(), String> {
        use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_SET_VALUE};
        use winreg::RegKey;

        let path = format!("SYSTEM\\CurrentControlSet\\Services\\{SERVICE_NAME}");
        RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(path, KEY_SET_VALUE)
            .and_then(|key| key.set_value("zapret-discord-youtube", &strategy_id))
            .map_err(|e| tf("Не удалось записать стратегию в реестр: {error}", &[("error", &e)]))
    }

    /// EN: Strategies with fooling=ts don't work without TCP timestamps.
    /// RU: Стратегии с fooling=ts не работают без TCP timestamps.
    pub fn enable_tcp_timestamps() {
        let _ = Command::new("netsh")
            .args(["interface", "tcp", "set", "global", "timestamps=enabled"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
}

#[cfg(not(windows))]
mod win {
    use crate::i18n::t;
    use std::path::Path;

    fn unsupported() -> String {
        t("Управление службой поддерживается только в Windows")
    }

    pub fn install(_exe: &Path, _args: &[String], _strategy_id: &str, _autostart: bool) -> Result<(), String> {
        Err(unsupported())
    }
    pub fn start() -> Result<(), String> {
        Err(unsupported())
    }
    pub fn stop() -> Result<(), String> {
        Err(unsupported())
    }
    pub fn remove() -> Result<(), String> {
        Err(unsupported())
    }
    pub fn reset_driver() {}
    pub fn enable_tcp_timestamps() {}
}
