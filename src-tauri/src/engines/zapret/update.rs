//! EN: zapret update: version comparison, untouched release copies and moving user changes into the
//!     new version. Lists are merged line by line against the untouched copy of the old version:
//!     lines the user added are added, lines the user removed are removed, the rest comes from the new version.
//! RU: Обновление zapret: сравнение версий, нетронутые копии релизов и перенос изменений пользователя в
//!     новую версию. Списки сливаются построчно относительно нетронутой копии старой версии: добавленные
//!     пользователем строки добавляются, удалённые — удаляются, остальное берётся из новой версии.

use super::control::USER_LIST_DEFAULTS;
use super::fsutil;
use super::release;
use crate::i18n::tf;
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use tauri::AppHandle;

/// EN: ipset-all.txt contents in the "none" mode of service.bat. RU: Содержимое ipset-all.txt в режиме «none» service.bat.
const IPSET_NONE: &str = "203.0.113.113/32";
const IPSET_FILE: &str = "ipset-all.txt";
/// EN: Fake packets the user can swap via the service.bat menu. RU: Подделки, которые пользователь меняет через меню service.bat.
const ACTIVE_FAKE_PREFIX: &str = "ACTIVE_";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListChange {
    pub file: String,
    pub added: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationReport {
    /// EN: User's own files copied as is (*-user.txt, own strategies). RU: Файлы пользователя, скопированы как есть.
    pub user_files: Vec<String>,
    /// EN: Author lists with the user's edits applied. RU: Списки авторов, в которые перенесены правки пользователя.
    pub merged_lists: Vec<ListChange>,
    /// EN: IPSet mode kept: "none" or "any". RU: Сохранённый режим IPSet: "none" или "any".
    pub ipset_mode: Option<String>,
    /// EN: Settings kept: game filter, update check, active fakes. RU: Сохранённые настройки.
    pub settings: Vec<String>,
    /// EN: Author files the user edited that are not merged (strategies, service.bat).
    /// RU: Файлы авторов с правками пользователя, которые не переносятся (стратегии, service.bat).
    pub not_transferred: Vec<String>,
}

/// EN: "1.10.4" vs "1.11.0": numbers are compared one by one.
/// RU: "1.10.4" и "1.11.0": числа сравниваются по очереди.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let parse = |v: &str| -> Vec<u64> {
        v.trim_start_matches(['v', 'V'])
            .split(|c: char| !c.is_ascii_digit())
            .filter(|part| !part.is_empty())
            .map(|part| part.parse().unwrap_or(0))
            .collect()
    };
    parse(a).cmp(&parse(b))
}

/// EN: Untouched copy of the given version: from the cache or downloaded from GitHub.
/// RU: Нетронутая копия версии: из кэша или скачивается с GitHub.
pub async fn ensure_original(app: &AppHandle, version: &str) -> Result<std::path::PathBuf, String> {
    let dir = fsutil::originals_dir(version)?;
    if dir.join("bin").join("winws.exe").is_file() {
        return Ok(dir);
    }
    let info = release::by_version(version).await?;
    release::download(app, &info, &dir).await?;
    Ok(dir)
}

/// EN: Saves an untouched copy right after download, before the user changes anything.
/// RU: Сохраняет нетронутую копию сразу после скачивания, пока пользователь ничего не поменял.
pub fn save_original(dir: &Path, version: &str) {
    if let Ok(target) = fsutil::originals_dir(version) {
        if !target.exists() {
            let _ = fsutil::copy_dir(dir, &target);
        }
    }
}

/// EN: Deletes untouched copies except the listed versions.
/// RU: Удаляет нетронутые копии, кроме перечисленных версий.
pub fn cleanup_originals(keep: &[&str]) {
    let Ok(root) = fsutil::originals_dir("") else {
        return;
    };
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !keep.contains(&name.as_str()) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// EN: Moves user changes from `old` into `new`. `original` is the untouched copy of the old version.
/// RU: Переносит изменения пользователя из `old` в `new`. `original` — нетронутая копия старой версии.
pub fn migrate(old: &Path, original: &Path, new: &Path) -> Result<MigrationReport, String> {
    let mut report = MigrationReport::default();
    migrate_lists(old, original, new, &mut report)?;
    migrate_utils(old, original, new, &mut report)?;
    migrate_fakes(old, original, new, &mut report)?;
    migrate_root(old, original, new, &mut report)?;
    Ok(report)
}

fn file_names(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().is_file())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn read_lines(path: &Path) -> Vec<String> {
    let bytes = fs::read(path).unwrap_or_default();
    String::from_utf8_lossy(&bytes)
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

/// EN: Files are equal ignoring line endings and empty lines.
/// RU: Файлы равны без учёта переводов строк и пустых строк.
fn same_text(a: &Path, b: &Path) -> bool {
    read_lines(a) == read_lines(b)
}

fn copy_file(from: &Path, to: &Path) -> Result<(), String> {
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| tf("Не удалось скопировать {path}: {error}", &[("path", &from.display()), ("error", &e)]))
}

fn write_lines(path: &Path, lines: &[String]) -> Result<(), String> {
    let mut text = lines.join("\r\n");
    text.push_str("\r\n");
    fs::write(path, text).map_err(|e| tf("Не удалось записать файл: {error}", &[("error", &e)]))
}

fn migrate_lists(old: &Path, original: &Path, new: &Path, report: &mut MigrationReport) -> Result<(), String> {
    let (old_dir, original_dir, new_dir) = (old.join("lists"), original.join("lists"), new.join("lists"));
    fs::create_dir_all(&new_dir).map_err(|e| tf("Не удалось создать папку: {error}", &[("error", &e)]))?;

    for name in file_names(&old_dir) {
        let old_file = old_dir.join(&name);
        let original_file = original_dir.join(&name);
        let new_file = new_dir.join(&name);

        // EN: The full ipset list in none/any mode is kept by service.bat in .backup — handled together with ipset-all.txt.
        // RU: Полный ipset в режимах none/any service.bat держит в .backup — обрабатывается вместе с ipset-all.txt.
        if name == format!("{IPSET_FILE}.backup") {
            continue;
        }
        if name == IPSET_FILE {
            if !same_text(&old_file, &original_file) {
                migrate_ipset(&old_file, &new_dir, report)?;
            }
            continue;
        }
        if !original_file.exists() {
            copy_file(&old_file, &new_file)?;
            if !is_untouched_template(&name, &old_file) {
                report.user_files.push(format!("lists\\{name}"));
            }
            continue;
        }
        if same_text(&old_file, &original_file) {
            continue;
        }
        let change = merge_list(&old_file, &original_file, &new_file)?;
        if change.added + change.removed > 0 {
            report.merged_lists.push(ListChange { file: format!("lists\\{name}"), ..change });
        }
    }
    Ok(())
}

/// EN: A user list that still has the template contents — nothing to report. Comment lines are ignored:
///     older service.bat versions wrote the template without the comment.
/// RU: Пользовательский список с содержимым-заготовкой — сообщать не о чем. Строки-комментарии не
///     учитываются: старые версии service.bat писали заготовку без комментария.
fn is_untouched_template(name: &str, file: &Path) -> bool {
    let without_comments = |lines: Vec<String>| -> Vec<String> {
        lines.into_iter().filter(|line| !line.starts_with('#')).collect()
    };
    USER_LIST_DEFAULTS.iter().any(|(template_name, content)| {
        let template: Vec<String> =
            content.lines().map(str::trim).filter(|line| !line.is_empty()).map(String::from).collect();
        *template_name == name && without_comments(read_lines(file)) == without_comments(template)
    })
}

/// EN: new + lines the user added − lines the user removed.
/// RU: Новая версия + строки, добавленные пользователем, − строки, удалённые пользователем.
fn merge_list(old_file: &Path, original_file: &Path, new_file: &Path) -> Result<ListChange, String> {
    let old_lines = read_lines(old_file);
    let original_lines = read_lines(original_file);
    let old_set: HashSet<&String> = old_lines.iter().collect();
    let original_set: HashSet<&String> = original_lines.iter().collect();

    let added: Vec<&String> = old_lines.iter().filter(|line| !original_set.contains(line)).collect();
    let removed: HashSet<&String> = original_lines.iter().filter(|line| !old_set.contains(line)).collect();

    let mut result: Vec<String> = read_lines(new_file).into_iter().filter(|line| !removed.contains(line)).collect();
    for line in &added {
        if !result.contains(line) {
            result.push((*line).clone());
        }
    }
    write_lines(new_file, &result)?;
    Ok(ListChange { file: String::new(), added: added.len(), removed: removed.len() })
}

/// EN: In none/any mode the fresh full list goes to .backup and the mode file is kept.
/// RU: В режиме none/any свежий полный список уходит в .backup, а файл режима сохраняется.
fn migrate_ipset(old_file: &Path, new_dir: &Path, report: &mut MigrationReport) -> Result<(), String> {
    let lines = read_lines(old_file);
    let mode = if lines.is_empty() {
        "any"
    } else if lines == [IPSET_NONE] {
        "none"
    } else {
        return Ok(());
    };
    let new_file = new_dir.join(IPSET_FILE);
    let backup = new_dir.join(format!("{IPSET_FILE}.backup"));
    if new_file.exists() {
        let _ = fs::remove_file(&backup);
        fs::rename(&new_file, &backup).map_err(|e| tf("Не удалось записать файл: {error}", &[("error", &e)]))?;
    }
    copy_file(old_file, &new_file)?;
    report.ipset_mode = Some(mode.into());
    Ok(())
}

/// EN: utils\*.enabled flags: the state of the old version is kept, including a deleted flag.
/// RU: Флаги utils\*.enabled: сохраняется состояние старой версии, в том числе удалённый флаг.
fn migrate_utils(old: &Path, original: &Path, new: &Path, report: &mut MigrationReport) -> Result<(), String> {
    let (old_dir, original_dir, new_dir) = (old.join("utils"), original.join("utils"), new.join("utils"));
    let mut names: HashSet<String> = file_names(&old_dir).into_iter().collect();
    names.extend(file_names(&original_dir));

    for name in names.into_iter().filter(|name| name.ends_with(".enabled")) {
        let old_file = old_dir.join(&name);
        let original_file = original_dir.join(&name);
        let new_file = new_dir.join(&name);
        match (old_file.exists(), original_file.exists()) {
            (true, has_original) => {
                if !has_original || !same_text(&old_file, &original_file) {
                    copy_file(&old_file, &new_file)?;
                    report.settings.push(format!("utils\\{name}"));
                }
            }
            (false, true) => {
                let _ = fs::remove_file(&new_file);
                report.settings.push(format!("utils\\{name}"));
            }
            (false, false) => {}
        }
    }
    Ok(())
}

/// EN: bin\ACTIVE_*.bin — fake packets chosen by the user.
/// RU: bin\ACTIVE_*.bin — подделки, выбранные пользователем.
fn migrate_fakes(old: &Path, original: &Path, new: &Path, report: &mut MigrationReport) -> Result<(), String> {
    let (old_dir, original_dir, new_dir) = (old.join("bin"), original.join("bin"), new.join("bin"));
    for name in file_names(&old_dir).into_iter().filter(|name| name.starts_with(ACTIVE_FAKE_PREFIX)) {
        let old_file = old_dir.join(&name);
        let original_bytes = fs::read(original_dir.join(&name)).ok();
        if original_bytes.as_deref() != fs::read(&old_file).ok().as_deref() {
            copy_file(&old_file, &new_dir.join(&name))?;
            report.settings.push(format!("bin\\{name}"));
        }
    }
    Ok(())
}

/// EN: Root files: user's own strategies are copied, edited author files are only reported.
/// RU: Файлы в корне: свои стратегии пользователя копируются, правки в файлах авторов только сообщаются.
fn migrate_root(old: &Path, original: &Path, new: &Path, report: &mut MigrationReport) -> Result<(), String> {
    for name in file_names(old) {
        let lower = name.to_lowercase();
        if !lower.ends_with(".bat") && !lower.ends_with(".cmd") {
            continue;
        }
        let old_file = old.join(&name);
        let original_file = original.join(&name);
        if !original_file.exists() {
            copy_file(&old_file, &new.join(&name))?;
            report.user_files.push(name);
        } else if !same_text(&old_file, &original_file) {
            report.not_transferred.push(name);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_numbers() {
        assert_eq!(compare_versions("1.10.4", "1.9.9"), Ordering::Greater);
        assert_eq!(compare_versions("1.10.4", "1.10.4"), Ordering::Equal);
        assert_eq!(compare_versions("v1.10", "1.10.1"), Ordering::Less);
    }

    #[test]
    fn merge_keeps_user_changes() {
        let dir = std::env::temp_dir().join(format!("netpilot-merge-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let (old, original, new) = (dir.join("old.txt"), dir.join("original.txt"), dir.join("new.txt"));
        fs::write(&original, "a.com\r\nb.com\r\nc.com\r\n").unwrap();
        // EN: user removed b.com and added my.com. RU: пользователь удалил b.com и добавил my.com.
        fs::write(&old, "a.com\r\nc.com\r\nmy.com\r\n").unwrap();
        // EN: authors added d.com. RU: авторы добавили d.com.
        fs::write(&new, "a.com\r\nb.com\r\nc.com\r\nd.com\r\n").unwrap();

        let change = merge_list(&old, &original, &new).unwrap();
        assert_eq!((change.added, change.removed), (1, 1));
        assert_eq!(read_lines(&new), vec!["a.com", "c.com", "d.com", "my.com"]);
        let _ = fs::remove_dir_all(&dir);
    }
}
