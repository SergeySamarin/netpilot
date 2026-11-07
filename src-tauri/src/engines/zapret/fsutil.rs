//! EN: File helpers for zapret folders: recursive copy, clearing, NetPilot folders in ProgramData.
//! RU: Работа с файлами папок zapret: рекурсивное копирование, очистка, папки NetPilot в ProgramData.

use crate::i18n::{t, tf};
use std::fs;
use std::path::{Path, PathBuf};

/// EN: C:\ProgramData\NetPilot — machine-wide folder without a user name in the path.
/// RU: C:\ProgramData\NetPilot — общая папка компьютера, без имени пользователя в пути.
pub fn app_data_dir() -> Result<PathBuf, String> {
    let program_data = std::env::var_os("ProgramData").ok_or_else(|| t("Не удалось найти папку ProgramData"))?;
    Ok(PathBuf::from(program_data).join("NetPilot"))
}

/// EN: Versions downloaded by the app: ProgramData\NetPilot\zapret\<version>.
/// RU: Версии, скачанные приложением: ProgramData\NetPilot\zapret\<версия>.
pub fn managed_root() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("zapret"))
}

/// EN: Untouched copies of releases, used to see what the user changed.
/// RU: Нетронутые копии релизов — по ним видно, что поменял пользователь.
pub fn originals_dir(version: &str) -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("originals").join(version))
}

/// EN: A new version is prepared here before replacing a user-chosen folder.
/// RU: Здесь готовится новая версия, прежде чем заменить папку, выбранную пользователем.
pub fn staging_dir(version: &str) -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("staging").join(version))
}

/// EN: Previous contents of a user-chosen folder, for rollback.
/// RU: Прежнее содержимое папки, выбранной пользователем, — для отката.
pub fn backup_dir() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("backup"))
}

pub fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| tf("Не удалось создать папку: {error}", &[("error", &e)]))?;
    let entries = fs::read_dir(from).map_err(|e| tf("Не удалось прочитать {path}: {error}", &[("path", &from.display()), ("error", &e)]))?;
    for entry in entries.flatten() {
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            copy_dir(&source, &target)?;
        } else {
            fs::copy(&source, &target)
                .map_err(|e| tf("Не удалось скопировать {path}: {error}", &[("path", &source.display()), ("error", &e)]))?;
        }
    }
    Ok(())
}

/// EN: Deletes everything inside the folder, keeping the folder itself.
/// RU: Удаляет всё внутри папки, саму папку оставляет.
pub fn clear_dir(dir: &Path) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| tf("Не удалось прочитать {path}: {error}", &[("path", &dir.display()), ("error", &e)]))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let result = if path.is_dir() { fs::remove_dir_all(&path) } else { fs::remove_file(&path) };
        result.map_err(|e| tf("Не удалось удалить {path}: {error}", &[("path", &path.display()), ("error", &e)]))?;
    }
    Ok(())
}

/// EN: Puts `source` contents into `target`, and the previous `target` contents into `backup`.
///     Works both for an update (source = new version) and for a rollback (source = backup).
/// RU: Кладёт содержимое `source` в `target`, а прежнее содержимое `target` — в `backup`.
///     Подходит и для обновления (source — новая версия), и для отката (source — резервная копия).
pub fn replace_contents(target: &Path, source: &Path, backup: &Path) -> Result<(), String> {
    let previous = backup.with_extension("previous");
    if previous.exists() {
        fs::remove_dir_all(&previous).map_err(|e| tf("Не удалось очистить папку: {error}", &[("error", &e)]))?;
    }
    copy_dir(target, &previous)?;
    clear_dir(target)?;
    copy_dir(source, target)?;

    if source != backup {
        let _ = fs::remove_dir_all(source);
    }
    if backup.exists() {
        fs::remove_dir_all(backup).map_err(|e| tf("Не удалось очистить папку: {error}", &[("error", &e)]))?;
    }
    fs::rename(&previous, backup).map_err(|e| tf("Не удалось сохранить резервную копию: {error}", &[("error", &e)]))
}

pub fn same_dir(a: &Path, b: &Path) -> bool {
    let normalize = |p: &Path| p.to_string_lossy().trim_end_matches('\\').to_lowercase();
    normalize(a) == normalize(b)
}
