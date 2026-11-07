//! EN: Reads a Chromium net log (`--log-net-log`) of the diagnostics player window and collects the video
//!     requests to googlevideo.com: when each one started, when the response headers came, whether it was
//!     cancelled, which protocol carried it; and for each video server — whether QUIC connected.
//!     The file is one JSON object, but each event is on its own line: the first line holds the constants
//!     (event names by number differ between Chromium versions), then one event per line. So the file is
//!     read line by line, also while the browser is still writing it.
//! RU: Читает сетевой журнал Chromium (`--log-net-log`) окна плеера диагностики и собирает запросы видео к
//!     googlevideo.com: когда каждый начался, когда пришли заголовки ответа, был ли отменён, по какому
//!     протоколу шёл; и по каждому серверу видео — установилось ли соединение QUIC.
//!     Файл — один JSON-объект, но каждое событие на своей строке: в первой строке константы (номера
//!     событий отличаются в разных версиях Chromium), дальше по событию на строку. Поэтому файл читается
//!     построчно, в том числе пока браузер его ещё пишет.

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const VIDEO_HOST_SUFFIX: &str = ".googlevideo.com";
const VIDEO_PATH: &str = "/videoplayback";

const PHASE_BEGIN: u64 = 1;
const PHASE_END: u64 = 2;

/// EN: One video request. Times are Unix ms. RU: Один запрос видео. Время — мс Unix.
#[derive(Debug, Clone, Default)]
pub struct NetRequest {
    pub host: String,
    pub start_ms: u64,
    /// EN: The request went to the server (after the connection). RU: Запрос ушёл на сервер (после соединения).
    pub sent_ms: Option<u64>,
    pub headers_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub cancelled: bool,
    /// EN: Chromium error name, e.g. ERR_CONNECTION_RESET. RU: Имя ошибки Chromium, например ERR_CONNECTION_RESET.
    pub error: Option<String>,
    pub quic: bool,
    pub(crate) stream_job: Option<u64>,
}

/// EN: QUIC connection attempts to one video server. RU: Попытки соединения QUIC с одним сервером видео.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuicAttempts {
    pub ok: u32,
    pub failed: u32,
    /// EN: Error of the last failed attempt. RU: Ошибка последней неудачной попытки.
    pub last_error: Option<String>,
}

#[derive(Debug, Default)]
pub struct NetSummary {
    pub requests: Vec<NetRequest>,
    /// EN: Host → QUIC attempts. RU: Сервер → попытки QUIC.
    pub quic: HashMap<String, QuicAttempts>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Constants {
    log_event_types: HashMap<String, u64>,
    net_error: HashMap<String, i64>,
    time_tick_offset: Value,
}

/// EN: Event numbers the parser needs. RU: Номера событий, которые нужны разбору.
struct EventIds {
    url_request_start_job: u64,
    request_alive: u64,
    send_request: u64,
    read_headers: u64,
    cancelled: u64,
    bound_to_job: u64,
    bound_to_quic_session: u64,
    stream_job: u64,
    quic_pool_job: u64,
    quic_pool_job_connect: u64,
}

impl EventIds {
    fn from(types: &HashMap<String, u64>) -> Self {
        // EN: A missing name gets a number no event has. RU: Отсутствующему имени — номер, которого нет ни у одного события.
        let id = |name: &str| types.get(name).copied().unwrap_or(u64::MAX);
        Self {
            url_request_start_job: id("URL_REQUEST_START_JOB"),
            request_alive: id("REQUEST_ALIVE"),
            send_request: id("HTTP_TRANSACTION_SEND_REQUEST"),
            read_headers: id("HTTP_TRANSACTION_READ_HEADERS"),
            cancelled: id("CANCELLED"),
            bound_to_job: id("HTTP_STREAM_REQUEST_BOUND_TO_JOB"),
            bound_to_quic_session: id("HTTP_STREAM_REQUEST_BOUND_TO_QUIC_SESSION"),
            stream_job: id("HTTP_STREAM_JOB"),
            quic_pool_job: id("QUIC_SESSION_POOL_JOB"),
            quic_pool_job_connect: id("QUIC_SESSION_POOL_JOB_CONNECT"),
        }
    }
}

struct Parser {
    ids: EventIds,
    error_names: HashMap<i64, String>,
    tick_offset: i64,
    /// EN: Source id of a request → index in `requests`. RU: Номер источника запроса → индекс в `requests`.
    request_by_source: HashMap<u64, usize>,
    requests: Vec<NetRequest>,
    /// EN: Stream job → it used QUIC. RU: Задание соединения → оно шло по QUIC.
    job_quic: HashMap<u64, bool>,
    /// EN: QUIC pool job → host. RU: Задание пула QUIC → сервер.
    pool_job_host: HashMap<u64, String>,
    quic: HashMap<String, QuicAttempts>,
}

impl Parser {
    fn new(constants: Constants) -> Self {
        let error_names = constants
            .net_error
            .iter()
            .map(|(name, code)| (*code, name.clone()))
            .collect();
        // EN: Chromium writes the offset as a string. RU: Chromium пишет смещение строкой.
        let tick_offset = match &constants.time_tick_offset {
            Value::String(text) => text.parse().unwrap_or(0),
            Value::Number(number) => number.as_i64().unwrap_or(0),
            _ => 0,
        };
        Self {
            ids: EventIds::from(&constants.log_event_types),
            error_names,
            tick_offset,
            request_by_source: HashMap::new(),
            requests: Vec::new(),
            job_quic: HashMap::new(),
            pool_job_host: HashMap::new(),
            quic: HashMap::new(),
        }
    }

    fn error_name(&self, params: &Value) -> Option<String> {
        let code = match params.get("net_error")? {
            Value::Number(number) => number.as_i64()?,
            Value::String(text) => text.parse().ok()?,
            _ => return None,
        };
        if code == 0 {
            return None;
        }
        Some(self.error_names.get(&code).cloned().unwrap_or_else(|| code.to_string()))
    }

    fn feed(&mut self, event: &Value) {
        let Some(kind) = event.get("type").and_then(Value::as_u64) else {
            return;
        };
        let Some(source) = event.pointer("/source/id").and_then(Value::as_u64) else {
            return;
        };
        let phase = event.get("phase").and_then(Value::as_u64).unwrap_or(0);
        let time = event
            .get("time")
            .and_then(Value::as_str)
            .and_then(|text| text.parse::<i64>().ok())
            .map(|tick| u64::try_from(tick + self.tick_offset).unwrap_or(0))
            .unwrap_or(0);
        let null = Value::Null;
        let params = event.get("params").unwrap_or(&null);
        let ids = &self.ids;

        if kind == ids.url_request_start_job {
            let url = params.get("url").and_then(Value::as_str).unwrap_or("");
            if let Some(host) = video_host(url)
                && !self.request_by_source.contains_key(&source)
            {
                self.request_by_source.insert(source, self.requests.len());
                self.requests.push(NetRequest {
                    host,
                    start_ms: time,
                    ..NetRequest::default()
                });
            }
            return;
        }
        if kind == ids.stream_job && phase == PHASE_BEGIN {
            let quic = params.get("using_quic").and_then(Value::as_bool).unwrap_or(false);
            self.job_quic.insert(source, quic);
            return;
        }
        if kind == ids.quic_pool_job && phase == PHASE_BEGIN {
            if let Some(host) = params.get("host").and_then(Value::as_str)
                && host.ends_with(VIDEO_HOST_SUFFIX)
            {
                self.pool_job_host.insert(source, host.to_string());
            }
            return;
        }
        if kind == ids.quic_pool_job_connect && phase == PHASE_END {
            if let Some(host) = self.pool_job_host.get(&source).cloned() {
                let error = self.error_name(params);
                let attempts = self.quic.entry(host).or_default();
                match error {
                    Some(error) => {
                        attempts.failed += 1;
                        attempts.last_error = Some(error);
                    }
                    None => attempts.ok += 1,
                }
            }
            return;
        }

        let Some(&index) = self.request_by_source.get(&source) else {
            return;
        };
        let error = self.error_name(params);
        let request = &mut self.requests[index];
        if kind == ids.send_request && phase == PHASE_BEGIN && request.sent_ms.is_none() {
            request.sent_ms = Some(time);
        } else if kind == ids.read_headers && phase == PHASE_END && request.headers_ms.is_none() {
            if error.is_none() {
                request.headers_ms = Some(time);
            }
        } else if kind == ids.cancelled {
            request.cancelled = true;
        } else if kind == ids.bound_to_quic_session {
            request.quic = true;
        } else if kind == ids.bound_to_job {
            request.stream_job = params.pointer("/source_dependency/id").and_then(Value::as_u64);
        } else if kind == ids.request_alive && phase == PHASE_END {
            request.end_ms = Some(time);
        }
        if let Some(error) = error
            && request.error.is_none()
            && phase == PHASE_END
        {
            request.error = Some(error);
        }
    }

    fn summary(&self) -> NetSummary {
        let requests = self
            .requests
            .iter()
            .map(|request| {
                let mut request = request.clone();
                if let Some(job) = request.stream_job
                    && self.job_quic.get(&job).copied().unwrap_or(false)
                {
                    request.quic = true;
                }
                request
            })
            .collect();
        NetSummary {
            requests,
            quic: self.quic.clone(),
        }
    }
}

/// EN: Host of a video request URL, None for other URLs. RU: Сервер из адреса запроса видео, None для других адресов.
fn video_host(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let (host, path) = rest.split_once('/')?;
    let path = format!("/{path}");
    (host.ends_with(VIDEO_HOST_SUFFIX) && path.starts_with(VIDEO_PATH)).then(|| host.to_string())
}

/// EN: Reads the log as it grows: each read() takes only the new full lines.
/// RU: Читает журнал по мере роста: каждый read() берёт только новые целые строки.
pub struct NetLogReader {
    path: PathBuf,
    offset: u64,
    /// EN: Start of a line whose end is not written yet. RU: Начало строки, конец которой ещё не записан.
    tail: Vec<u8>,
    parser: Option<Parser>,
}

impl NetLogReader {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            offset: 0,
            tail: Vec::new(),
            parser: None,
        }
    }

    /// EN: Reads new lines. No file yet is not an error: the browser creates it a bit later.
    /// RU: Читает новые строки. Отсутствие файла — не ошибка: браузер создаёт его чуть позже.
    pub fn read(&mut self) {
        let Ok(mut file) = File::open(&self.path) else {
            return;
        };
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return;
        }
        let mut chunk = Vec::new();
        if file.read_to_end(&mut chunk).is_err() {
            return;
        }
        self.offset += chunk.len() as u64;
        self.tail.extend_from_slice(&chunk);
        let Some(last_newline) = self.tail.iter().rposition(|byte| *byte == b'\n') else {
            return;
        };
        let complete: Vec<u8> = self.tail.drain(..=last_newline).collect();
        for line in complete.split(|byte| *byte == b'\n') {
            self.feed_line(line);
        }
    }

    fn feed_line(&mut self, line: &[u8]) {
        let text = String::from_utf8_lossy(line);
        let text = text.trim().trim_end_matches(',');
        if text.is_empty() {
            return;
        }
        match &mut self.parser {
            None => {
                // EN: The first line: `{"constants":{...},`. RU: Первая строка: `{"constants":{...},`.
                let Some(body) = text.strip_prefix("{\"constants\":") else {
                    return;
                };
                if let Ok(constants) = serde_json::from_str::<Constants>(body) {
                    self.parser = Some(Parser::new(constants));
                }
            }
            Some(parser) => {
                if let Ok(event) = serde_json::from_str::<Value>(text) {
                    parser.feed(&event);
                }
            }
        }
    }

    pub fn summary(&self) -> NetSummary {
        self.parser.as_ref().map(Parser::summary).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const CONSTANTS: &str = r#"{"constants":{"logEventTypes":{"URL_REQUEST_START_JOB":1,"REQUEST_ALIVE":2,"HTTP_TRANSACTION_SEND_REQUEST":3,"HTTP_TRANSACTION_READ_HEADERS":4,"CANCELLED":5,"HTTP_STREAM_REQUEST_BOUND_TO_JOB":6,"HTTP_STREAM_REQUEST_BOUND_TO_QUIC_SESSION":7,"HTTP_STREAM_JOB":8,"QUIC_SESSION_POOL_JOB":9,"QUIC_SESSION_POOL_JOB_CONNECT":10},"netError":{"ERR_QUIC_PROTOCOL_ERROR":-356,"ERR_ABORTED":-3},"timeTickOffset":"1000000"},"#;

    fn event(kind: u64, source: u64, phase: u64, time: u64, params: &str) -> String {
        format!(
            r#"{{"params":{params},"phase":{phase},"source":{{"id":{source},"type":1}},"time":"{time}","type":{kind}}},"#
        )
    }

    fn write_log(lines: &[String], finished: bool) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("netpilot-netlog-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("netlog-{}.json", lines.len() + usize::from(finished)));
        let mut file = File::create(&path).unwrap();
        writeln!(file, "{CONSTANTS}").unwrap();
        writeln!(file, "\"events\": [").unwrap();
        for line in lines {
            writeln!(file, "{line}").unwrap();
        }
        if finished {
            writeln!(file, "{{}}]}}").unwrap();
        }
        path
    }

    #[test]
    fn collects_video_requests_and_quic() {
        let url = r#"{"method":"POST","url":"https://rr2---sn-abc.googlevideo.com/videoplayback?expire=1"}"#;
        let lines = vec![
            // EN: A hung request over TCP, cancelled. RU: Зависший запрос по TCP, отменён.
            event(1, 10, 0, 100, url),
            event(6, 10, 0, 110, r#"{"source_dependency":{"id":20,"type":17}}"#),
            event(8, 20, 1, 105, r#"{"using_quic":false}"#),
            event(3, 10, 1, 120, "{}"),
            event(4, 10, 1, 130, "{}"),
            event(5, 10, 0, 5500, "{}"),
            event(2, 10, 2, 5510, r#"{"net_error":-3}"#),
            // EN: A QUIC attempt to the same server failed. RU: Попытка QUIC к тому же серверу не удалась.
            event(9, 30, 1, 100, r#"{"host":"rr2---sn-abc.googlevideo.com"}"#),
            event(10, 30, 2, 4100, r#"{"net_error":-356}"#),
            // EN: A normal request over QUIC. RU: Обычный запрос по QUIC.
            event(1, 11, 0, 6000, &url.replace("rr2", "rr1")),
            event(7, 11, 0, 6001, "{}"),
            event(3, 11, 1, 6002, "{}"),
            event(4, 11, 2, 6300, "{}"),
            event(2, 11, 2, 7000, "{}"),
            // EN: Not a video request. RU: Не запрос видео.
            event(1, 12, 0, 6000, r#"{"url":"https://www.youtube.com/youtubei/v1/player"}"#),
        ];
        let path = write_log(&lines, true);
        let mut reader = NetLogReader::new(&path);
        reader.read();
        let summary = reader.summary();

        assert_eq!(summary.requests.len(), 2);
        let hung = &summary.requests[0];
        assert_eq!(hung.host, "rr2---sn-abc.googlevideo.com");
        assert_eq!(hung.start_ms, 1_000_100);
        assert_eq!(hung.headers_ms, None);
        assert!(hung.cancelled);
        assert!(!hung.quic);
        assert_eq!(hung.end_ms, Some(1_005_510));
        assert_eq!(hung.error.as_deref(), Some("ERR_ABORTED"));

        let ok = &summary.requests[1];
        assert!(ok.quic);
        assert_eq!(ok.headers_ms, Some(1_006_300));
        assert!(!ok.cancelled);

        let quic = &summary.quic["rr2---sn-abc.googlevideo.com"];
        assert_eq!(quic.failed, 1);
        assert_eq!(quic.last_error.as_deref(), Some("ERR_QUIC_PROTOCOL_ERROR"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reads_a_growing_file() {
        let url = r#"{"url":"https://rr1---sn-x.googlevideo.com/videoplayback?a=1"}"#;
        let first = vec![event(1, 10, 0, 100, url)];
        let path = write_log(&first, false);
        let mut reader = NetLogReader::new(&path);
        reader.read();
        assert_eq!(reader.summary().requests.len(), 1);
        assert_eq!(reader.summary().requests[0].headers_ms, None);

        // EN: The browser appends a line, the end of the next one is not written yet.
        // RU: Браузер дописывает строку, конец следующей ещё не записан.
        let mut file = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(file, "{}", event(4, 10, 2, 400, "{}")).unwrap();
        write!(file, "{{\"params\":{{}},\"pha").unwrap();
        drop(file);
        reader.read();
        assert_eq!(reader.summary().requests[0].headers_ms, Some(1_000_400));
        let _ = std::fs::remove_file(path);
    }
}
