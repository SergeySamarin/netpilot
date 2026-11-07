//! EN: zapret lists as services. Domains of the author lists (list-general.txt, list-google.txt) are sorted
//!     into services by rules (the domain matches a known suffix or is its subdomain); the rest is "Other
//!     from the zapret authors". Author files are never changed:
//!     - a turned-off service — its domains go to a "# NetPilot" block in list-exclude-user.txt;
//!     - own domains — list-general-user.txt; own exclusions — list-exclude-user.txt outside the block.
//!     winws re-reads changed lists by itself in a few seconds, so changes need no service restart.
//! RU: Списки zapret как сервисы. Домены из списков авторов (list-general.txt, list-google.txt)
//!     раскладываются по сервисам по правилам (домен совпадает с известным суффиксом или является его
//!     поддоменом), остальное — «Прочее от авторов zapret». Файлы авторов не меняются никогда:
//!     - выключенный сервис — его домены пишутся в блок «# NetPilot» в list-exclude-user.txt;
//!     - свои домены — в list-general-user.txt; свои исключения — в list-exclude-user.txt вне блока.
//!     winws сам перечитывает изменённые списки за несколько секунд, поэтому перезапуск службы не нужен.

use crate::i18n::{t, tf};
use serde::Serialize;
use std::fs;
use std::path::Path;

const GENERAL: &str = "list-general.txt";
const GOOGLE: &str = "list-google.txt";
const EXCLUDE: &str = "list-exclude.txt";
const GENERAL_USER: &str = "list-general-user.txt";
const EXCLUDE_USER: &str = "list-exclude-user.txt";
/// EN: zapret needs non-empty user lists, this domain keeps them non-empty.
/// RU: zapret нужны непустые пользовательские списки, этот домен держит их непустыми.
const PLACEHOLDER: &str = "domain.example.abc";
const BLOCK_START: &str = "# NetPilot: turned-off services — managed by the app, do not edit";
const BLOCK_END: &str = "# NetPilot: end";

pub const OTHER: &str = "other";
/// EN: Order in the card: the services people care about first, technical ones last.
/// RU: Порядок в карточке: сначала сервисы, которые важны людям, технические — в конце.
const DISPLAY_ORDER: [&str; 6] = ["youtube", "discord", "twitch", "cloudflare", "dns", OTHER];

/// EN: A service and its rules. RU: Сервис и его правила.
struct Service {
    id: &'static str,
    /// EN: The name in the UI language. RU: Название на языке интерфейса.
    name: fn() -> String,
    /// EN: The domain is this suffix or its subdomain. RU: Домен — этот суффикс или его поддомен.
    suffixes: &'static [&'static str],
    /// EN: The domain contains this word. RU: Домен содержит это слово.
    words: &'static [&'static str],
}

/// EN: The order matters: the first matching service wins (cloudflare-dns.com is DNS, not Cloudflare).
/// RU: Порядок важен: побеждает первый подходящий сервис (cloudflare-dns.com — это DNS, а не Cloudflare).
const SERVICES: [Service; 5] = [
    Service {
        id: "dns",
        name: || t("Защищённый DNS"),
        suffixes: &[
            "dns.google",
            "dns.quad9.net",
            "dns.nextdns.io",
            "doh.opendns.com",
            "doh.cleanbrowsing.org",
            "freedns.controld.com",
            "wikimedia-dns.org",
            "cloudflare-dns.com",
        ],
        words: &[],
    },
    Service {
        id: "youtube",
        name: || "YouTube".to_string(),
        suffixes: &[
            "youtube.com",
            "youtu.be",
            "googlevideo.com",
            "ytimg.com",
            "ggpht.com",
            "youtube-nocookie.com",
            "youtubekids.com",
            "youtubei.googleapis.com",
            "youtube.googleapis.com",
            "youtubeembeddedplayer.googleapis.com",
            "jnn-pa.googleapis.com",
            "yt3.googleusercontent.com",
            "wide-youtube.l.google.com",
            "youtube-ui.l.google.com",
            "yt-video-upload.l.google.com",
            "ytimg.l.google.com",
        ],
        words: &[],
    },
    Service { id: "discord", name: || "Discord".to_string(), suffixes: &["dis.gd"], words: &["discord"] },
    Service {
        id: "twitch",
        name: || t("Смайлы в чатах стримов"),
        suffixes: &["frankerfacez.com", "ffzap.com", "betterttv.net", "7tv.app", "7tv.io", "live-video.net"],
        words: &[],
    },
    Service {
        id: "cloudflare",
        name: || "Cloudflare".to_string(),
        suffixes: &["cloudfront.net", "encryptedsni.com"],
        words: &["cloudflare"],
    },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    /// EN: Entries of the author lists as written there (`^` — without subdomains).
    /// RU: Записи списков авторов как они там написаны (`^` — без поддоменов).
    pub domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServicesState {
    pub services: Vec<ServiceInfo>,
    /// EN: Own domains (list-general-user.txt). RU: Свои домены (list-general-user.txt).
    pub custom: Vec<String>,
    /// EN: Exclusions of the authors (list-exclude.txt). RU: Исключения авторов (list-exclude.txt).
    pub author_exclusions: Vec<String>,
    /// EN: Own exclusions (list-exclude-user.txt outside the app block). RU: Свои исключения (list-exclude-user.txt вне блока приложения).
    pub user_exclusions: Vec<String>,
    /// EN: Author entries that appeared with the last zapret update. RU: Записи авторов, появившиеся с последним обновлением zapret.
    pub fresh: Vec<String>,
}

/// EN: Result of adding an own domain. RU: Итог добавления своего домена.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AddResult {
    Added { domain: String },
    /// EN: Already among own domains. RU: Уже есть среди своих доменов.
    Exists { domain: String },
    /// EN: Already in the list of a service. RU: Уже есть в списке сервиса.
    Covered { domain: String, service: String, by: String },
}

/// EN: Strips the scheme, the path, the port and "www.". RU: Убирает схему, путь, порт и «www.».
pub fn normalize(input: &str) -> Option<String> {
    let text = input.trim().to_lowercase();
    let text = text.split("://").last().unwrap_or(&text);
    let host = text.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.split(':').next().unwrap_or("").trim_end_matches('.');
    let host = host.strip_prefix("www.").unwrap_or(host);
    let valid = host.contains('.')
        && !host.starts_with('.')
        && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
    valid.then(|| host.to_string())
}

fn is_under(domain: &str, suffix: &str) -> bool {
    domain == suffix || domain.ends_with(&format!(".{suffix}"))
}

/// EN: The service of a domain by the rules; None — no rule matches. RU: Сервис домена по правилам; None — ни одно правило не подошло.
fn service_of(domain: &str) -> Option<&'static Service> {
    let domain = domain.trim_start_matches('^');
    SERVICES.iter().find(|service| {
        service.suffixes.iter().any(|suffix| is_under(domain, suffix))
            || service.words.iter().any(|word| domain.contains(word))
    })
}

/// EN: A list entry covers the domain: the same domain or its parent (entries with `^` — only the same).
/// RU: Запись списка покрывает домен: тот же домен или родительский (запись с `^` — только тот же).
fn covers(entry: &str, domain: &str) -> bool {
    match entry.strip_prefix('^') {
        Some(exact) => exact == domain,
        None => is_under(domain, entry),
    }
}

fn read_entries(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && *line != PLACEHOLDER)
        .map(str::to_string)
        .collect()
}

/// EN: Author entries with their service id. List-google entries without a rule are YouTube helpers
///     (Google fonts and static files for the YouTube page). RU: Записи авторов с идентификатором сервиса.
///     Записи list-google без правила — вспомогательные для YouTube (шрифты и статика Google для страницы).
fn author_entries(lists: &Path) -> Vec<(String, &'static str)> {
    let mut entries: Vec<(String, &'static str)> = Vec::new();
    for (file, fallback) in [(GOOGLE, "youtube"), (GENERAL, OTHER)] {
        for entry in read_entries(&lists.join(file)) {
            if entries.iter().any(|(known, _)| *known == entry) {
                continue;
            }
            let id = service_of(&entry).map_or(fallback, |service| service.id);
            entries.push((entry, id));
        }
    }
    entries
}

/// EN: Lines of list-exclude-user.txt: (outside the app block, inside it). RU: Строки list-exclude-user.txt: (вне блока приложения, внутри).
fn split_exclude_user(text: &str) -> (Vec<String>, Vec<String>) {
    let mut outside = Vec::new();
    let mut inside = Vec::new();
    let mut is_inside = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == BLOCK_START {
            is_inside = true;
        } else if trimmed == BLOCK_END {
            is_inside = false;
        } else if is_inside {
            inside.push(trimmed.to_string());
        } else {
            outside.push(line.trim_end().to_string());
        }
    }
    (outside, inside)
}

fn domains_only(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#') && *line != PLACEHOLDER)
        .map(str::to_string)
        .collect()
}

pub fn state(zapret_dir: &Path, disabled: &[String]) -> ServicesState {
    let lists = zapret_dir.join("lists");
    let entries = author_entries(&lists);
    let mut services: Vec<ServiceInfo> = SERVICES
        .iter()
        .map(|service| ServiceInfo {
            id: service.id.to_string(),
            name: (service.name)(),
            enabled: !disabled.iter().any(|id| id == service.id),
            domains: Vec::new(),
        })
        .collect();
    services.push(ServiceInfo {
        id: OTHER.to_string(),
        name: t("Прочее от авторов zapret"),
        enabled: !disabled.iter().any(|id| id == OTHER),
        domains: Vec::new(),
    });
    for (entry, id) in entries {
        if let Some(service) = services.iter_mut().find(|service| service.id == id) {
            service.domains.push(entry);
        }
    }
    services.retain(|service| !service.domains.is_empty());
    services.sort_by_key(|service| DISPLAY_ORDER.iter().position(|id| *id == service.id).unwrap_or(usize::MAX));
    let exclude_user = fs::read_to_string(lists.join(EXCLUDE_USER)).unwrap_or_default();
    let (outside, _) = split_exclude_user(&exclude_user);
    ServicesState {
        services,
        custom: read_entries(&lists.join(GENERAL_USER)),
        author_exclusions: read_entries(&lists.join(EXCLUDE)),
        user_exclusions: domains_only(&outside),
        fresh: Vec::new(),
    }
}

/// EN: All author entries, to notice new ones after an update. RU: Все записи авторов, чтобы заметить новые после обновления.
pub fn author_domains(zapret_dir: &Path) -> Vec<String> {
    author_entries(&zapret_dir.join("lists")).into_iter().map(|(entry, _)| entry).collect()
}

/// EN: New author entries against the known ones. Empty `known` — the first look, nothing is new.
/// RU: Новые записи авторов относительно известных. Пустой `known` — первый взгляд, нового нет.
pub fn new_domains(known: &[String], current: &[String]) -> Vec<String> {
    if known.is_empty() {
        return Vec::new();
    }
    current.iter().filter(|entry| !known.contains(entry)).cloned().collect()
}

/// EN: Rewrites the app block of list-exclude-user.txt with the domains of the turned-off services.
///     Called after a change and after a zapret update (the new version has its own author lists).
/// RU: Переписывает блок приложения в list-exclude-user.txt доменами выключенных сервисов.
///     Вызывается после изменения и после обновления zapret (у новой версии свои списки авторов).
pub fn sync_exclusions(zapret_dir: &Path, disabled: &[String]) -> Result<(), String> {
    let lists = zapret_dir.join("lists");
    let path = lists.join(EXCLUDE_USER);
    let text = fs::read_to_string(&path).unwrap_or_default();
    let (outside, inside) = split_exclude_user(&text);
    let domains: Vec<String> = author_entries(&lists)
        .into_iter()
        .filter(|(_, id)| disabled.iter().any(|disabled_id| disabled_id == id))
        .map(|(entry, _)| entry)
        .collect();
    if domains == inside {
        return Ok(());
    }
    write_lines(&path, &compose_exclude_user(&outside, &domains))
}

fn compose_exclude_user(outside: &[String], domains: &[String]) -> Vec<String> {
    let mut lines: Vec<String> = outside.iter().filter(|line| !line.trim().is_empty()).cloned().collect();
    if lines.is_empty() {
        lines.push(PLACEHOLDER.to_string());
    }
    if !domains.is_empty() {
        lines.push(BLOCK_START.to_string());
        lines.extend(domains.iter().cloned());
        lines.push(BLOCK_END.to_string());
    }
    lines
}

fn write_lines(path: &Path, lines: &[String]) -> Result<(), String> {
    let mut text = lines.join("\n");
    text.push('\n');
    fs::write(path, text).map_err(|e| {
        tf("Не удалось записать {name}: {error}", &[("name", &path.display()), ("error", &e)])
    })
}

/// EN: Adds an own domain to list-general-user.txt, unless a service list already covers it.
/// RU: Добавляет свой домен в list-general-user.txt, если его ещё не покрывает список сервиса.
pub fn add_custom(zapret_dir: &Path, input: &str) -> Result<AddResult, String> {
    let domain = normalize(input).ok_or_else(|| tf("«{text}» не похоже на домен", &[("text", &input.trim())]))?;
    let lists = zapret_dir.join("lists");
    if let Some((entry, id)) = author_entries(&lists).into_iter().find(|(entry, _)| covers(entry, &domain)) {
        let service = SERVICES.iter().find(|service| service.id == id).map_or_else(
            || t("Прочее от авторов zapret"),
            |service| (service.name)(),
        );
        return Ok(AddResult::Covered { domain, service, by: entry });
    }
    let path = lists.join(GENERAL_USER);
    let mut lines: Vec<String> =
        fs::read_to_string(&path).unwrap_or_default().lines().map(|line| line.trim_end().to_string()).collect();
    if lines.iter().any(|line| line.trim() == domain) {
        return Ok(AddResult::Exists { domain });
    }
    lines.retain(|line| !line.trim().is_empty());
    lines.push(domain.clone());
    write_lines(&path, &lines)?;
    Ok(AddResult::Added { domain })
}

/// EN: Removes a line from a user list; keeps the placeholder so the list never gets empty.
/// RU: Удаляет строку из пользовательского списка; оставляет заглушку, чтобы список не опустел.
fn remove_line(path: &Path, domain: &str, keep_block: bool) -> Result<(), String> {
    let text = fs::read_to_string(path).unwrap_or_default();
    if keep_block {
        let (outside, inside) = split_exclude_user(&text);
        let outside: Vec<String> = outside.into_iter().filter(|line| line.trim() != domain).collect();
        return write_lines(path, &compose_exclude_user(&outside, &inside));
    }
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| line.trim_end().to_string())
        .filter(|line| line.trim() != domain && !line.trim().is_empty())
        .collect();
    if domains_only(&lines).is_empty() && !lines.iter().any(|line| line.trim() == PLACEHOLDER) {
        lines.push(PLACEHOLDER.to_string());
    }
    write_lines(path, &lines)
}

pub fn remove_custom(zapret_dir: &Path, domain: &str) -> Result<(), String> {
    remove_line(&zapret_dir.join("lists").join(GENERAL_USER), domain, false)
}

pub fn add_exclusion(zapret_dir: &Path, input: &str) -> Result<String, String> {
    let domain = normalize(input).ok_or_else(|| tf("«{text}» не похоже на домен", &[("text", &input.trim())]))?;
    let path = zapret_dir.join("lists").join(EXCLUDE_USER);
    let text = fs::read_to_string(&path).unwrap_or_default();
    let (mut outside, inside) = split_exclude_user(&text);
    if !outside.iter().any(|line| line.trim() == domain) {
        outside.push(domain.clone());
        write_lines(&path, &compose_exclude_user(&outside, &inside))?;
    }
    Ok(domain)
}

pub fn remove_exclusion(zapret_dir: &Path, domain: &str) -> Result<(), String> {
    remove_line(&zapret_dir.join("lists").join(EXCLUDE_USER), domain, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_lists(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("netpilot-services-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("lists")).unwrap();
        fs::write(dir.join("lists").join(GOOGLE), "googlevideo.com\nyoutube.com\nfonts.googleapis.com\nstable.dl2.discordapp.net\n").unwrap();
        fs::write(dir.join("lists").join(GENERAL), "^dns.google\ncloudflare-dns.com\ncloudflare-ech.com\ndiscord.com\nzendesk.com\n").unwrap();
        fs::write(dir.join("lists").join(EXCLUDE_USER), "domain.example.abc\nmy-exclude.ru\n").unwrap();
        fs::write(dir.join("lists").join(GENERAL_USER), "# Never leave this file empty\ndomain.example.abc\n").unwrap();
        dir
    }

    #[test]
    fn notices_new_author_domains() {
        let known = vec!["a.com".to_string(), "b.com".to_string()];
        let current = vec!["a.com".to_string(), "c.com".to_string()];
        assert_eq!(new_domains(&known, &current), vec!["c.com"]);
        assert!(new_domains(&[], &current).is_empty());
    }

    #[test]
    fn normalizes_input() {
        assert_eq!(normalize("https://www.Rutracker.org/forum?x=1").as_deref(), Some("rutracker.org"));
        assert_eq!(normalize(" rr3.googlevideo.com:443 ").as_deref(), Some("rr3.googlevideo.com"));
        assert_eq!(normalize("not a domain"), None);
    }

    #[test]
    fn sorts_author_domains() {
        let dir = temp_lists("sort");
        let state = state(&dir, &[]);
        let ids = |id: &str| state.services.iter().find(|service| service.id == id).unwrap().domains.clone();
        assert_eq!(ids("youtube"), vec!["googlevideo.com", "youtube.com", "fonts.googleapis.com"]);
        assert_eq!(ids("discord"), vec!["stable.dl2.discordapp.net", "discord.com"]);
        assert_eq!(ids("dns"), vec!["^dns.google", "cloudflare-dns.com"]);
        assert_eq!(ids("cloudflare"), vec!["cloudflare-ech.com"]);
        assert_eq!(ids(OTHER), vec!["zendesk.com"]);
        assert_eq!(state.user_exclusions, vec!["my-exclude.ru"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn turned_off_service_goes_to_the_app_block_and_back() {
        let dir = temp_lists("block");
        let path = dir.join("lists").join(EXCLUDE_USER);
        sync_exclusions(&dir, &["discord".to_string()]).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(BLOCK_START) && text.contains("discord.com") && text.contains("my-exclude.ru"));
        assert!(!state(&dir, &["discord".to_string()]).user_exclusions.contains(&"discord.com".to_string()));

        add_exclusion(&dir, "another.ru").unwrap();
        remove_exclusion(&dir, "my-exclude.ru").unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("another.ru") && text.contains("discord.com") && !text.contains("my-exclude.ru"));

        sync_exclusions(&dir, &[]).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("discord.com") && !text.contains(BLOCK_START) && text.contains("another.ru"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn custom_domains_respect_service_lists() {
        let dir = temp_lists("custom");
        match add_custom(&dir, "rr5---sn-abc.googlevideo.com").unwrap() {
            AddResult::Covered { service, by, .. } => {
                assert_eq!(service, "YouTube");
                assert_eq!(by, "googlevideo.com");
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(add_custom(&dir, "https://rutracker.org/").unwrap(), AddResult::Added { .. }));
        assert!(matches!(add_custom(&dir, "rutracker.org").unwrap(), AddResult::Exists { .. }));
        assert_eq!(state(&dir, &[]).custom, vec!["rutracker.org"]);
        remove_custom(&dir, "rutracker.org").unwrap();
        let text = fs::read_to_string(dir.join("lists").join(GENERAL_USER)).unwrap();
        assert!(text.contains(PLACEHOLDER) && !text.contains("rutracker.org"));
        let _ = fs::remove_dir_all(dir);
    }
}
