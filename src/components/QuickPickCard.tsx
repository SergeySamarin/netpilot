import { useState } from 'react';
import { IPickProgress, IPickResult, IServiceCheck, IStrategyResult } from '../api/checks';
import { IDeepPickResult, IDeepStrategyResult } from '../api/diag';
import { strategyTitle } from '../api/zapret';
import { IPickTiming } from '../hooks/useQuickPick';
import { dateLocale, t } from '../i18n';
import { estimateRemainingSeconds, formatRemaining, formatSeconds } from '../utils/format';
import { CheckBadge } from './CheckBadge';

interface IRankedRow {
    row: IStrategyResult;
    rank: number;
}

interface IQuickPickCardProps {
    /** EN: Pick for the current folder and version. RU: Подбор для текущей папки и версии. */
    result: IPickResult | null;
    /**
     * EN: There is a last pick, but for another folder or version.
     * RU: Последний подбор есть, но для другой папки или версии.
     */
    isStale: boolean;
    progress: IPickProgress | null;
    timing: IPickTiming | null;
    isRunning: boolean;
    currentStrategyId: string | null;
    disabled: boolean;
    onCancel: () => void;
    onApply: (strategyId: string) => void;
    /** EN: The last deep pick by video. RU: Последний глубокий подбор по видео. */
    deepResult: IDeepPickResult | null;
    isDeepRunning: boolean;
    /** EN: Opens the "Diagnostics" tab. RU: Открывает вкладку «Диагностика». */
    onOpenDiag: () => void;
}

const MS_IN_SECOND = 1000;
/**
 * EN: How many strategies to check before showing the time estimate.
 * RU: Сколько стратегий проверить, прежде чем показывать оценку оставшегося времени.
 */
const MIN_DONE_FOR_ESTIMATE = 3;
/** EN: How many top strategies to show when collapsed. RU: Сколько лучших стратегий показывать свёрнуто. */
const TOP_COUNT = 5;
/**
 * EN: Bar share of each round, percent: the first round checks all strategies, the next ones only the leaders.
 *     Fixed in advance, so the bar does not jump back when the number of leaders becomes known.
 * RU: Доля полосы каждого круга, в процентах: первый круг проверяет все стратегии, следующие — только лидеров.
 *     Задана заранее, чтобы полоса не откатывалась, когда становится известно число лидеров.
 */
const FIRST_ROUND_SHARE = 70;
const LEADERS_ROUND_SHARE = 15;
const ROUND_SHARES = [FIRST_ROUND_SHARE, LEADERS_ROUND_SHARE, LEADERS_ROUND_SHARE];

function pickPercent(progress: IPickProgress): number {
    const before = ROUND_SHARES.slice(0, progress.round - 1).reduce(
        (sum: number, share: number) => sum + share,
        0
    );
    const share = ROUND_SHARES[progress.round - 1] ?? 0;
    return Math.round(before + (share * progress.roundDone) / Math.max(progress.roundTotal, 1));
}

function progressText(progress: IPickProgress | null): string {
    if (!progress?.current) {
        return t('Жду подтверждения в окне Windows…');
    }
    const values = {
        round: progress.round,
        rounds: progress.rounds,
        name: strategyTitle(progress.current),
        n: progress.roundDone + 1,
        total: progress.roundTotal,
    };
    return progress.round === 1
        ? t('Круг {round} из {rounds} · все стратегии · {name} — {n} из {total}', values)
        : t('Круг {round} из {rounds} · лучшие · {name} — {n} из {total}', values);
}

/**
 * EN: Right card of the tab: pick progress or the result of the last pick.
 * RU: Правая карточка вкладки: ход подбора или результат последнего подбора.
 */
export function QuickPickCard({
    result,
    isStale,
    progress,
    timing,
    isRunning,
    currentStrategyId,
    disabled,
    onCancel,
    onApply,
    deepResult,
    isDeepRunning,
    onOpenDiag,
}: IQuickPickCardProps) {
    const [isExpanded, setIsExpanded] = useState(false);

    if (isRunning) {
        const percent = progress ? pickPercent(progress) : 0;
        const remainingSeconds =
            progress && timing
                ? estimateRemainingSeconds(
                      timing.done,
                      progress.total,
                      timing.startedAt,
                      timing.lastStepAt,
                      MIN_DONE_FOR_ESTIMATE
                  )
                : null;
        let remainingText: string | null = null;
        if (remainingSeconds !== null) {
            remainingText = t('осталось {time}', { time: formatRemaining(remainingSeconds) });
        } else if (progress) {
            remainingText = t('оцениваю время…');
        }
        return (
            <div className="card">
                <h3>
                    {t('Подбор стратегии')}
                    {remainingText && <span className="hint">{remainingText}</span>}
                </h3>
                <p className="muted">{progressText(progress)}</p>
                <div className="progress-wrap">
                    <div className="progress">
                        <div style={{ width: `${percent}%` }} />
                    </div>
                </div>
                {progress?.current && (
                    <p className="muted small hint-text">
                        {progress.round === 1
                            ? t(
                                  'Первый круг — проверяю все стратегии: с какими открываются YouTube и Discord.'
                              )
                            : t(
                                  'Круги 2 и 3 — перепроверяю лучшие стратегии первого круга, чтобы отсеять случайные удачи и сбои сети.'
                              )}
                    </p>
                )}
                <p className="muted small hint-text">
                    {t('Пока идёт подбор, YouTube и Discord в браузере могут ненадолго пропадать.')}
                </p>
                <div className="btns">
                    <button className="btn" onClick={onCancel}>
                        {t('Прервать')}
                    </button>
                </div>
            </div>
        );
    }

    if (!result) {
        return (
            <div className="card">
                <h3>{t('Последний подбор')}</h3>
                <p className="muted">
                    {isStale
                        ? t(
                              'Последний подбор был для другой папки или версии zapret, а в ней другие стратегии. Запустите подбор заново.'
                          )
                        : t(
                              'Подбор ещё не запускался. Нажмите «Подобрать стратегию» — приложение проверит все стратегии и покажет, с какими открываются YouTube и Discord.'
                          )}
                </p>
            </div>
        );
    }

    // EN: One recommendation from both picks: the newer one decides. The deep pick checks the leaders of
    //     the quick one, so its choice is among the rows of this table.
    // RU: Одна рекомендация из обоих подборов: решает более свежий. Глубокий подбор проверяет лидеров
    //     быстрого, поэтому его выбор — среди строк этой таблицы.
    const deepRecommended = deepResult?.recommended ?? null;
    const isDeepNewer =
        deepResult !== null &&
        deepResult.finishedAt > result.finishedAt &&
        result.results.some((row: IStrategyResult) => row.strategyId === deepRecommended);
    const recommendedId = isDeepNewer ? deepRecommended : result.recommended;
    const deepById = new Map(
        (deepResult?.results ?? []).map((row: IDeepStrategyResult) => [row.strategyId, row])
    );
    const finishedAt = formatDateTime(Math.max(result.finishedAt, deepResult?.finishedAt ?? 0));
    // EN: Rank is the place in the rating; the recommended strategy goes first, the current one right
    //     after it, so both are always visible.
    // RU: Место — позиция в рейтинге; рекомендуемая стратегия идёт первой, текущая — сразу за ней,
    //     чтобы обе всегда были видны.
    const ranked = result.results.map((row: IStrategyResult, index: number) => ({
        row,
        rank: index + 1,
    }));
    const first =
        ranked.find((item: IRankedRow) => item.row.strategyId === recommendedId) ?? ranked[0];
    const current = ranked.find(
        (item: IRankedRow) => item !== first && item.row.strategyId === currentStrategyId
    );
    const ordered = [
        first,
        ...(current ? [current] : []),
        ...ranked.filter((item: IRankedRow) => item !== first && item !== current),
    ];
    const rows = isExpanded ? ordered : ordered.slice(0, TOP_COUNT);
    const hasEquivalent = result.results.some((row: IStrategyResult) => row.equivalent);

    return (
        <div className={`card ${isExpanded ? 'wide' : ''}`}>
            <h3>
                {t('Последний подбор')}
                <span className="hint">
                    {finishedAt}
                    {result.cancelled ? ` · ${t('прерван')}` : ''}
                </span>
            </h3>
            {result.restoreError && (
                <div className="banner error">
                    {t('Не удалось вернуть службу в исходное состояние: {error}', {
                        error: result.restoreError,
                    })}
                </div>
            )}
            <div className="pick-table">
                {rows.map(({ row, rank }: IRankedRow) => {
                    const isCurrent = row.strategyId === currentStrategyId;
                    const isRecommended = row.strategyId === recommendedId;
                    const isSuitable = row.equivalent || row.strategyId === result.recommended;
                    const video = deepById.get(row.strategyId)?.stats ?? null;
                    const latencyText =
                        row.latencyMs !== null ? t('{ms} мс', { ms: row.latencyMs }) : '—';
                    return (
                        <div key={row.strategyId} className="pick-row">
                            <span className="pick-mark">
                                {isRecommended ? (
                                    <span className="tag recommended">{t('рекомендуем')}</span>
                                ) : isSuitable ? (
                                    <span className="tag ok">{t('подходит')}</span>
                                ) : (
                                    <span className="tag rank">{rank}</span>
                                )}
                            </span>
                            <span className="name">{strategyTitle(row.strategyId)}</span>
                            {isCurrent && <span className="tag info">{t('текущая')}</span>}
                            <span className="right">
                                {row.error ? (
                                    <span className="tag bad" title={row.error}>
                                        {t('не запустилась')}
                                    </span>
                                ) : (
                                    row.services.map((check: IServiceCheck) => (
                                        <CheckBadge key={check.id} check={check} short />
                                    ))
                                )}
                                {deepResult ? (
                                    <span
                                        className={`video-start ${video && (video.startMedianMs ?? 0) >= SLOW_VIDEO_START_MS ? 'slow' : ''} ${video ? '' : 'none'}`}
                                        title={
                                            video
                                                ? t(
                                                      'Старт видео по глубокому подбору · сайты открываются за {latency}',
                                                      {
                                                          latency: latencyText,
                                                      }
                                                  )
                                                : t(
                                                      'Глубокий подбор эту стратегию не проверял · сайты открываются за {latency}',
                                                      {
                                                          latency: latencyText,
                                                      }
                                                  )
                                        }
                                    >
                                        {video
                                            ? `▶ ${formatSeconds(video.startMedianMs ?? 0)}`
                                            : '—'}
                                    </span>
                                ) : (
                                    <span className="ms">{latencyText}</span>
                                )}
                                <button
                                    className="btn link"
                                    disabled={disabled || isCurrent || row.error !== null}
                                    onClick={() => onApply(row.strategyId)}
                                >
                                    {t('включить')}
                                </button>
                            </span>
                        </div>
                    );
                })}
            </div>
            {hasEquivalent && (
                <p className="muted small hint-text">
                    {t(
                        '«Подходит» — открывает сайты так же надёжно, как рекомендуемая: лидеров подбор проверяет трижды. Разница в задержке между ними — случайный разброс сети.'
                    )}
                </p>
            )}
            <p className="deep-summary">
                {deepSummary(result, deepResult, isDeepNewer, isDeepRunning)}{' '}
                <button className="btn link inline" onClick={onOpenDiag}>
                    {t('В «Диагностику» →')}
                </button>
            </p>
            <div className="btns">
                <button className="btn link" onClick={() => setIsExpanded(!isExpanded)}>
                    {isExpanded
                        ? t('Свернуть')
                        : t('Вся таблица ({count}) →', { count: result.results.length })}
                </button>
            </div>
            {isExpanded && (
                <p className="muted small hint-text">
                    {t(
                        'Жёлтая метка — сайт открылся, но часть адресов только со второй попытки. Быстрый подбор проверяет только открытие сайтов; если видео тормозит — используйте «Диагностику».'
                    )}
                </p>
            )}
        </div>
    );
}

/** EN: A video start slower than this is highlighted. RU: Старт видео медленнее этого подсвечивается. */
const SLOW_VIDEO_START_MS = 1000;

function formatDateTime(unixSeconds: number): string {
    return new Date(unixSeconds * MS_IN_SECOND).toLocaleString(dateLocale(), {
        day: '2-digit',
        month: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
    });
}

/**
 * EN: The line under the table: where the recommendation comes from.
 * RU: Строка под таблицей: откуда взята рекомендация.
 */
function deepSummary(
    quick: IPickResult,
    deep: IDeepPickResult | null,
    isDeepNewer: boolean,
    isDeepRunning: boolean
): string {
    if (isDeepRunning) {
        return t('Идёт глубокий подбор по видео…');
    }
    if (!deep) {
        return t('Если видео тормозит — глубокий подбор проверит лидеров по видео.');
    }
    const values = {
        deep: formatDateTime(deep.finishedAt),
        quick: formatDateTime(quick.finishedAt),
    };
    return isDeepNewer
        ? t(
              'Рекомендация — по видео (глубокий подбор, {deep}). Сайты открываются у всех «подходит» (быстрый подбор, {quick}).',
              values
          )
        : t(
              'Рекомендация — по быстрому подбору ({quick}): он свежее глубокого ({deep}). Глубокий подбор заново проверит новых лидеров по видео.',
              values
          );
}
