//! EN: Tray icon: the bypass state, turning it on and off, opening the window and quitting. The window close
//!     button hides the window to the tray (if the setting is on), so the app keeps working in the background.
//! RU: Значок в трее: состояние обхода, включение и выключение, открыть окно и выход. Крестик окна
//!     сворачивает окно в трей (если включена настройка), и приложение продолжает работать в фоне.

use crate::engines::zapret::control::ZapretAction;
use crate::engines::zapret::service::{self, ServiceState};
use crate::geo::{CountryState, RUSSIA};
use crate::i18n::{t, tf};
use crate::privileged;
use std::time::Duration;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

/// EN: The service state changed outside the window (tray, country) — the window re-reads it.
/// RU: Состояние службы изменилось не из окна (трей, страна) — окно перечитывает его.
pub const STATE_EVENT: &str = "zapret-state-changed";
const MAIN_WINDOW: &str = "main";
const REFRESH_INTERVAL: Duration = Duration::from_secs(3);

pub fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

struct TrayItems {
    status: MenuItem<Wry>,
    bypass: CheckMenuItem<Wry>,
    open: MenuItem<Wry>,
    quit: MenuItem<Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let items = TrayItems {
        status: MenuItem::with_id(app, "status", "NetPilot", false, None::<&str>)?,
        bypass: CheckMenuItem::with_id(app, "bypass", t("Обход zapret"), true, false, None::<&str>)?,
        open: MenuItem::with_id(app, "open", t("Открыть окно"), true, None::<&str>)?,
        quit: MenuItem::with_id(app, "quit", t("Выход"), true, None::<&str>)?,
    };
    let menu = Menu::with_items(
        app,
        &[
            &items.status,
            &PredefinedMenuItem::separator(app)?,
            &items.bypass,
            &PredefinedMenuItem::separator(app)?,
            &items.open,
            &items.quit,
        ],
    )?;
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("NetPilot")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_window(app),
            "quit" => app.exit(0),
            "bypass" => toggle_bypass(app.clone()),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let tray = builder.build(app)?;

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            refresh(&app, &items, &tray);
            tokio::time::sleep(REFRESH_INTERVAL).await;
        }
    });
    Ok(())
}

/// EN: Texts follow the language and the service state. RU: Тексты следуют за языком и состоянием службы.
fn refresh(app: &AppHandle, items: &TrayItems, tray: &tauri::tray::TrayIcon) {
    let state = service::query().state;
    let running = matches!(state, ServiceState::Running);
    let foreign = app
        .try_state::<CountryState>()
        .and_then(|country| country.get())
        .is_some_and(|country| country.code != RUSSIA);
    let status = match state {
        ServiceState::Running => t("обход работает"),
        ServiceState::NotInstalled => t("служба не установлена"),
        _ if foreign => t("не в России, обход выключен"),
        _ => t("обход выключен"),
    };
    let title = tf("NetPilot — {status}", &[("status", &status)]);
    let _ = items.status.set_text(&title);
    let _ = items.bypass.set_text(t("Обход zapret"));
    let _ = items.bypass.set_checked(running);
    let _ = items.bypass.set_enabled(!matches!(state, ServiceState::NotInstalled));
    let _ = items.open.set_text(t("Открыть окно"));
    let _ = items.quit.set_text(t("Выход"));
    let _ = tray.set_tooltip(Some(&title));
}

fn toggle_bypass(app: AppHandle) {
    let action = if matches!(service::query().state, ServiceState::Running) {
        ZapretAction::Stop
    } else {
        ZapretAction::Start
    };
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = privileged::run(&action) {
            crate::error!("tray: {action:?} failed: {e}");
        }
        let _ = app.emit(STATE_EVENT, ());
    });
}
