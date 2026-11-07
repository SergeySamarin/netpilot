//! EN: Reading the zapret folder: validation, version, list of strategies.
//! RU: Чтение папки zapret: проверка, версия, список стратегий.

use crate::i18n::{t, tf};
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Strategy {
    /// EN: File name without .bat, e.g. "general (ALT2)". The same name service.bat writes to the registry.
    /// RU: Имя файла без .bat, например "general (ALT2)". Совпадает с тем, что service.bat пишет в реестр.
    pub id: String,
    /// EN: Short name for the UI: "ALT2", for general.bat — "general".
    /// RU: Короткое имя для интерфейса: "ALT2", для general.bat — "general".
    pub title: String,
}

/// EN: Checks that the folder contains zapret. Returns an error message for the user.
/// RU: Проверяет, что в папке лежит zapret. Возвращает текст ошибки для пользователя.
pub fn validate_dir(dir: &Path) -> Result<(), String> {
    if !dir.is_dir() {
        return Err(tf("Папка не найдена: {path}", &[("path", &dir.display())]));
    }
    if !dir.join("bin").join("winws.exe").is_file() {
        return Err(t("В папке нет bin\\winws.exe — это не папка zapret"));
    }
    if list_strategies(dir).is_empty() {
        return Err(t("В папке нет файлов стратегий general*.bat"));
    }
    Ok(())
}

/// EN: Version from the `set "LOCAL_VERSION=1.10.4"` line in service.bat.
/// RU: Версия из строки `set "LOCAL_VERSION=1.10.4"` в service.bat.
pub fn read_version(dir: &Path) -> Option<String> {
    let bytes = fs::read(dir.join("service.bat")).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    text.lines().find_map(|line| {
        let rest = line.split("LOCAL_VERSION=").nth(1)?;
        let version = rest.trim_end_matches(['"', ' ', '\r']).trim();
        (!version.is_empty()).then(|| version.to_string())
    })
}

/// EN: Strategies from general*.bat files, sorted with numbers in mind: ALT2 before ALT10.
/// RU: Стратегии из файлов general*.bat, отсортированные с учётом чисел: ALT2 раньше ALT10.
pub fn list_strategies(dir: &Path) -> Vec<Strategy> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut result: Vec<Strategy> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            if !lower.starts_with("general") || !lower.ends_with(".bat") {
                return None;
            }
            let id = name[..name.len() - ".bat".len()].to_string();
            Some(Strategy { title: strategy_title(&id), id })
        })
        .collect();
    result.sort_by_key(|s| natural_key(&s.id));
    result
}

/// EN: "general (ALT2)" -> "ALT2", "general" -> "general".
/// RU: "general (ALT2)" -> "ALT2", "general" -> "general".
pub fn strategy_title(id: &str) -> String {
    match (id.find('('), id.rfind(')')) {
        (Some(start), Some(end)) if end > start => id[start + 1..end].to_string(),
        _ => id.to_string(),
    }
}

/// EN: Sort key: numbers are padded with zeros on the left, so ALT2 goes before ALT10.
/// RU: Ключ сортировки: числа дополняются нулями слева, чтобы ALT2 шла раньше ALT10.
fn natural_key(text: &str) -> String {
    let mut key = String::new();
    let mut number = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            number.push(ch);
            continue;
        }
        if !number.is_empty() {
            key.push_str(&format!("{number:0>8}"));
            number.clear();
        }
        key.push(ch);
    }
    if !number.is_empty() {
        key.push_str(&format!("{number:0>8}"));
    }
    key.to_lowercase()
}
