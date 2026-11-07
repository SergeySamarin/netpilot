//! EN: Quick service checks: whether YouTube, Discord and the chat emote servers open. Each address gets an HTTPS request
//!     over TLS 1.2 and TLS 1.3. Any HTTP response counts as success: what matters is that DPI did not
//!     break the connection.
//! RU: Быстрые проверки сервисов: открываются ли сайты YouTube, Discord и серверы смайлов для чатов. К каждому адресу — HTTPS-запрос
//!     через TLS 1.2 и TLS 1.3. Любой HTTP-ответ считается успехом: важно, что соединение не оборвал DPI.

use crate::i18n::t;
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// EN: Timeout of one attempt. With a retry the check takes at most 2 × REQUEST_TIMEOUT.
/// RU: Таймаут одной попытки. С повтором проверка занимает не больше 2 × REQUEST_TIMEOUT.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

struct ServiceTargets {
    id: &'static str,
    name: &'static str,
    hosts: &'static [&'static str],
}

const SERVICES: [ServiceTargets; 3] = [
    ServiceTargets {
        id: "youtube",
        name: "YouTube",
        hosts: &["www.youtube.com", "i.ytimg.com", "redirector.googlevideo.com"],
    },
    ServiceTargets {
        id: "discord",
        name: "Discord",
        hosts: &["discord.com", "gateway.discord.gg", "cdn.discordapp.com", "updates.discord.com"],
    },
    // EN: Servers of the chat emote extensions (7TV, BetterTTV, FrankerFaceZ); the id matches the service in services.rs.
    // RU: Серверы расширений со смайлами для чатов (7TV, BetterTTV, FrankerFaceZ); id совпадает с сервисом в services.rs.
    ServiceTargets {
        id: "twitch",
        name: "Смайлы в чатах стримов",
        hosts: &["7tv.io", "cdn.betterttv.net", "cdn.frankerfacez.com"],
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckStatus {
    /// EN: All checks passed. RU: Все проверки прошли.
    Ok,
    /// EN: Some addresses or TLS versions failed. RU: Часть адресов или версий TLS не прошла.
    Partial,
    /// EN: Nothing opened. RU: Ничего не открылось.
    Fail,
}

impl CheckStatus {
    pub fn from_counts(passed: u32, total: u32) -> Self {
        if passed == total {
            Self::Ok
        } else if passed == 0 {
            Self::Fail
        } else {
            Self::Partial
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetResult {
    pub host: String,
    /// EN: Response time in ms, None — the request failed.
    /// RU: Время ответа в мс, None — запрос не прошёл.
    pub tls12_ms: Option<u32>,
    pub tls13_ms: Option<u32>,
    /// EN: Error of the last failed attempt. RU: Ошибка последней неудачной попытки.
    pub error: Option<String>,
    /// EN: At least one request passed only on the second attempt.
    /// RU: Хотя бы один запрос прошёл только со второй попытки.
    pub flaky: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCheck {
    pub id: String,
    pub name: String,
    pub status: CheckStatus,
    pub passed: u32,
    pub total: u32,
    /// EN: How many addresses opened only on the second attempt.
    /// RU: Сколько адресов открылись только со второй попытки.
    pub flaky: u32,
    /// EN: Average response time of successful requests, ms.
    /// RU: Среднее время ответа по успешным запросам, мс.
    pub latency_ms: Option<u32>,
    pub targets: Vec<TargetResult>,
}

#[derive(Clone, Copy)]
enum Tls {
    V12,
    V13,
}

/// EN: Only classic X25519 / P-256 key exchange. The post-quantum X25519MLKEM768 (on by default in rustls)
///     makes ClientHello ~1 KB larger, some strategies break it, and the check would fail on a working site.
/// RU: Только классический обмен ключами X25519 / P-256. Постквантовый X25519MLKEM768 (в rustls включён по
///     умолчанию) делает ClientHello на ~1 КБ больше, часть стратегий его ломает, и проверка не проходила бы
///     на работающем сайте.
fn tls_config(tls: Tls) -> Result<rustls::ClientConfig, String> {
    use rustls::crypto::aws_lc_rs::{self, kx_group};
    use rustls_platform_verifier::BuilderVerifierExt;

    let mut provider = aws_lc_rs::default_provider();
    provider.kx_groups = vec![kx_group::X25519, kx_group::SECP256R1];
    let version: &'static rustls::SupportedProtocolVersion = match tls {
        Tls::V12 => &rustls::version::TLS12,
        Tls::V13 => &rustls::version::TLS13,
    };
    let mut config = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(provider))
        .with_protocol_versions(&[version])
        .map_err(|e| e.to_string())?
        .with_platform_verifier()
        .map_err(|e| e.to_string())?
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(config)
}

/// EN: A client without the system proxy: otherwise requests would go through HTTP_PROXY and the check
///     would say nothing about zapret.
/// RU: Клиент без системного прокси: иначе запросы пошли бы через HTTP_PROXY и проверка ничего не
///     говорила бы о работе zapret.
fn client(tls: Tls) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .tls_backend_preconfigured(tls_config(tls)?)
        .build()
        .map_err(|e| e.to_string())
}

async fn probe(client: &reqwest::Client, host: &str) -> Result<u32, String> {
    let started = Instant::now();
    client
        .head(format!("https://{host}/"))
        .send()
        .await
        .map_err(|e| {
            crate::debug!("probe {host}: {e:?}");
            short_error(&e)
        })?;
    Ok(u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX))
}

fn short_error(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        t("нет ответа (таймаут)")
    } else if error.is_connect() {
        t("соединение не установлено")
    } else {
        t("соединение оборвано")
    }
}

/// EN: A request with one retry: some connections through zapret sometimes fail on the first try.
///     Returns (time or error, whether it passed only on the second attempt).
/// RU: Запрос с одним повтором: часть соединений через zapret иногда не устанавливается с первого раза.
///     Возвращает (время или ошибка, прошёл ли только со второй попытки).
async fn probe_with_retry(client: &reqwest::Client, host: &str) -> (Result<u32, String>, bool) {
    match probe(client, host).await {
        Ok(ms) => (Ok(ms), false),
        Err(_) => {
            let retry = probe(client, host).await;
            let is_flaky = retry.is_ok();
            (retry, is_flaky)
        }
    }
}

async fn check_target(clients: &(reqwest::Client, reqwest::Client), host: &str) -> TargetResult {
    let ((r12, flaky12), (r13, flaky13)) =
        futures_util::join!(probe_with_retry(&clients.0, host), probe_with_retry(&clients.1, host));
    let error = r13.as_ref().err().or(r12.as_ref().err()).cloned();
    crate::debug!("check {host}: tls1.2={r12:?} tls1.3={r13:?} flaky12={flaky12} flaky13={flaky13}");
    TargetResult { host: host.to_string(), tls12_ms: r12.ok(), tls13_ms: r13.ok(), error, flaky: flaky12 || flaky13 }
}

async fn check_service(clients: &(reqwest::Client, reqwest::Client), service: &ServiceTargets) -> ServiceCheck {
    let targets = join_all(service.hosts.iter().map(|host| check_target(clients, host))).await;

    let times: Vec<u32> = targets.iter().flat_map(|t| [t.tls12_ms, t.tls13_ms]).flatten().collect();
    let total = u32::try_from(targets.len() * 2).unwrap_or(u32::MAX);
    let passed = u32::try_from(times.len()).unwrap_or(u32::MAX);
    let status = CheckStatus::from_counts(passed, total);
    let latency_ms = (!times.is_empty()).then(|| times.iter().sum::<u32>() / passed);
    let flaky = u32::try_from(targets.iter().filter(|t| t.flaky).count()).unwrap_or(u32::MAX);

    ServiceCheck { id: service.id.into(), name: t(service.name), status, passed, total, flaky, latency_ms, targets }
}

/// EN: Checks all services in parallel. Takes at most 2 × REQUEST_TIMEOUT.
/// RU: Проверяет все сервисы параллельно. Занимает не дольше 2 × REQUEST_TIMEOUT.
pub async fn run() -> Result<Vec<ServiceCheck>, String> {
    run_only(None).await
}

/// EN: Checks the services with these ids (None — all). RU: Проверяет сервисы с этими идентификаторами (None — все).
pub async fn run_only(ids: Option<&[String]>) -> Result<Vec<ServiceCheck>, String> {
    let clients = (client(Tls::V12)?, client(Tls::V13)?);
    let selected = SERVICES
        .iter()
        .filter(|service| ids.is_none_or(|ids| ids.iter().any(|id| id == service.id)));
    Ok(join_all(selected.map(|service| check_service(&clients, service))).await)
}

#[cfg(test)]
mod tests {
    /// EN: Manual network check: `cargo test print_live_checks -- --ignored --nocapture`
    /// RU: Ручная проверка по сети: `cargo test print_live_checks -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn print_live_checks() {
        let results = tauri::async_runtime::block_on(super::run()).expect("run");
        for service in results {
            println!(
                "{} {:?} {}/{} flaky={} {:?} ms",
                service.name, service.status, service.passed, service.total, service.flaky, service.latency_ms
            );
            for target in service.targets {
                println!(
                    "  {} tls1.2={:?} tls1.3={:?} flaky={} {:?}",
                    target.host, target.tls12_ms, target.tls13_ms, target.flaky, target.error
                );
            }
        }
    }
}
