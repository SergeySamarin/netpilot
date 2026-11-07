//! EN: Translation of user-facing messages. The key is the Russian text, English lives in locales/en.json
//!     (the same file is used by the UI).
//! RU: Перевод сообщений для пользователя. Ключ — русский текст, английский — в locales/en.json
//!     (тот же файл использует интерфейс).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Ru,
    En,
}

static IS_EN: AtomicBool = AtomicBool::new(false);

static EN: LazyLock<HashMap<String, String>> =
    LazyLock::new(|| serde_json::from_str(include_str!("../../locales/en.json")).unwrap_or_default());

pub fn set_lang(lang: Lang) {
    IS_EN.store(lang == Lang::En, Ordering::SeqCst);
}

pub fn lang() -> Lang {
    if IS_EN.load(Ordering::SeqCst) {
        Lang::En
    } else {
        Lang::Ru
    }
}

/// EN: Russian system language -> ru, any other -> en.
/// RU: Русский язык системы -> ru, любой другой -> en.
pub fn system_lang() -> Lang {
    match sys_locale::get_locale() {
        Some(locale) if locale.to_lowercase().starts_with("ru") => Lang::Ru,
        _ => Lang::En,
    }
}

/// EN: Translates a message. Without a translation returns the Russian text.
/// RU: Переводит сообщение. Если перевода нет — возвращает русский текст.
pub fn t(key: &str) -> String {
    if lang() == Lang::En {
        if let Some(text) = EN.get(key) {
            return text.clone();
        }
    }
    key.to_string()
}

/// EN: Translates and substitutes parameters: `tf("Error: {error}", &[("error", &e)])`.
/// RU: Переводит и подставляет параметры: `tf("Ошибка: {error}", &[("error", &e)])`.
pub fn tf(key: &str, params: &[(&str, &dyn Display)]) -> String {
    params.iter().fold(t(key), |text, (name, value)| text.replace(&format!("{{{name}}}"), &value.to_string()))
}
