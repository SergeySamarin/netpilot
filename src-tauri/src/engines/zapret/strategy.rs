//! EN: Parsing a general*.bat strategy file into winws.exe arguments.
//! RU: Разбор файла стратегии general*.bat в аргументы winws.exe.

use crate::i18n::tf;
use std::fs;
use std::path::Path;

/// EN: Port placeholder used when the game filter is off.
/// RU: Заглушка вместо портов, когда игровой фильтр выключен.
const GAME_FILTER_OFF: &str = "12";
const GAME_FILTER_DEFAULT_RANGE: &str = "1024-65535";

/// EN: winws.exe arguments for the `strategy_id` strategy (bat file name without extension).
/// RU: Аргументы winws.exe для стратегии `strategy_id` (имя bat-файла без расширения).
pub fn build_args(zapret_dir: &Path, strategy_id: &str) -> Result<Vec<String>, String> {
    let bat_path = zapret_dir.join(format!("{strategy_id}.bat"));
    let bytes = fs::read(&bat_path).map_err(|e| tf("Не удалось прочитать {path}: {error}", &[("path", &bat_path.display()), ("error", &e)]))?;
    let text = String::from_utf8_lossy(&bytes);

    let marker = "winws.exe\"";
    let start = text
        .find(marker)
        .ok_or_else(|| tf("В {name}.bat не найден запуск winws.exe", &[("name", &strategy_id)]))?
        + marker.len();
    let command = unescape_cmd_line(&text[start..]);

    let bin = format!("{}\\", zapret_dir.join("bin").display());
    let lists = format!("{}\\", zapret_dir.join("lists").display());
    let (game_tcp, game_udp) = game_filter(zapret_dir);
    let command = command
        .replace("%BIN%", &bin)
        .replace("%LISTS%", &lists)
        .replace("%GameFilterTCP%", &game_tcp)
        .replace("%GameFilterUDP%", &game_udp)
        .replace("%GameFilter%", &game_tcp);

    if let Some(pos) = command.find('%') {
        let rest: String = command[pos..].chars().take(40).collect();
        return Err(tf("В стратегии осталась неизвестная переменная: {rest}…", &[("rest", &rest)]));
    }

    let args = split_args(&command);
    if args.is_empty() {
        return Err(tf("В {name}.bat нет аргументов winws.exe", &[("name", &strategy_id)]));
    }
    Ok(args)
}

/// EN: Takes the command to its end (with ^ line continuations) and removes cmd escaping:
///     outside quotes `^x` becomes `x`.
/// RU: Берёт команду до конца (с учётом переноса строк через ^) и убирает экранирование cmd:
///     вне кавычек `^x` превращается в `x`.
fn unescape_cmd_line(text: &str) -> String {
    let mut result = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                result.push(ch);
            }
            '^' if !in_quotes => match chars.next() {
                // EN: ^ at the end of a line — the command continues on the next line.
                // RU: ^ в конце строки — продолжение команды на следующей строке.
                Some('\r') => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    result.push(' ');
                }
                Some('\n') => result.push(' '),
                Some(next) => result.push(next),
                None => {}
            },
            '\r' | '\n' => break,
            _ => result.push(ch),
        }
    }
    result
}

/// EN: Splits a command line into arguments: spaces outside quotes separate, quotes are removed.
/// RU: Делит командную строку на аргументы: пробелы вне кавычек разделяют, кавычки убираются.
fn split_args(command: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut has_arg = false;
    for ch in command.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                has_arg = true;
            }
            ' ' | '\t' if !in_quotes => {
                if has_arg {
                    args.push(std::mem::take(&mut current));
                    has_arg = false;
                }
            }
            _ => {
                current.push(ch);
                has_arg = true;
            }
        }
    }
    if has_arg {
        args.push(current);
    }
    args
}

/// EN: Game filter ports from utils\game_filter.enabled (written by the service.bat menu: mode=, tcp=, udp=).
///     No file — the filter is off.
/// RU: Порты игрового фильтра из utils\game_filter.enabled (пишет меню service.bat: mode=, tcp=, udp=).
///     Файла нет — фильтр выключен.
fn game_filter(zapret_dir: &Path) -> (String, String) {
    let off = (GAME_FILTER_OFF.to_string(), GAME_FILTER_OFF.to_string());
    let Ok(text) = fs::read_to_string(zapret_dir.join("utils").join("game_filter.enabled")) else {
        return off;
    };

    let mut mode = String::from("disabled");
    let mut tcp = GAME_FILTER_DEFAULT_RANGE.to_string();
    let mut udp = GAME_FILTER_DEFAULT_RANGE.to_string();
    for line in text.lines() {
        let (key, value) = line.split_once('=').unwrap_or((line, ""));
        let value = value.trim();
        match key.trim().to_lowercase().as_str() {
            "mode" => mode = value.to_lowercase(),
            "all" => mode = "all".into(),
            "tcp" if value.is_empty() => mode = "tcp".into(),
            "udp" if value.is_empty() => mode = "udp".into(),
            "tcp" => tcp = value.to_string(),
            "udp" => udp = value.to_string(),
            _ => {}
        }
    }

    match mode.as_str() {
        "all" => (tcp, udp),
        "tcp" => (tcp, GAME_FILTER_OFF.to_string()),
        "udp" => (GAME_FILTER_OFF.to_string(), udp),
        _ => off,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescape_joins_lines_and_keeps_quotes() {
        let text = " --a=1 ^\r\n--b=\"x y\" --c=^! ^\r\n--d\r\nnext line";
        assert_eq!(unescape_cmd_line(text), " --a=1  --b=\"x y\" --c=!  --d");
    }

    /// EN: Manual check on a real folder. RU: Ручная проверка на настоящей папке:
    /// `ZAPRET_TEST_DIR=C:\ProgramData\NetPilot\zapret\1.10.4 cargo test print_real_strategy -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn print_real_strategy() {
        let dir = std::env::var("ZAPRET_TEST_DIR").expect("ZAPRET_TEST_DIR");
        let id = std::env::var("ZAPRET_TEST_STRATEGY").unwrap_or_else(|_| "general (SIMPLE FAKE)".into());
        for arg in build_args(Path::new(&dir), &id).expect("build_args") {
            println!("{arg}");
        }
    }

    #[test]
    fn split_removes_quotes() {
        let args = split_args(" --a=1 --b=\"C:\\x y\\f.txt\"  --c");
        assert_eq!(args, vec!["--a=1", "--b=C:\\x y\\f.txt", "--c"]);
    }
}
