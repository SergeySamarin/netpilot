//! EN: Deep pick: the video check (a short playlist) with each leader of the quick pick. The strategies are
//!     held by an administrator process (engines/zapret/host.rs, one UAC prompt), the window runs the
//!     player check with each one. The winner is the strategy where video starts fastest and does not hang;
//!     with equal results the one where QUIC works wins: then the browser does not waste time on it.
//! RU: Глубокий подбор: проверка видео (короткий плейлист) с каждым лидером быстрого подбора. Стратегии
//!     держит процесс администратора (engines/zapret/host.rs, одно окно UAC), окно проводит с каждой
//!     проверку плеера. Побеждает стратегия, где видео стартует быстрее всего и не зависает; при равных
//!     результатах — та, где работает QUIC: тогда браузер не тратит на него время.

use super::analysis::{QuicState, VideoStats};
use super::{run_video_check, DiagState, Ending, Quality, Scenario};
use crate::engines::zapret::control::ZapretAction;
use crate::engines::zapret::host::{HostCommand, HostFiles, HostStatus};
use crate::i18n::t;
use crate::privileged;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering as CmpOrdering;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

const PROGRESS_EVENT: &str = "diag-deep-progress";
const POLL: Duration = Duration::from_millis(300);
/// EN: winws.exe starts in a few seconds; longer — the host is stuck. RU: winws.exe стартует за несколько секунд; дольше — хост завис.
const READY_TIMEOUT: Duration = Duration::from_secs(30);
/// EN: Starts closer than this are equal: the difference is network noise. RU: Старты ближе этого равны: разница — шум сети.
const EQUAL_START_MS: u64 = 150;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepStrategyResult {
    pub strategy_id: String,
    /// EN: winws.exe did not start or the check failed. RU: winws.exe не запустился или проверка упала.
    pub error: Option<String>,
    pub stats: Option<VideoStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepPickResult {
    /// EN: Unix seconds. RU: Секунды Unix.
    pub finished_at: u64,
    pub cancelled: bool,
    /// EN: Best first. RU: Лучшие первыми.
    pub results: Vec<DeepStrategyResult>,
    pub recommended: Option<String>,
    /// EN: The service strategy before the pick. RU: Стратегия службы до подбора.
    pub current: Option<String>,
    pub restore_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepPickProgress {
    /// EN: All strategies of the pick, in the order of checking. RU: Все стратегии подбора, в порядке проверки.
    pub candidates: Vec<String>,
    /// EN: Index of the strategy being checked. RU: Номер проверяемой стратегии.
    pub index: usize,
    pub total: usize,
    pub strategy_id: Option<String>,
    /// EN: Results so far, in the order of checking. RU: Результаты на сейчас, в порядке проверки.
    pub results: Vec<DeepStrategyResult>,
}

/// EN: Unsets the cancel flag of the deep pick even on an error. RU: Снимает флаг отмены глубокого подбора даже при ошибке.
struct DeepGuard<'a>(&'a DiagState);

impl Drop for DeepGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut cancel) = self.0.deep_cancel.lock() {
            *cancel = None;
        }
    }
}

pub async fn run(
    app: &AppHandle,
    state: &DiagState,
    zapret_dir: PathBuf,
    candidates: Vec<String>,
    current: Option<String>,
    quality: Quality,
) -> Result<DeepPickResult, String> {
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = state.deep_cancel.lock().map_err(|e| e.to_string())?;
        if slot.is_some() {
            return Err(t("Проверка уже идёт"));
        }
        *slot = Some(cancel.clone());
    }
    let _guard = DeepGuard(state);
    crate::info!("deep pick: start candidates={candidates:?} current={current:?}");

    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
    let temp = std::env::temp_dir();
    let files = HostFiles {
        command: temp.join(format!("netpilot-{stamp}-host-command.json")),
        status: temp.join(format!("netpilot-{stamp}-host-status.json")),
    };
    let action = ZapretAction::StrategyHost { zapret_dir, parent_pid: std::process::id(), files: files.clone() };
    let host_done = Arc::new(AtomicBool::new(false));
    let host_done_in_task = host_done.clone();
    let host = tauri::async_runtime::spawn_blocking(move || {
        let result = privileged::run(&action);
        host_done_in_task.store(true, Ordering::SeqCst);
        result
    });

    let mut results: Vec<DeepStrategyResult> = Vec::new();
    let emit = |index: usize, strategy_id: Option<String>, results: &[DeepStrategyResult]| {
        let progress =
            DeepPickProgress {
                candidates: candidates.clone(),
                index,
                total: candidates.len(),
                strategy_id,
                results: results.to_vec(),
            };
        let _ = app.emit(PROGRESS_EVENT, progress);
    };
    emit(0, None, &results);

    for (index, strategy_id) in candidates.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) || host_done.load(Ordering::SeqCst) {
            break;
        }
        emit(index, Some(strategy_id.clone()), &results);
        let seq = u32::try_from(index + 1).unwrap_or(u32::MAX);
        write_command(&files.command, &HostCommand { seq, strategy: Some(strategy_id.clone()) });
        let status = wait_for_status(&files.status, seq, &host_done, &cancel).await;
        let result = match status {
            None => break,
            Some(HostStatus { error: Some(error), .. }) => {
                DeepStrategyResult { strategy_id: strategy_id.clone(), error: Some(error), stats: None }
            }
            Some(_) => {
                match run_video_check(app, state, Scenario::Pick, quality, true, Ending::DeepPick).await {
                    Ok(check) if check.cancelled => break,
                    Ok(check) => DeepStrategyResult {
                        strategy_id: strategy_id.clone(),
                        error: None,
                        stats: Some(check.trace.stats),
                    },
                    Err(error) => DeepStrategyResult { strategy_id: strategy_id.clone(), error: Some(error), stats: None },
                }
            }
        };
        crate::debug!("deep pick {strategy_id}: {result:?}");
        results.push(result);
    }
    let cancelled = cancel.load(Ordering::SeqCst) || results.len() < candidates.len();
    emit(results.len(), None, &results);

    // EN: Stop: the host stops winws.exe and restores the service. RU: Остановка: хост гасит winws.exe и возвращает службу.
    let stop_seq = u32::try_from(candidates.len() + 1).unwrap_or(u32::MAX);
    write_command(&files.command, &HostCommand { seq: stop_seq, strategy: None });
    let host_result = host.await.map_err(|e| e.to_string()).and_then(|result| result);
    let final_status = read_status(&files.status);
    let _ = fs::remove_file(&files.command);
    let _ = fs::remove_file(&files.status);
    host_result?;

    results.sort_by(compare);
    let recommended = recommend(&results, current.as_deref(), &candidates);
    let finished_at = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default();
    let restore_error = final_status.and_then(|status| status.restore_error);
    crate::info!("deep pick: done cancelled={cancelled} recommended={recommended:?} restore_error={restore_error:?}");
    Ok(DeepPickResult { finished_at, cancelled, results, recommended, current, restore_error })
}

fn write_command(path: &Path, command: &HostCommand) {
    if let Ok(text) = serde_json::to_string(command) {
        let _ = fs::write(path, text);
    }
}

fn read_status(path: &Path) -> Option<HostStatus> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// EN: Waits until the host has executed command `seq`. None — the host is gone, stuck or the pick is cancelled.
/// RU: Ждёт, пока хост выполнит команду `seq`. None — хоста нет, он завис или подбор отменён.
async fn wait_for_status(path: &Path, seq: u32, host_done: &AtomicBool, cancel: &AtomicBool) -> Option<HostStatus> {
    let mut waited = Duration::ZERO;
    // EN: The first command also waits for the UAC prompt. RU: Первая команда ждёт ещё и окно UAC.
    let limit = if seq == 1 { READY_TIMEOUT * 4 } else { READY_TIMEOUT };
    while waited < limit {
        if host_done.load(Ordering::SeqCst) || cancel.load(Ordering::SeqCst) {
            return None;
        }
        if let Some(status) = read_status(path).filter(|status| status.seq == seq) {
            return Some(status);
        }
        tokio::time::sleep(POLL).await;
        waited += POLL;
    }
    crate::warn!("deep pick: no answer from the strategy host for command {seq}");
    None
}

fn problems(stats: &VideoStats) -> usize {
    stats.hung + stats.stalls
}

fn quic_works(stats: &VideoStats) -> bool {
    stats.quic == Some(QuicState::Ok)
}

/// EN: Errors last, then fewer hangs and stops, then QUIC works, then a faster start.
/// RU: Ошибки в конце, дальше меньше зависаний и остановок, дальше работает QUIC, дальше быстрее старт.
fn compare(a: &DeepStrategyResult, b: &DeepStrategyResult) -> CmpOrdering {
    match (&a.stats, &b.stats) {
        (None, None) => CmpOrdering::Equal,
        (None, Some(_)) => CmpOrdering::Greater,
        (Some(_), None) => CmpOrdering::Less,
        (Some(x), Some(y)) => problems(x)
            .cmp(&problems(y))
            .then(quic_works(y).cmp(&quic_works(x)))
            .then(x.start_median_ms.unwrap_or(u64::MAX).cmp(&y.start_median_ms.unwrap_or(u64::MAX)))
            .then(x.start_worst_ms.unwrap_or(u64::MAX).cmp(&y.start_worst_ms.unwrap_or(u64::MAX))),
    }
}

/// EN: As good as the best one: the same hangs and QUIC, the start differs by network noise only.
/// RU: Не хуже лучшей: те же зависания и QUIC, старт отличается только на шум сети.
fn is_as_good(stats: &VideoStats, best: &VideoStats) -> bool {
    problems(stats) == problems(best)
        && quic_works(stats) == quic_works(best)
        && stats.start_median_ms.unwrap_or(u64::MAX) <= best.start_median_ms.unwrap_or(u64::MAX).saturating_add(EQUAL_START_MS)
}

/// EN: Among the ones as good as the best: the current strategy (no reason to switch), otherwise the first in
///     the order of checking — that is the order of the quick pick, so the choice does not jump between runs.
/// RU: Среди тех, что не хуже лучшей: текущая стратегия (незачем переключаться), иначе первая в порядке
///     проверки — это порядок быстрого подбора, поэтому выбор не прыгает от прогона к прогону.
fn recommend(sorted: &[DeepStrategyResult], current: Option<&str>, order: &[String]) -> Option<String> {
    let best_stats = sorted.first()?.stats.as_ref()?;
    let good: Vec<&DeepStrategyResult> = sorted
        .iter()
        .filter(|result| result.stats.as_ref().is_some_and(|stats| is_as_good(stats, best_stats)))
        .collect();
    if let Some(current) = good.iter().find(|result| Some(result.strategy_id.as_str()) == current) {
        return Some(current.strategy_id.clone());
    }
    good.iter()
        .min_by_key(|result| order.iter().position(|id| *id == result.strategy_id).unwrap_or(usize::MAX))
        .map(|result| result.strategy_id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, hung: usize, quic: QuicState, median: u64) -> DeepStrategyResult {
        DeepStrategyResult {
            strategy_id: id.to_string(),
            error: None,
            stats: Some(VideoStats {
                starts: 6,
                start_median_ms: Some(median),
                start_worst_ms: Some(median + 100),
                hung,
                quic: Some(quic),
                ..VideoStats::default()
            }),
        }
    }

    #[test]
    fn quic_wins_among_equal_and_hangs_go_last() {
        let mut results = vec![
            row("slow-hangs", 2, QuicState::Ok, 300),
            DeepStrategyResult { strategy_id: "broken".into(), error: Some("x".into()), stats: None },
            row("no-quic", 0, QuicState::Failed, 350),
            row("quic", 0, QuicState::Ok, 400),
        ];
        results.sort_by(compare);
        let order: Vec<&str> = results.iter().map(|result| result.strategy_id.as_str()).collect();
        assert_eq!(order, vec!["quic", "no-quic", "slow-hangs", "broken"]);
        let order = ids(&["slow-hangs", "broken", "no-quic", "quic"]);
        assert_eq!(recommend(&results, None, &order).as_deref(), Some("quic"));
    }

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn keeps_the_current_one_if_as_good() {
        let order = ids(&["best", "current"]);
        let mut results = vec![row("best", 0, QuicState::Ok, 400), row("current", 0, QuicState::Ok, 500)];
        results.sort_by(compare);
        assert_eq!(recommend(&results, Some("current"), &order).as_deref(), Some("current"));
        let mut results = vec![row("best", 0, QuicState::Ok, 400), row("current", 0, QuicState::Failed, 400)];
        results.sort_by(compare);
        assert_eq!(recommend(&results, Some("current"), &order).as_deref(), Some("best"));
    }

    #[test]
    fn close_starts_follow_the_quick_pick_order() {
        let order = ids(&["first", "second"]);
        let mut results = vec![row("first", 0, QuicState::Failed, 680), row("second", 0, QuicState::Failed, 660)];
        results.sort_by(compare);
        assert_eq!(results[0].strategy_id, "second");
        assert_eq!(recommend(&results, None, &order).as_deref(), Some("first"));
    }
}
