//! EN: System snapshot for a log recording: app and Windows versions, settings, zapret service and driver,
//!     user lists, proxy, network adapters and DNS, a quick service check. Without passwords: proxy
//!     credentials are masked.
//! RU: Снимок системы для записи журнала: версии приложения и Windows, настройки, служба и драйвер zapret,
//!     пользовательские списки, прокси, сетевые адаптеры и DNS, быстрая проверка сервисов. Без паролей:
//!     логин и пароль прокси скрываются.

use crate::checks::{self, ServiceCheck};
use crate::config;
use crate::engines::zapret::{files, service};
use std::fmt::Write;
use std::fs;
use std::path::Path;
use tauri::AppHandle;

/// EN: Collects the snapshot as text. title — "start" or "end" of the recording.
/// RU: Собирает снимок текстом. title — «начало» или «конец» записи.
pub async fn collect(app: &AppHandle, title: &str) -> String {
    let mut text = String::new();
    let _ = writeln!(text, "===== {title} · {} =====", local_time_text());
    let _ = writeln!(text, "NetPilot {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(text, "Windows: {}", windows_version());
    let _ = writeln!(text, "UI language: {:?}", crate::i18n::lang());

    let config = config::load(app);
    let _ = writeln!(text, "\n[settings]\n{}", serde_json::to_string_pretty(&config).unwrap_or_default());

    let _ = writeln!(text, "\n[zapret]");
    if let Some(dir) = &config.zapret_dir {
        let _ = writeln!(text, "version: {:?}", files::read_version(dir));
        let _ = writeln!(text, "strategies: {}", files::list_strategies(dir).len());
        let _ = writeln!(text, "lists:");
        text.push_str(&describe_lists(&dir.join("lists")));
    }
    let _ = writeln!(text, "service: {}", service::describe_for_log());
    let _ = writeln!(text, "winws.exe running: {}", service::is_winws_running());

    let _ = writeln!(text, "\n[proxy]");
    text.push_str(&describe_proxy());

    let _ = writeln!(text, "\n[network]");
    let network = tauri::async_runtime::spawn_blocking(describe_network).await.unwrap_or_default();
    text.push_str(&network);

    let _ = writeln!(text, "\n[service check]");
    match checks::run().await {
        Ok(services) => text.push_str(&describe_checks(&services)),
        Err(e) => {
            let _ = writeln!(text, "failed: {e}");
        }
    }
    text
}

/// EN: Line count of each list file: enough to see empty or broken lists without copying the domains.
/// RU: Число строк каждого файла списка: достаточно, чтобы увидеть пустые или сломанные списки, не копируя домены.
fn describe_lists(dir: &Path) -> String {
    let mut text = String::new();
    let Ok(entries) = fs::read_dir(dir) else {
        let _ = writeln!(text, "  (no folder {})", dir.display());
        return text;
    };
    let mut names: Vec<String> = entries.flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    for name in names {
        let lines = fs::read_to_string(dir.join(&name)).map(|content| content.lines().count()).unwrap_or(0);
        let _ = writeln!(text, "  {name}: {lines} lines");
    }
    text
}

fn describe_checks(services: &[ServiceCheck]) -> String {
    let mut text = String::new();
    for service in services {
        let _ = writeln!(
            text,
            "{} {:?} {}/{} flaky={} latency={:?}",
            service.name, service.status, service.passed, service.total, service.flaky, service.latency_ms
        );
        for target in &service.targets {
            let _ = writeln!(
                text,
                "  {} tls1.2={:?} tls1.3={:?} flaky={} error={:?}",
                target.host, target.tls12_ms, target.tls13_ms, target.flaky, target.error
            );
        }
    }
    text
}

/// EN: Hides login and password in a proxy address: http://user:pass@host:port → http://***@host:port.
/// RU: Скрывает логин и пароль в адресе прокси: http://user:pass@host:port → http://***@host:port.
fn mask_credentials(address: &str) -> String {
    let Some(at) = address.rfind('@') else {
        return address.to_string();
    };
    let start = address.find("://").map(|index| index + 3).unwrap_or(0);
    if start > at {
        return address.to_string();
    }
    format!("{}***{}", &address[..start], &address[at..])
}

fn describe_proxy() -> String {
    let mut text = String::new();
    for name in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "NO_PROXY"] {
        if let Ok(value) = std::env::var(name) {
            let _ = writeln!(text, "{name}={}", mask_credentials(&value));
        }
    }
    text.push_str(&system_proxy());
    text
}

#[cfg(windows)]
fn system_proxy() -> String {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let Ok(key) = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
    else {
        return String::new();
    };
    let enabled: u32 = key.get_value("ProxyEnable").unwrap_or(0);
    let server: String = key.get_value("ProxyServer").unwrap_or_default();
    let auto_config: String = key.get_value("AutoConfigURL").unwrap_or_default();
    format!(
        "system proxy: enabled={} server={} pac={}\n",
        enabled == 1,
        mask_credentials(&server),
        mask_credentials(&auto_config)
    )
}

#[cfg(not(windows))]
fn system_proxy() -> String {
    String::new()
}

#[cfg(windows)]
fn windows_version() -> String {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion")
    else {
        return String::from("?");
    };
    let product: String = key.get_value("ProductName").unwrap_or_default();
    let display: String = key.get_value("DisplayVersion").unwrap_or_default();
    let build: String = key.get_value("CurrentBuild").unwrap_or_default();
    // EN: ProductName says "Windows 10" on Windows 11 too — the build tells them apart (22000 and above is 11).
    // RU: ProductName и в Windows 11 пишет «Windows 10» — различить можно по сборке (22000 и выше — 11).
    format!("{product} {display} build {build}")
}

#[cfg(not(windows))]
fn windows_version() -> String {
    String::from("-")
}

/// EN: Active adapters (VPN adapters are visible here too) and DNS servers, via PowerShell.
/// RU: Активные адаптеры (здесь же видны VPN-адаптеры) и DNS-серверы, через PowerShell.
#[cfg(windows)]
fn describe_network() -> String {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const SCRIPT: &str = "[Console]::OutputEncoding = [Text.Encoding]::UTF8; \
        $up = Get-NetAdapter | Where-Object Status -eq 'Up'; \
        $up | ForEach-Object { \"adapter: $($_.Name) | $($_.InterfaceDescription)\" }; \
        Get-DnsClientServerAddress | Where-Object { $_.ServerAddresses -and ($up.Name -contains $_.InterfaceAlias) } | \
        ForEach-Object { \"dns $($_.InterfaceAlias) $(if ($_.AddressFamily -eq 2) { 'IPv4' } else { 'IPv6' }): $($_.ServerAddresses -join ', ')\" }";

    match std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        Ok(output) => String::from_utf8_lossy(&output.stdout).into_owned(),
        Err(e) => format!("failed: {e}\n"),
    }
}

#[cfg(not(windows))]
fn describe_network() -> String {
    String::new()
}

/// EN: Local date and time: in the archive name and the snapshot header.
/// RU: Местные дата и время: в имени архива и в заголовке снимка.
#[cfg(windows)]
pub fn local_time() -> (u16, u16, u16, u16, u16, u16) {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    // EN: SAFETY: GetLocalTime only fills the passed struct.
    // RU: SAFETY: GetLocalTime только заполняет переданную структуру.
    let time = unsafe {
        let mut time = std::mem::zeroed();
        GetLocalTime(&mut time);
        time
    };
    (time.wYear, time.wMonth, time.wDay, time.wHour, time.wMinute, time.wSecond)
}

#[cfg(not(windows))]
pub fn local_time() -> (u16, u16, u16, u16, u16, u16) {
    (0, 0, 0, 0, 0, 0)
}

fn local_time_text() -> String {
    let (year, month, day, hour, minute, second) = local_time();
    format!("{year}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} (local)")
}

#[cfg(test)]
mod tests {
    use super::mask_credentials;

    #[test]
    fn masks_proxy_credentials() {
        assert_eq!(mask_credentials("http://user:pass@1.2.3.4:3128"), "http://***@1.2.3.4:3128");
        assert_eq!(mask_credentials("user:p@ss@host:80"), "***@host:80");
        assert_eq!(mask_credentials("http://host:80"), "http://host:80");
        assert_eq!(mask_credentials(""), "");
    }
}
