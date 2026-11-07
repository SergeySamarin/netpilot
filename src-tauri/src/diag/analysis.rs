//! EN: Builds the video check result from the player events and the net log: how long each video took
//!     to start, where it stopped, which video requests hung and whether QUIC works on the video servers.
//! RU: Собирает итог проверки видео из событий плеера и сетевого журнала: сколько стартовало каждое
//!     видео, где оно останавливалось, какие запросы видео зависли и работает ли QUIC на серверах видео.

use super::netlog::{NetRequest, NetSummary};
use serde::{Deserialize, Serialize};

/// EN: Longer wait for the response — the request is slow. RU: Ждали ответа дольше — запрос медленный.
const SLOW_MS: u64 = 1000;
/// EN: No response for this long — the request hung. RU: Нет ответа столько времени — запрос завис.
const HUNG_MS: u64 = 3000;

/// EN: Player states of the YouTube IFrame API. RU: Состояния плеера YouTube IFrame API.
const STATE_PLAYING: i64 = 1;
const STATE_BUFFERING: i64 = 3;

/// EN: An event from the player page, time is Unix ms. RU: Событие страницы плеера, время — мс Unix.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlayerEvent {
    /// EN: A video is loaded (start or a playlist switch). RU: Загружено видео (старт или переключение плейлиста).
    Load { at: u64, video: String },
    State { at: u64, state: i64 },
    Quality { at: u64, quality: String },
    Error { at: u64, code: i64 },
    /// EN: The scenario is over: the last video and the moment it stopped at, seconds.
    /// RU: Сценарий закончен: последнее видео и момент, на котором оно остановлено, в секундах.
    Done {
        at: u64,
        #[serde(default)]
        video: Option<String>,
        #[serde(default)]
        position: Option<f64>,
    },
}

impl PlayerEvent {
    fn at(&self) -> u64 {
        match self {
            Self::Load { at, .. }
            | Self::State { at, .. }
            | Self::Quality { at, .. }
            | Self::Error { at, .. }
            | Self::Done { at, .. } => *at,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SpanKind {
    /// EN: From loading a video until it plays. RU: От загрузки видео до начала показа.
    Start,
    /// EN: The video was playing and stopped to load. RU: Видео шло и остановилось на загрузку.
    Stall,
}

/// EN: A time span of the player; times are ms from the check start. RU: Отрезок времени плеера; время — мс от начала проверки.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSpan {
    pub kind: SpanKind,
    pub start_ms: u64,
    pub end_ms: u64,
    /// EN: Not finished by the end of the check. RU: Не закончился к концу проверки.
    pub open: bool,
    /// EN: YouTube video id. RU: Идентификатор видео YouTube.
    pub video: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RequestKind {
    Ok,
    Slow,
    /// EN: No response for HUNG_MS or more. RU: Нет ответа HUNG_MS и дольше.
    Hung,
    /// EN: Ended with a network error before the response. RU: Закончился сетевой ошибкой до ответа.
    Failed,
    /// EN: Cancelled by the player quickly, without waiting long. RU: Быстро отменён плеером, без долгого ожидания.
    Cancelled,
    /// EN: Still waiting for the response, less than HUNG_MS. RU: Ещё ждёт ответа, меньше HUNG_MS.
    Pending,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoRequest {
    /// EN: Index in `servers`. RU: Индекс в `servers`.
    pub server: usize,
    pub start_ms: u64,
    pub end_ms: u64,
    /// EN: Wait for the response headers. RU: Ожидание заголовков ответа.
    pub wait_ms: u64,
    pub kind: RequestKind,
    pub quic: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QuicState {
    Ok,
    Failed,
    /// EN: QUIC is turned off for this check. RU: QUIC выключен для этой проверки.
    Off,
    /// EN: The browser did not try QUIC. RU: Браузер не пробовал QUIC.
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoServer {
    pub host: String,
    pub quic: QuicState,
    pub quic_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityChange {
    pub at_ms: u64,
    pub quality: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerError {
    pub at_ms: u64,
    pub code: i64,
    pub video: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoStats {
    pub starts: usize,
    pub start_median_ms: Option<u64>,
    pub start_worst_ms: Option<u64>,
    pub stalls: usize,
    pub stall_ms: u64,
    pub requests: usize,
    /// EN: Hung and failed requests. RU: Зависшие и упавшие запросы.
    pub hung: usize,
    pub slow: usize,
    pub quic: Option<QuicState>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoTrace {
    pub duration_ms: u64,
    pub player: Vec<PlayerSpan>,
    /// EN: Playlist switches, ms from the start. RU: Переключения плейлиста, мс от начала.
    pub switches: Vec<u64>,
    pub qualities: Vec<QualityChange>,
    pub errors: Vec<PlayerError>,
    pub servers: Vec<VideoServer>,
    pub requests: Vec<VideoRequest>,
    pub stats: VideoStats,
}

/// EN: Builds the trace at moment `now_ms` (Unix): during the check — what is known so far.
/// RU: Собирает трассу на момент `now_ms` (Unix): во время проверки — то, что известно к этому времени.
pub fn build(started_at_ms: u64, now_ms: u64, events: &[PlayerEvent], net: &NetSummary, quic_on: bool) -> VideoTrace {
    let since = |at: u64| at.saturating_sub(started_at_ms);
    let now = since(now_ms);

    let mut events = events.to_vec();
    events.sort_by_key(PlayerEvent::at);
    let mut player = Vec::new();
    let mut switches = Vec::new();
    let mut qualities = Vec::new();
    let mut errors = Vec::new();
    let mut open: Option<(SpanKind, u64)> = None;
    let mut has_played = false;
    let mut video = String::new();
    let mut end = now;
    let close = |open: &mut Option<(SpanKind, u64)>, player: &mut Vec<PlayerSpan>, at: u64, video: &str| {
        if let Some((kind, start_ms)) = open.take() {
            player.push(PlayerSpan { kind, start_ms, end_ms: at.max(start_ms), open: false, video: video.to_string() });
        }
    };
    for event in &events {
        let at = since(event.at());
        match event {
            PlayerEvent::Load { video: next, .. } => {
                if !player.is_empty() || open.is_some() {
                    switches.push(at);
                }
                close(&mut open, &mut player, at, &video);
                open = Some((SpanKind::Start, at));
                video.clone_from(next);
                has_played = false;
            }
            PlayerEvent::State { state, .. } => {
                if *state == STATE_PLAYING {
                    close(&mut open, &mut player, at, &video);
                    has_played = true;
                } else if *state == STATE_BUFFERING && has_played && open.is_none() {
                    open = Some((SpanKind::Stall, at));
                }
            }
            PlayerEvent::Quality { quality, .. } => qualities.push(QualityChange { at_ms: at, quality: quality.clone() }),
            PlayerEvent::Error { code, .. } => {
                // EN: The video can't be played (removed, embedding forbidden) — it is not a network start.
                // RU: Видео нельзя проиграть (удалено, запрещено встраивание) — это не сетевой старт.
                if matches!(open, Some((SpanKind::Start, _))) {
                    open = None;
                }
                errors.push(PlayerError { at_ms: at, code: *code, video: video.clone() });
            }
            PlayerEvent::Done { .. } => end = end.min(at),
        }
    }
    if let Some((kind, start_ms)) = open {
        player.push(PlayerSpan { kind, start_ms, end_ms: end.max(start_ms), open: true, video });
    }

    let mut servers: Vec<VideoServer> = Vec::new();
    let mut requests = Vec::new();
    for request in &net.requests {
        let server = match servers.iter().position(|server| server.host == request.host) {
            Some(index) => index,
            None => {
                servers.push(VideoServer { host: request.host.clone(), quic: QuicState::Unknown, quic_error: None });
                servers.len() - 1
            }
        };
        requests.push(classify(request, server, started_at_ms, now));
    }
    for server in &mut servers {
        let attempts = net.quic.get(&server.host);
        let used = net.requests.iter().any(|request| request.host == server.host && request.quic);
        server.quic = if !quic_on {
            QuicState::Off
        } else if used || attempts.is_some_and(|attempts| attempts.ok > 0) {
            QuicState::Ok
        } else if attempts.is_some_and(|attempts| attempts.failed > 0) {
            QuicState::Failed
        } else {
            QuicState::Unknown
        };
        server.quic_error = attempts.and_then(|attempts| attempts.last_error.clone());
    }

    let stats = stats(&player, &requests, &servers, quic_on);
    VideoTrace { duration_ms: end, player, switches, qualities, errors, servers, requests, stats }
}

fn classify(request: &NetRequest, server: usize, started_at_ms: u64, now: u64) -> VideoRequest {
    let since = |at: u64| at.saturating_sub(started_at_ms);
    let start_ms = since(request.start_ms);
    let end_ms = request.end_ms.map(since).unwrap_or(now).max(start_ms);
    let asked_ms = since(request.sent_ms.unwrap_or(request.start_ms));
    let (wait_ms, kind) = match request.headers_ms.map(since) {
        Some(headers_ms) => {
            let wait = headers_ms.saturating_sub(asked_ms);
            (wait, if wait >= SLOW_MS { RequestKind::Slow } else { RequestKind::Ok })
        }
        None => {
            let wait = end_ms.saturating_sub(start_ms);
            let kind = if wait >= HUNG_MS {
                RequestKind::Hung
            } else if request.end_ms.is_none() {
                RequestKind::Pending
            } else if request.cancelled || request.error.as_deref() == Some("ERR_ABORTED") {
                RequestKind::Cancelled
            } else if request.error.is_some() {
                RequestKind::Failed
            } else {
                RequestKind::Cancelled
            };
            (wait, kind)
        }
    };
    VideoRequest { server, start_ms, end_ms, wait_ms, kind, quic: request.quic, error: request.error.clone() }
}

fn stats(player: &[PlayerSpan], requests: &[VideoRequest], servers: &[VideoServer], quic_on: bool) -> VideoStats {
    let mut starts: Vec<u64> = player
        .iter()
        .filter(|span| span.kind == SpanKind::Start)
        .map(|span| span.end_ms - span.start_ms)
        .collect();
    starts.sort_unstable();
    let stalls: Vec<&PlayerSpan> = player.iter().filter(|span| span.kind == SpanKind::Stall).collect();
    let quic = if !quic_on {
        Some(QuicState::Off)
    } else if servers.iter().any(|server| server.quic == QuicState::Ok) {
        Some(QuicState::Ok)
    } else if servers.iter().any(|server| server.quic == QuicState::Failed) {
        Some(QuicState::Failed)
    } else if servers.is_empty() {
        None
    } else {
        Some(QuicState::Unknown)
    };
    VideoStats {
        starts: starts.len(),
        start_median_ms: starts.get(starts.len() / 2).copied(),
        start_worst_ms: starts.last().copied(),
        stalls: stalls.len(),
        stall_ms: stalls.iter().map(|span| span.end_ms - span.start_ms).sum(),
        requests: requests.iter().filter(|request| request.kind != RequestKind::Cancelled).count(),
        hung: requests
            .iter()
            .filter(|request| matches!(request.kind, RequestKind::Hung | RequestKind::Failed))
            .count(),
        slow: requests.iter().filter(|request| request.kind == RequestKind::Slow).count(),
        quic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::netlog::QuicAttempts;

    const T0: u64 = 1_000_000;

    fn request(host: &str, start: u64, headers: Option<u64>, end: Option<u64>) -> NetRequest {
        NetRequest {
            host: host.to_string(),
            start_ms: T0 + start,
            sent_ms: Some(T0 + start),
            headers_ms: headers.map(|at| T0 + at),
            end_ms: end.map(|at| T0 + at),
            ..NetRequest::default()
        }
    }

    /// EN: Prints the summary of a real net log: `NETPILOT_NETLOG=<path> cargo test real_net_log -- --ignored --nocapture`.
    /// RU: Печатает итог по настоящему сетевому журналу: `NETPILOT_NETLOG=<путь> cargo test real_net_log -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_net_log() {
        let path = std::env::var("NETPILOT_NETLOG").expect("NETPILOT_NETLOG");
        let mut reader = crate::diag::netlog::NetLogReader::new(std::path::Path::new(&path));
        reader.read();
        let net = reader.summary();
        let start = net.requests.first().map(|request| request.start_ms).unwrap_or(0);
        let trace = build(start, u64::MAX / 2, &[], &net, true);
        for request in &trace.requests {
            println!("{:>7} {:<40} {:?} wait={} quic={} {:?}", request.start_ms, trace.servers[request.server].host, request.kind, request.wait_ms, request.quic, request.error);
        }
        println!("{:?}", trace.servers);
        println!("{:?}", trace.stats);
    }

    #[test]
    fn spans_from_player_events() {
        let events = vec![
            PlayerEvent::Load { at: T0, video: "a".into() },
            PlayerEvent::State { at: T0 + 10, state: STATE_BUFFERING },
            PlayerEvent::State { at: T0 + 6500, state: STATE_PLAYING },
            PlayerEvent::State { at: T0 + 30_000, state: STATE_BUFFERING },
            PlayerEvent::State { at: T0 + 34_000, state: STATE_PLAYING },
            PlayerEvent::Load { at: T0 + 60_000, video: "b".into() },
            PlayerEvent::State { at: T0 + 60_400, state: STATE_PLAYING },
            PlayerEvent::Done { at: T0 + 120_000, video: None, position: None },
        ];
        let trace = build(T0, T0 + 121_000, &events, &NetSummary::default(), true);
        assert_eq!(trace.player.len(), 3);
        assert_eq!(trace.switches, vec![60_000]);
        assert_eq!(trace.duration_ms, 120_000);
        assert_eq!(trace.stats.starts, 2);
        assert_eq!(trace.stats.start_worst_ms, Some(6500));
        assert_eq!(trace.stats.start_median_ms, Some(6500));
        assert_eq!(trace.stats.stalls, 1);
        assert_eq!(trace.stats.stall_ms, 4000);
    }

    #[test]
    fn a_video_that_never_started_is_an_open_start() {
        let events = vec![PlayerEvent::Load { at: T0 + 1000, video: "a".into() }];
        let trace = build(T0, T0 + 9000, &events, &NetSummary::default(), true);
        assert_eq!(trace.player.len(), 1);
        assert!(trace.player[0].open);
        assert_eq!(trace.stats.start_worst_ms, Some(8000));
    }

    #[test]
    fn request_kinds_and_quic() {
        let host = "rr1---sn-a.googlevideo.com";
        let mut cancelled = request(host, 100, None, Some(5500));
        cancelled.cancelled = true;
        let mut quick_cancel = request(host, 200, None, Some(300));
        quick_cancel.cancelled = true;
        let mut reset = request(host, 400, None, Some(450));
        reset.error = Some("ERR_CONNECTION_RESET".into());
        let net = NetSummary {
            requests: vec![
                cancelled,
                quick_cancel,
                reset,
                request(host, 6000, Some(6200), Some(7000)),
                request(host, 8000, Some(9500), Some(9600)),
                request(host, 10_000, None, None),
            ],
            quic: [(host.to_string(), QuicAttempts { ok: 0, failed: 2, last_error: Some("ERR_QUIC_PROTOCOL_ERROR".into()) })]
                .into_iter()
                .collect(),
        };
        let trace = build(T0, T0 + 11_000, &[], &net, true);
        let kinds: Vec<RequestKind> = trace.requests.iter().map(|request| request.kind).collect();
        assert_eq!(
            kinds,
            vec![
                RequestKind::Hung,
                RequestKind::Cancelled,
                RequestKind::Failed,
                RequestKind::Ok,
                RequestKind::Slow,
                RequestKind::Pending
            ]
        );
        assert_eq!(trace.stats.hung, 2);
        assert_eq!(trace.stats.requests, 5);
        assert_eq!(trace.servers[0].quic, QuicState::Failed);
        assert_eq!(trace.stats.quic, Some(QuicState::Failed));

        let off = build(T0, T0 + 11_000, &[], &net, false);
        assert_eq!(off.servers[0].quic, QuicState::Off);
    }
}
