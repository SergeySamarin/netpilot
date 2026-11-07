import {
    IDeepPickProgress,
    IDeepStrategyResult,
    IVideoCheckProgress,
    VideoQuality,
} from '../api/diag';
import { IZapretOverview, strategyTitle, zapretInstall } from '../api/zapret';
import { IDeepPick } from '../hooks/useDeepPick';
import { useServiceAction } from '../hooks/useServiceAction';
import { dateLocale, t } from '../i18n';
import { formatClock, formatSeconds } from '../utils/format';
import { BAD_START_MS } from '../utils/videoVerdict';
import { BROWSERS_QUIC_ID } from './BrowsersQuicStrip';
import { VideoTimeline, VideoTimelineLegend } from './VideoTimeline';

interface IDeepPickCardProps {
    deep: IDeepPick;
    quality: VideoQuality;
    overview: IZapretOverview;
    /** EN: There is a quick pick for the current folder and version. RU: Есть быстрый подбор для текущей папки и версии. */
    hasQuickPick: boolean;
    isBusy: boolean;
    onChange: (overview: IZapretOverview) => void;
    /** EN: Opens the zapret tab and starts the quick pick. RU: Открывает вкладку zapret и запускает быстрый подбор. */
    onQuickPick: () => void;
}

const MS_IN_SECOND = 1000;
const SECONDS_IN_MINUTE = 60;
const PERCENT = 100;
/** EN: One strategy: winws.exe start, the check and the window. RU: Одна стратегия: запуск winws.exe, проверка и окно. */
const STRATEGY_SECONDS = 70;
/** EN: Videos per strategy (Scenario::Pick in Rust). RU: Видео на стратегию (Scenario::Pick в Rust). */
const VIDEOS_PER_STRATEGY = 6;

/**
 * EN: Deep pick: the video check with the leaders of the quick pick, its progress and the table.
 * RU: Глубокий подбор: проверка видео с лидерами быстрого подбора, её ход и таблица.
 */
export function DeepPickCard({
    deep,
    quality,
    overview,
    hasQuickPick,
    isBusy,
    onChange,
    onQuickPick,
}: IDeepPickCardProps) {
    const action = useServiceAction(onChange);
    const currentId = overview.serviceFromOtherDir ? null : overview.service.strategyId;
    const { result } = deep;
    const allMinutes = Math.ceil(
        (overview.strategies.length * STRATEGY_SECONDS) / SECONDS_IN_MINUTE
    );

    function handleInstall(strategyId: string): void {
        const text = t('Устанавливаю службу со стратегией {name}…', {
            name: strategyTitle(strategyId),
        });
        action.run(text, () => zapretInstall(strategyId)).catch(() => undefined);
    }

    function start(allStrategies: boolean): void {
        deep.start(quality, allStrategies).catch(() => undefined);
    }

    let body;
    if (deep.isRunning) {
        body = <DeepProgress progress={deep.progress} check={deep.check} onCancel={deep.cancel} />;
    } else {
        const recommended = result?.recommended ?? null;
        const best = result?.results.find((row) => row.strategyId === recommended);
        const withStats = result?.results.filter((row) => row.stats) ?? [];
        const isQuicDeadEverywhere =
            withStats.length > 0 && withStats.every((row) => row.stats?.quic !== 'ok');
        const isStartBlocked = isBusy || action.busyText !== null;
        body = (
            <>
                <p className="muted small">
                    {t(
                        'Обход по очереди переключается на лидеров быстрого подбора и текущую стратегию, и с каждой плеер проигрывает 6 коротких видео. Побеждает та, где видео стартует быстрее и не зависает, а QUIC работает. Около минуты на стратегию.'
                    )}
                </p>
                {result && (
                    <>
                        {result.cancelled && (
                            <div className="banner info">
                                {t('Подбор прервали — в таблице то, что успели проверить.')}
                            </div>
                        )}
                        {result.restoreError && (
                            <div className="banner error">
                                {t('Не удалось вернуть службу: {error}', {
                                    error: result.restoreError,
                                })}
                            </div>
                        )}
                        <ResultTable
                            rows={result.results}
                            recommended={recommended}
                            currentId={currentId}
                            onInstall={isStartBlocked ? undefined : handleInstall}
                        />
                        {best?.stats && (
                            <div className="verdict ok">
                                {recommended === currentId
                                    ? t(
                                          'Текущая стратегия {name} — одна из лучших для видео, менять не нужно.',
                                          { name: strategyTitle(best.strategyId) }
                                      )
                                    : t('Лучше всего видео идёт на {name}: старт ~{start}.', {
                                          name: strategyTitle(best.strategyId),
                                          start: formatSeconds(best.stats.startMedianMs ?? 0),
                                      })}
                            </div>
                        )}
                        {isQuicDeadEverywhere && (
                            <div className="verdict">
                                <b>{t('QUIC не работает ни на одной стратегии')}</b>{' '}
                                {t(
                                    '— значит, дело не в стратегии: QUIC к серверам видео не проходит совсем. Браузер теряет на нём время при каждом новом видео. Чтобы этого не было, QUIC можно выключить в браузерах.'
                                )}{' '}
                                <button
                                    className="btn link inline"
                                    onClick={() =>
                                        document
                                            .getElementById(BROWSERS_QUIC_ID)
                                            ?.scrollIntoView({ behavior: 'smooth' })
                                    }
                                >
                                    {t('К настройке QUIC ↑')}
                                </button>
                            </div>
                        )}
                    </>
                )}
                {action.busyText && <div className="muted small">{action.busyText}</div>}
                {action.error && <div className="banner error">{action.error}</div>}
                {deep.error && <div className="banner error">{deep.error}</div>}
                {hasQuickPick ? (
                    <div className="btns">
                        <button
                            className={`btn ${result ? '' : 'primary'}`}
                            disabled={isStartBlocked}
                            onClick={() => start(false)}
                        >
                            {result ? t('Подобрать заново') : t('Запустить глубокий подбор')}
                        </button>
                    </div>
                ) : (
                    <div className="banner info">
                        <p>
                            {t(
                                'Быстрый подбор ещё не делали. Без него глубокий подбор проверит все {n} стратегий — это около {minutes} мин. Лучше сначала быстрый подбор (~4 мин): он отберёт 5–6 лидеров, и глубокий займёт ~7 мин.',
                                { n: overview.strategies.length, minutes: allMinutes }
                            )}
                        </p>
                        <div className="btns">
                            <button
                                className="btn primary"
                                disabled={isStartBlocked}
                                onClick={onQuickPick}
                            >
                                {t('Запустить быстрый подбор')}
                            </button>
                            <button
                                className="btn"
                                disabled={isStartBlocked}
                                onClick={() => start(true)}
                            >
                                {t('Всё равно по всем · ~{minutes} мин', { minutes: allMinutes })}
                            </button>
                        </div>
                    </div>
                )}
            </>
        );
    }

    return (
        <div className="card wide">
            <h3>
                {t('Глубокий подбор по видео')}
                <span className="hint">
                    {result && !deep.isRunning
                        ? new Date(result.finishedAt * MS_IN_SECOND).toLocaleString(dateLocale(), {
                              day: '2-digit',
                              month: '2-digit',
                              hour: '2-digit',
                              minute: '2-digit',
                          })
                        : quality}
                </span>
            </h3>
            {body}
        </div>
    );
}

interface IDeepProgressProps {
    progress: IDeepPickProgress | null;
    check: IVideoCheckProgress | null;
    onCancel: () => void;
}

/**
 * EN: The pick in progress: the list of strategies (done, now, waiting), the live timeline of the current one.
 * RU: Подбор в процессе: список стратегий (проверена, сейчас, ждёт), живая шкала текущей.
 */
function DeepProgress({ progress, check, onCancel }: IDeepProgressProps) {
    if (!progress || !progress.strategyId) {
        return (
            <>
                <div className="muted small">{t('Жду подтверждения в окне Windows…')}</div>
                <div className="btns">
                    <button className="btn" onClick={onCancel}>
                        {t('Прервать')}
                    </button>
                </div>
            </>
        );
    }
    const total = Math.max(progress.total, 1);
    const inStrategy = check ? check.elapsedMs / Math.max(check.totalMs, 1) : 0;
    const left = (total - progress.index - inStrategy) * STRATEGY_SECONDS;
    const video = Math.min(
        VIDEOS_PER_STRATEGY,
        Math.max(1, check?.trace.player.filter((span) => span.kind === 'start').length ?? 0)
    );
    const doneById = new Map(progress.results.map((row) => [row.strategyId, row]));
    return (
        <>
            <div className="muted small">
                {t('Стратегия {n} из {total} · осталось ~{left}', {
                    n: progress.index + 1,
                    total,
                    left: formatClock(left),
                })}
            </div>
            <div className="progress">
                <div style={{ width: `${((progress.index + inStrategy) / total) * PERCENT}%` }} />
            </div>
            <ol className="deep-steps">
                {progress.candidates.map((id, index) => {
                    const done = doneById.get(id);
                    const isNow = index === progress.index;
                    let state = t('ждёт');
                    if (done?.stats) {
                        state = t('старт {start} · QUIC {quic}', {
                            start: formatSeconds(done.stats.startMedianMs ?? 0),
                            quic: done.stats.quic === 'ok' ? t('работает') : t('нет'),
                        });
                    } else if (done) {
                        state = done.error ?? '';
                    } else if (isNow) {
                        state = check
                            ? t('включил стратегию, видео {n} из {total}', {
                                  n: video,
                                  total: VIDEOS_PER_STRATEGY,
                              })
                            : t('включаю стратегию…');
                    }
                    return (
                        <li key={id} className={done ? 'done' : isNow ? 'now' : ''}>
                            <span className="step-mark">{done ? '✓' : isNow ? '▶' : '·'}</span>
                            <span className="step-name">{strategyTitle(id)}</span>
                            <span className="muted small">{state}</span>
                        </li>
                    );
                })}
            </ol>
            {check && (
                <>
                    <VideoTimeline
                        trace={check.trace}
                        totalMs={check.totalMs}
                        nowMs={check.elapsedMs}
                    />
                    <VideoTimelineLegend />
                </>
            )}
            <div className="btns">
                <button className="btn" onClick={onCancel}>
                    {t('Прервать')}
                </button>
            </div>
        </>
    );
}

interface IResultTableProps {
    rows: IDeepStrategyResult[];
    recommended: string | null;
    currentId: string | null;
    onInstall?: (strategyId: string) => void;
}

function ResultTable({ rows, recommended, currentId, onInstall }: IResultTableProps) {
    return (
        <table className="compare">
            <thead>
                <tr>
                    <th>{t('Стратегия')}</th>
                    <th>{t('Старт видео')}</th>
                    <th>{t('Худший старт')}</th>
                    <th>{t('Остановки')}</th>
                    <th>{t('Зависшие')}</th>
                    <th>QUIC</th>
                    <th />
                </tr>
            </thead>
            <tbody>
                {rows.map((row) => {
                    const stats = row.stats;
                    return (
                        <tr key={row.strategyId}>
                            <td>
                                {strategyTitle(row.strategyId)}{' '}
                                {row.strategyId === recommended && (
                                    <span className="tag recommended">{t('рекомендуем')}</span>
                                )}{' '}
                                {row.strategyId === currentId && (
                                    <span className="tag info">{t('текущая')}</span>
                                )}
                            </td>
                            {stats ? (
                                <>
                                    <td>
                                        {stats.startMedianMs === null
                                            ? '—'
                                            : formatSeconds(stats.startMedianMs)}
                                    </td>
                                    <td
                                        className={
                                            (stats.startWorstMs ?? 0) >= BAD_START_MS
                                                ? 'error-text'
                                                : ''
                                        }
                                    >
                                        {stats.startWorstMs === null
                                            ? '—'
                                            : formatSeconds(stats.startWorstMs)}
                                    </td>
                                    <td>{stats.stalls}</td>
                                    <td>{stats.hung}</td>
                                    <td>
                                        <span
                                            className={`tag ${stats.quic === 'ok' ? 'ok' : 'bad'}`}
                                        >
                                            {stats.quic === 'ok' ? t('работает') : t('нет')}
                                        </span>
                                    </td>
                                </>
                            ) : (
                                <td colSpan={5} className="error-text">
                                    {row.error}
                                </td>
                            )}
                            <td>
                                {onInstall && stats && row.strategyId !== currentId && (
                                    <button
                                        className="btn link"
                                        onClick={() => onInstall(row.strategyId)}
                                    >
                                        {t('включить')}
                                    </button>
                                )}
                            </td>
                        </tr>
                    );
                })}
            </tbody>
        </table>
    );
}
