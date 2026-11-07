//! EN: Quick strategy pick: each strategy runs as a plain winws.exe process, then the services are
//!     checked. The first round checks all strategies, the next rounds only the leaders: one check is
//!     noisy, and a middle strategy may look like a leader by chance. Runs with administrator rights.
//!     Talks to the app window via files: progress (rewritten after each check), result (at the end) and
//!     a cancel flag (the window creates it on "Stop").
//! RU: Быстрый подбор стратегии: каждая стратегия запускается как обычный процесс winws.exe, после чего
//!     проверяется, открываются ли сервисы. Первый круг проверяет все стратегии, следующие — только
//!     лидеров: одна проверка шумная, и средняя стратегия может случайно выглядеть лидером. Выполняется
//!     с правами администратора. Связь с окном — через файлы: прогресс (перезаписывается после каждой
//!     проверки), результат (в конце) и флаг отмены (окно создаёт его по кнопке «Прервать»).

use super::control::{self, ZapretAction};
use super::files::read_version;
use super::service::{self, ServiceState};
use super::strategy;
use crate::checks::{self, CheckStatus, ServiceCheck};
use crate::i18n::{t, tf};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

/// EN: How long to wait after starting winws.exe before checking the services.
/// RU: Сколько ждать после запуска winws.exe, прежде чем проверять сервисы.
const WARMUP: Duration = Duration::from_secs(2);
/// EN: Pause after winws.exe exits, so the WinDivert driver releases the traffic.
/// RU: Пауза после завершения winws.exe, чтобы драйвер WinDivert отпустил трафик.
const COOLDOWN: Duration = Duration::from_millis(300);
/// EN: How many rounds the leaders get (the first round included).
/// RU: Сколько кругов проходят лидеры (вместе с первым).
const ROUNDS: u32 = 3;
/// EN: Leader after the first round: failed requests plus retries at most this.
/// RU: Лидер после первого круга: непрошедших запросов плюс повторов не больше этого.
const MAX_PROBLEMS_FOR_LEADER: u32 = 1;
/// EN: Equivalent to the best: failed requests plus retries over all rounds at most the best one's plus this.
/// RU: Равноценна лучшей: непрошедших запросов плюс повторов за все круги не больше, чем у лучшей, плюс это.
const EQUAL_PROBLEMS_SLACK: u32 = 1;
/// EN: Equivalent to the best: latency at most this percent of the best one's.
/// RU: Равноценна лучшей: задержка не больше этого процента от задержки лучшей.
const EQUAL_LATENCY_PERCENT: u32 = 130;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickFiles {
    pub progress: PathBuf,
    pub result: PathBuf,
    pub cancel: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickProgress {
    /// EN: Checks done and planned over the whole pick, for the time estimate. During the first round the
    ///     leader checks are projected, after it they are exact.
    /// RU: Сделано проверок и запланировано за весь подбор, для оценки времени. Во время первого круга
    ///     проверки лидеров — прикидка, после него — точно.
    pub done: usize,
    pub total: usize,
    pub current: Option<String>,
    pub round: u32,
    pub rounds: u32,
    /// EN: Checks done and planned in the current round. RU: Сделано проверок и запланировано в текущем круге.
    pub round_done: usize,
    pub round_total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyResult {
    pub strategy_id: String,
    /// EN: winws.exe failed to start with this strategy. Then services is empty.
    /// RU: Ошибка запуска winws.exe с этой стратегией. Тогда services пустой.
    pub error: Option<String>,
    /// EN: Per service, summed over all rounds; the addresses are from the last round.
    /// RU: По каждому сервису, суммой за все круги; адреса — из последнего круга.
    pub services: Vec<ServiceCheck>,
    /// EN: How many requests passed out of all (all services, TLS versions and rounds).
    /// RU: Сколько запросов прошло из всех (по всем сервисам, версиям TLS и кругам).
    pub passed: u32,
    pub total: u32,
    /// EN: How many addresses opened only on the second attempt, over all rounds.
    /// RU: Сколько адресов открылись только со второй попытки, за все круги.
    pub flaky: u32,
    /// EN: Median over the rounds. RU: Медиана по кругам.
    pub latency_ms: Option<u32>,
    #[serde(default = "one_round")]
    pub rounds: u32,
    /// EN: Works as well as the recommended one. RU: Работает так же, как рекомендованная.
    #[serde(default)]
    pub equivalent: bool,
}

fn one_round() -> u32 {
    1
}

impl StrategyResult {
    fn problems(&self) -> u32 {
        self.total - self.passed + self.flaky
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickResult {
    /// EN: Finish time, Unix seconds. RU: Время окончания, секунды Unix.
    pub finished_at: u64,
    pub cancelled: bool,
    /// EN: The recommended one goes first, then the equivalent ones, then the rest by share of failed
    ///     requests, retries per round and latency.
    /// RU: Первой идёт рекомендованная, за ней равноценные, дальше остальные по доле непрошедших
    ///     запросов, повторам на круг и задержке.
    pub results: Vec<StrategyResult>,
    /// EN: None — nothing works, there is nothing to recommend.
    /// RU: None — ничего не работает, рекомендовать нечего.
    #[serde(default)]
    pub recommended: Option<String>,
    /// EN: Error while restoring the service to its previous state.
    /// RU: Ошибка при возврате службы в исходное состояние.
    pub restore_error: Option<String>,
    /// EN: zapret folder and version the pick was made for: in another folder or version the strategies differ.
    /// RU: Папка и версия zapret, для которых делался подбор: в другой папке или версии стратегии другие.
    #[serde(default)]
    pub zapret_dir: Option<PathBuf>,
    #[serde(default)]
    pub version: Option<String>,
}

pub fn run(zapret_dir: &Path, strategy_ids: &[String], preferred: &[String], files: &PickFiles) -> Result<(), String> {
    control::prepare(zapret_dir)?;

    let was_running = matches!(service::query().state, ServiceState::Running);
    if was_running {
        control::execute(&ZapretAction::Stop)?;
    }
    // EN: The driver must be loaded from the folder being tested, not from an old one.
    // RU: Драйвер должен загрузиться из проверяемой папки, а не из старой.
    control::reset_driver();
    if service::is_winws_running() {
        return Err(t("winws.exe запущен вручную (из bat-файла). Закройте его и повторите подбор"));
    }

    // EN: Results of every round, in the order of strategy_ids.
    // RU: Результаты всех кругов, в порядке strategy_ids.
    let mut samples: Vec<Vec<StrategyResult>> = Vec::new();
    let mut progress = PickProgress {
        done: 0,
        total: strategy_ids.len(),
        current: None,
        round: 1,
        rounds: ROUNDS,
        round_done: 0,
        round_total: strategy_ids.len(),
    };
    let mut cancelled = false;

    for id in strategy_ids {
        if files.cancel.exists() {
            cancelled = true;
            break;
        }
        progress.current = Some(id.clone());
        progress.total = strategy_ids.len() + projected_leader_checks(&samples, strategy_ids.len());
        write_json(&files.progress, &progress);
        let result = test_strategy(zapret_dir, id);
        crate::debug!("pick round 1 {id}: {}", describe(&result));
        samples.push(vec![result]);
        progress.done += 1;
        progress.round_done += 1;
    }

    if !cancelled {
        let leaders: Vec<usize> = samples
            .iter()
            .enumerate()
            .filter(|(_, rounds)| is_leader(&rounds[0]))
            .map(|(index, _)| index)
            .collect();
        progress.total = strategy_ids.len() + leaders.len() * (ROUNDS as usize - 1);
        crate::info!(
            "pick leaders after round 1: {:?}",
            leaders.iter().map(|&index| &strategy_ids[index]).collect::<Vec<_>>()
        );
        'rounds: for round in 2..=ROUNDS {
            progress.round = round;
            progress.round_done = 0;
            progress.round_total = leaders.len();
            for &index in &leaders {
                if files.cancel.exists() {
                    cancelled = true;
                    break 'rounds;
                }
                progress.current = Some(strategy_ids[index].clone());
                write_json(&files.progress, &progress);
                let result = test_strategy(zapret_dir, &strategy_ids[index]);
                crate::debug!("pick round {round} {}: {}", strategy_ids[index], describe(&result));
                samples[index].push(result);
                progress.done += 1;
                progress.round_done += 1;
            }
        }
    }

    let restore_error = if was_running { control::execute(&ZapretAction::Start).err() } else { None };

    let mut results: Vec<StrategyResult> = samples.into_iter().filter_map(merge).collect();
    results.sort_by(compare);
    let recommended = recommend(&mut results, strategy_ids, preferred);
    crate::info!(
        "pick done: cancelled={cancelled} preferred={preferred:?} recommended={recommended:?} suitable={:?}",
        results.iter().filter(|result| result.equivalent).map(|result| &result.strategy_id).collect::<Vec<_>>()
    );
    for result in &results {
        crate::debug!("pick result {}: {}", result.strategy_id, describe(result));
    }

    let finished_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let result = PickResult {
        finished_at,
        cancelled,
        results,
        recommended,
        restore_error,
        zapret_dir: Some(zapret_dir.to_path_buf()),
        version: read_version(zapret_dir),
    };
    write_json(&files.result, &result);
    Ok(())
}

fn describe(result: &StrategyResult) -> String {
    match &result.error {
        Some(error) => format!("error: {error}"),
        None => format!(
            "{}/{} flaky={} latency={:?} rounds={}",
            result.passed, result.total, result.flaky, result.latency_ms, result.rounds
        ),
    }
}

fn is_leader(first_round: &StrategyResult) -> bool {
    first_round.error.is_none() && first_round.problems() <= MAX_PROBLEMS_FOR_LEADER
}

/// EN: Leader checks expected in rounds 2 and later: the share of leaders among the strategies checked so far
///     is applied to all of them.
/// RU: Сколько проверок лидеров ожидается в кругах 2 и дальше: доля лидеров среди уже проверенных стратегий
///     переносится на все.
fn projected_leader_checks(first_round: &[Vec<StrategyResult>], strategy_count: usize) -> usize {
    if first_round.is_empty() {
        return 0;
    }
    let leaders = first_round.iter().filter(|rounds| is_leader(&rounds[0])).count();
    (leaders * strategy_count).div_ceil(first_round.len()) * (ROUNDS as usize - 1)
}

/// EN: Sums the rounds of one strategy. If winws.exe failed to start in any round, that round is the result.
/// RU: Складывает круги одной стратегии. Если в каком-то круге winws.exe не запустился, итог — этот круг.
fn merge(samples: Vec<StrategyResult>) -> Option<StrategyResult> {
    if let Some(failed) = samples.iter().find(|sample| sample.error.is_some()) {
        return Some(failed.clone());
    }
    let last = samples.last()?;
    let services = last
        .services
        .iter()
        .map(|service| {
            let same: Vec<&ServiceCheck> = samples
                .iter()
                .filter_map(|sample| sample.services.iter().find(|check| check.id == service.id))
                .collect();
            let passed = same.iter().map(|check| check.passed).sum();
            let total = same.iter().map(|check| check.total).sum();
            ServiceCheck {
                status: CheckStatus::from_counts(passed, total),
                passed,
                total,
                flaky: same.iter().map(|check| check.flaky).sum(),
                latency_ms: median(same.iter().filter_map(|check| check.latency_ms).collect()),
                ..service.clone()
            }
        })
        .collect();

    Some(StrategyResult {
        strategy_id: last.strategy_id.clone(),
        error: None,
        services,
        passed: samples.iter().map(|sample| sample.passed).sum(),
        total: samples.iter().map(|sample| sample.total).sum(),
        flaky: samples.iter().map(|sample| sample.flaky).sum(),
        latency_ms: median(samples.iter().filter_map(|sample| sample.latency_ms).collect()),
        rounds: u32::try_from(samples.len()).unwrap_or(u32::MAX),
        equivalent: false,
    })
}

fn median(mut values: Vec<u32>) -> Option<u32> {
    values.sort_unstable();
    let middle = values.len() / 2;
    match values.len() {
        0 => None,
        len if len % 2 == 1 => Some(values[middle]),
        _ => Some((values[middle - 1] + values[middle]) / 2),
    }
}

/// EN: Shares instead of counts: the leaders have more rounds than the rest.
/// RU: Доли, а не количество: у лидеров больше кругов, чем у остальных.
fn compare(a: &StrategyResult, b: &StrategyResult) -> Ordering {
    let failed_share = |x: &StrategyResult, y: &StrategyResult| u64::from(x.total - x.passed) * u64::from(y.total);
    let flaky_share = |x: &StrategyResult, y: &StrategyResult| u64::from(x.flaky) * u64::from(y.rounds);
    a.error
        .is_some()
        .cmp(&b.error.is_some())
        .then(failed_share(a, b).cmp(&failed_share(b, a)))
        .then(flaky_share(a, b).cmp(&flaky_share(b, a)))
        .then(a.latency_ms.unwrap_or(u32::MAX).cmp(&b.latency_ms.unwrap_or(u32::MAX)))
}

/// EN: Marks the strategies equivalent to the best one and picks one of them: a preferred one (the current
///     strategy, then the previous recommendation), otherwise the first in the zapret list. The latency
///     difference between leaders is network noise, so the choice among them must not depend on it.
///     Puts the recommended one first, then the equivalent ones.
/// RU: Отмечает стратегии, равноценные лучшей, и выбирает из них одну: предпочтительную (текущую, затем
///     прошлую рекомендацию), иначе первую по списку zapret. Разница задержки у лидеров — шум сети, поэтому
///     выбор между ними от неё не зависит. Ставит первой рекомендованную, за ней равноценные.
fn recommend(results: &mut [StrategyResult], strategy_ids: &[String], preferred: &[String]) -> Option<String> {
    let best = results.first().filter(|best| best.error.is_none() && best.passed > 0)?.clone();
    for result in results.iter_mut() {
        result.equivalent = is_equivalent(result, &best);
    }
    let recommended = preferred
        .iter()
        .chain(strategy_ids)
        .find(|id| results.iter().any(|result| result.equivalent && &result.strategy_id == *id))?
        .clone();
    results.sort_by_key(|result| {
        if result.strategy_id == recommended {
            0
        } else if result.equivalent {
            1
        } else {
            2
        }
    });
    results[0].equivalent = false;
    Some(recommended)
}

fn is_equivalent(result: &StrategyResult, best: &StrategyResult) -> bool {
    let is_latency_close = match (result.latency_ms, best.latency_ms) {
        (Some(ms), Some(best_ms)) => u64::from(ms) * 100 <= u64::from(best_ms) * u64::from(EQUAL_LATENCY_PERCENT),
        _ => false,
    };
    result.error.is_none()
        && result.rounds == best.rounds
        && result.problems() <= best.problems() + EQUAL_PROBLEMS_SLACK
        && is_latency_close
}

fn test_strategy(zapret_dir: &Path, strategy_id: &str) -> StrategyResult {
    let failed = |error: String| StrategyResult {
        strategy_id: strategy_id.to_string(),
        error: Some(error),
        services: Vec::new(),
        passed: 0,
        total: 0,
        flaky: 0,
        latency_ms: None,
        rounds: 1,
        equivalent: false,
    };

    let child = match start_winws(zapret_dir, strategy_id) {
        Ok(child) => child,
        Err(e) => return failed(e),
    };

    let services = tauri::async_runtime::block_on(checks::run_only(Some(&pick_services()))).unwrap_or_default();
    stop_winws(child);

    let passed = services.iter().map(|s| s.passed).sum();
    let total = services.iter().map(|s| s.total).sum();
    let flaky = services.iter().map(|s| s.flaky).sum();
    let latencies: Vec<u32> = services.iter().filter_map(|s| s.latency_ms).collect();
    let latency_ms = (!latencies.is_empty())
        .then(|| latencies.iter().sum::<u32>() / u32::try_from(latencies.len()).unwrap_or(1));

    StrategyResult {
        strategy_id: strategy_id.to_string(),
        error: None,
        services,
        passed,
        total,
        flaky,
        latency_ms,
        rounds: 1,
        equivalent: false,
    }
}

/// EN: Services the pick counts. The emote servers are left out: on some networks the provider DNS
///     returns wrong addresses for them, every strategy fails them alike and no leader would be left.
/// RU: Сервисы, которые учитывает подбор. Серверы смайлов не учитываются: в некоторых сетях DNS провайдера
///     отдаёт для них неверные адреса, все стратегии проваливают их одинаково, и лидеров бы не осталось.
fn pick_services() -> Vec<String> {
    vec!["youtube".to_string(), "discord".to_string()]
}

/// EN: Starts winws.exe with the strategy as a plain process and waits WARMUP. Error — it did not start
///     or exited right away.
/// RU: Запускает winws.exe со стратегией как обычный процесс и ждёт WARMUP. Ошибка — не запустился или
///     сразу завершился.
pub(super) fn start_winws(zapret_dir: &Path, strategy_id: &str) -> Result<Child, String> {
    let args = strategy::build_args(zapret_dir, strategy_id)?;
    let bin = zapret_dir.join("bin");
    let mut command = Command::new(bin.join("winws.exe"));
    command.args(&args).current_dir(&bin).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|e| tf("Не удалось запустить winws.exe: {error}", &[("error", &e)]))?;
    sleep(WARMUP);
    if let Ok(Some(status)) = child.try_wait() {
        return Err(tf(
            "winws.exe завершился сразу после запуска (код {code})",
            &[("code", &format!("{:?}", status.code()))],
        ));
    }
    Ok(child)
}

/// EN: Stops winws.exe started by start_winws and waits COOLDOWN.
/// RU: Останавливает winws.exe, запущенный start_winws, и ждёт COOLDOWN.
pub(super) fn stop_winws(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
    sleep(COOLDOWN);
}

fn write_json<T: Serialize>(path: &Path, value: &T) {
    if let Ok(text) = serde_json::to_string(value) {
        let _ = fs::write(path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, passed: u32, flaky: u32, latency_ms: u32, rounds: u32) -> StrategyResult {
        StrategyResult {
            strategy_id: id.to_string(),
            error: None,
            services: Vec::new(),
            passed,
            total: 14 * rounds,
            flaky,
            latency_ms: Some(latency_ms),
            rounds,
            equivalent: false,
        }
    }

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    fn sorted(mut results: Vec<StrategyResult>) -> Vec<StrategyResult> {
        results.sort_by(compare);
        results
    }

    #[test]
    fn projects_leader_checks_from_share() {
        assert_eq!(projected_leader_checks(&[], 22), 0);
        let first_round = vec![vec![row("A", 14, 0, 250, 1)], vec![row("B", 10, 0, 200, 1)]];
        assert_eq!(projected_leader_checks(&first_round, 22), 22);
    }

    #[test]
    fn median_of_even_and_odd() {
        assert_eq!(median(vec![]), None);
        assert_eq!(median(vec![300, 100, 200]), Some(200));
        assert_eq!(median(vec![100, 300]), Some(200));
    }

    #[test]
    fn failed_share_beats_latency_and_rounds() {
        let results = sorted(vec![row("one round", 14, 0, 100, 1), row("three rounds", 41, 0, 300, 3)]);
        assert_eq!(results[0].strategy_id, "one round");
        let results = sorted(vec![row("fast", 13, 0, 100, 1), row("slow", 42, 0, 300, 3)]);
        assert_eq!(results[0].strategy_id, "slow");
    }

    #[test]
    fn recommends_current_among_equivalent() {
        let order = ids(&["A", "B", "C", "D"]);
        let mut results = sorted(vec![
            row("A", 42, 0, 260, 3),
            row("B", 42, 0, 250, 3),
            row("C", 42, 1, 280, 3),
            row("D", 40, 3, 240, 3),
        ]);
        let recommended = recommend(&mut results, &order, &ids(&["C"]));
        assert_eq!(recommended.as_deref(), Some("C"));
        assert_eq!(results[0].strategy_id, "C");
        let equivalent: Vec<&str> =
            results.iter().filter(|r| r.equivalent).map(|r| r.strategy_id.as_str()).collect();
        assert_eq!(equivalent, ["B", "A"]);
    }

    #[test]
    fn falls_back_to_zapret_order() {
        let order = ids(&["A", "B", "C"]);
        let mut results = sorted(vec![row("A", 42, 0, 260, 3), row("B", 42, 0, 250, 3), row("C", 13, 0, 200, 1)]);
        let recommended = recommend(&mut results, &order, &ids(&["C"]));
        assert_eq!(recommended.as_deref(), Some("A"));
    }

    #[test]
    fn equivalent_go_before_the_rest() {
        let order = ids(&["X", "Y", "Z"]);
        let mut results =
            sorted(vec![row("X", 42, 0, 250, 3), row("Y", 42, 0, 340, 3), row("Z", 42, 1, 260, 3)]);
        recommend(&mut results, &order, &[]);
        let order_after: Vec<&str> = results.iter().map(|r| r.strategy_id.as_str()).collect();
        assert_eq!(order_after, ["X", "Z", "Y"]);
    }

    #[test]
    fn slow_leader_is_not_equivalent() {
        let order = ids(&["A", "B"]);
        let mut results = sorted(vec![row("A", 42, 0, 400, 3), row("B", 42, 0, 250, 3)]);
        let recommended = recommend(&mut results, &order, &[]);
        assert_eq!(recommended.as_deref(), Some("B"));
        assert!(!results[1].equivalent);
    }
}
