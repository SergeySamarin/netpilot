//! EN: zapret-discord-youtube releases on GitHub: latest version, download and unpacking.
//! RU: Релизы zapret-discord-youtube на GitHub: последняя версия, скачивание и распаковка.

use crate::i18n::{t, tf};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

const RELEASES_URL: &str = "https://api.github.com/repos/Flowseal/zapret-discord-youtube/releases";
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/Flowseal/zapret-discord-youtube/releases/latest";
const USER_AGENT: &str = "NetPilot";

/// EN: UI event with the download progress. RU: Событие для интерфейса с ходом скачивания.
pub const PROGRESS_EVENT: &str = "zapret-download-progress";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub version: String,
    pub published_at: String,
    pub download_url: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    /// EN: "download" or "extract". RU: "download" или "extract".
    pub stage: &'static str,
    pub downloaded: u64,
    pub total: u64,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    published_at: String,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

/// EN: GitHub is reachable from Russia directly, so the system proxy (HTTP_PROXY) is not used.
/// RU: GitHub из России доступен напрямую, поэтому системный прокси (HTTP_PROXY) не используем.
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())
}

pub async fn latest() -> Result<ReleaseInfo, String> {
    fetch(LATEST_RELEASE_URL).await
}

/// EN: A specific release by its version tag, e.g. "1.10.3".
/// RU: Конкретный релиз по тегу версии, например "1.10.3".
pub async fn by_version(version: &str) -> Result<ReleaseInfo, String> {
    fetch(&format!("{RELEASES_URL}/tags/{version}")).await
}

async fn fetch(url: &str) -> Result<ReleaseInfo, String> {
    let release: GithubRelease = client()?
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| tf("Не удалось получить данные о релизе с GitHub: {error}", &[("error", &e)]))?
        .json()
        .await
        .map_err(|e| tf("GitHub вернул непонятный ответ: {error}", &[("error", &e)]))?;

    let asset = release
        .assets
        .into_iter()
        .find(|a| a.name.to_lowercase().ends_with(".zip"))
        .ok_or_else(|| t("В релизе нет zip-архива"))?;
    crate::debug!("github release {url}: tag={} asset={} size={}", release.tag_name, asset.name, asset.size);

    Ok(ReleaseInfo {
        version: release.tag_name,
        published_at: release.published_at,
        download_url: asset.browser_download_url,
        size: asset.size,
    })
}

/// EN: Downloads the release and unpacks it to `target_dir`. The folder is created anew.
/// RU: Скачивает релиз и распаковывает в `target_dir`. Папка создаётся заново.
pub async fn download(app: &AppHandle, release: &ReleaseInfo, target_dir: &Path) -> Result<(), String> {
    if target_dir.exists() {
        fs::remove_dir_all(target_dir).map_err(|e| tf("Не удалось очистить папку: {error}", &[("error", &e)]))?;
    }
    fs::create_dir_all(target_dir).map_err(|e| tf("Не удалось создать папку: {error}", &[("error", &e)]))?;

    let zip_path = target_dir.join("release.zip");
    download_file(app, &release.download_url, &zip_path).await?;

    let _ = app.emit(PROGRESS_EVENT, DownloadProgress { stage: "extract", downloaded: 0, total: 0 });
    let dir = target_dir.to_path_buf();
    let zip_for_task = zip_path.clone();
    tauri::async_runtime::spawn_blocking(move || extract_zip(&zip_for_task, &dir))
        .await
        .map_err(|e| e.to_string())??;

    let _ = fs::remove_file(&zip_path);
    Ok(())
}

async fn download_file(app: &AppHandle, url: &str, path: &PathBuf) -> Result<(), String> {
    let response = client()?
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| tf("Не удалось скачать архив: {error}", &[("error", &e)]))?;

    let total = response.content_length().unwrap_or(0);
    crate::debug!("download {url}: status={} size={total}", response.status());
    let mut file = fs::File::create(path).map_err(|e| tf("Не удалось создать файл: {error}", &[("error", &e)]))?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| tf("Скачивание прервалось: {error}", &[("error", &e)]))?;
        file.write_all(&chunk).map_err(|e| tf("Не удалось записать файл: {error}", &[("error", &e)]))?;
        downloaded += chunk.len() as u64;
        let _ = app.emit(PROGRESS_EVENT, DownloadProgress { stage: "download", downloaded, total });
    }
    crate::debug!("download done: {downloaded} bytes");
    Ok(())
}

/// EN: Unpacks the archive. The release has everything in one `zapret-discord-youtube-<version>/` folder — it is dropped.
/// RU: Распаковывает архив. В релизе всё лежит в одной папке `zapret-discord-youtube-<версия>/` — её убираем.
fn extract_zip(zip_path: &Path, target_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| tf("Архив повреждён: {error}", &[("error", &e)]))?;

    let root = common_root(&mut archive);

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        // EN: enclosed_name protects against ../../ paths inside the archive.
        // RU: enclosed_name защищает от путей вида ../../ внутри архива.
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let relative = match &root {
            Some(root) => match relative.strip_prefix(root) {
                Ok(rest) => rest.to_path_buf(),
                Err(_) => relative,
            },
            None => relative,
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let out_path = target_dir.join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = fs::File::create(&out_path)
            .map_err(|e| tf("Не удалось создать {name}: {error}", &[("name", &relative.display()), ("error", &e)]))?;
        std::io::copy(&mut entry, &mut out)
            .map_err(|e| tf("Не удалось распаковать {name}: {error}", &[("name", &relative.display()), ("error", &e)]))?;
    }
    Ok(())
}

/// EN: If all archive files are in one top-level folder, returns it.
/// RU: Если все файлы архива лежат в одной папке верхнего уровня — возвращает её.
fn common_root(archive: &mut zip::ZipArchive<fs::File>) -> Option<PathBuf> {
    let mut root: Option<PathBuf> = None;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).ok()?;
        let path = entry.enclosed_name()?;
        let first = PathBuf::from(path.components().next()?.as_os_str());
        // EN: A file right in the archive root — there is no common folder.
        // RU: Файл прямо в корне архива — общей папки нет.
        if path.components().count() == 1 && !entry.is_dir() {
            return None;
        }
        match &root {
            None => root = Some(first),
            Some(existing) if *existing != first => return None,
            _ => {}
        }
    }
    root
}
