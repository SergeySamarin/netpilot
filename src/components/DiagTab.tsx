import { useState } from 'react';
import { IVideoStats, QuicState, VideoQuality, VideoScenario } from '../api/diag';
import { IZapretOverview } from '../api/zapret';
import { IBrowsersQuicState } from '../hooks/useBrowsersQuic';
import { IDeepPick } from '../hooks/useDeepPick';
import { IVideoCheck, IVideoCheckReport, VideoCheckPhase } from '../hooks/useVideoCheck';
import { dateLocale, t, tk } from '../i18n';
import { formatClock, formatSeconds } from '../utils/format';
import { BAD_START_MS, IVerdictLine, SLOW_START_MS, videoVerdict } from '../utils/videoVerdict';
import { BrowsersQuicStrip } from './BrowsersQuicStrip';
import { DeepPickCard } from './DeepPickCard';
import { VideoTimeline, VideoTimelineLegend } from './VideoTimeline';

interface IDiagTabProps {
    check: IVideoCheck;
    deep: IDeepPick;
    browsersQuic: IBrowsersQuicState;
    overview: IZapretOverview;
    /** EN: There is a quick pick for the current folder and version. RU: Есть быстрый подбор для текущей папки и версии. */
    hasQuickPick: boolean;
    /** EN: A pick or an update changes the service now. RU: Сейчас службу меняет подбор или обновление. */
    isBusy: boolean;
    onChange: (overview: IZapretOverview) => void;
    onQuickPick: () => void;
}

interface ISetupCardProps {
    check: IVideoCheck;
    isZapretRunning: boolean;
    isBusy: boolean;
    /** EN: QUIC is on in the user's browsers. RU: QUIC включён в браузерах пользователя. */
    mainQuic: boolean;
}

const MS_IN_SECOND = 1000;
const SECONDS_IN_MINUTE = 60;
const PERCENT = 100;

const SCENARIOS: { id: VideoScenario; title: string; description: string; seconds: number }[] = [
    {
        id: 'video',
        title: tk('Одно видео'),
        description: tk('Смотрим длинное видео. Ловит зависания и остановки'),
        seconds: 120,
    },
    {
        id: 'playlist',
        title: tk('Плейлист / Shorts'),
        description: tk('12 видео подряд по 10 с — каждый раз новый сервер'),
        seconds: 120,
    },
    {
        id: 'long',
        title: tk('Долгое видео'),
        description: tk('Если видео встаёт только через несколько минут'),
        seconds: 300,
    },
];

const QUALITIES: VideoQuality[] = ['1080p', '1440p', '2160p'];

function phaseTitle(quic: boolean): string {
    return quic ? t('С zapret') : t('С zapret, без QUIC');
}

/** EN: QUIC of a stage: the main one is as in the browsers, the compare one is the other way. RU: QUIC этапа: основной — как в браузерах, сравнение — наоборот. */
function phaseQuic(phase: VideoCheckPhase, mainQuic: boolean): boolean {
    return phase === 'main' ? mainQuic : !mainQuic;
}

const QUIC_TEXTS: Record<QuicState, string> = {
    ok: tk('работает'),
    failed: tk('не работает'),
    off: tk('выключен'),
    unknown: tk('не пробовал'),
};

function scenarioOf(id: VideoScenario) {
    return SCENARIOS.find((scenario) => scenario.id === id) ?? SCENARIOS[0];
}

function severity(value: number, warn: number, bad: number): string {
    if (value >= bad) {
        return 'bad';
    }
    return value >= warn ? 'warn' : 'ok';
}

/**
 * EN: "Diagnostics" tab: the YouTube video check in a player window, its progress and the report.
 * RU: Вкладка «Диагностика»: проверка видео YouTube в окне плеера, её ход и итог.
 */
export function DiagTab({
    check,
    deep,
    browsersQuic,
    overview,
    hasQuickPick,
    isBusy,
    onChange,
    onQuickPick,
}: IDiagTabProps) {
    const isZapretRunning = overview.service.state === 'running';
    const browsers = browsersQuic.status?.browsers ?? [];
    const mainQuic = !(browsers.length > 0 && browsers.every((browser) => browser.quicOff));
    return (
        <div className="grid">
            <BrowsersQuicStrip state={browsersQuic} />
            {check.error && <div className="banner error">{check.error}</div>}
            {check.run && <RunCard check={check} />}
            {!check.run && !deep.isRunning && (
                <>
                    {check.report && (
                        <ReportCard report={check.report} onClose={check.clearReport} />
                    )}
                    <SetupCard
                        check={check}
                        isZapretRunning={isZapretRunning}
                        isBusy={isBusy}
                        mainQuic={mainQuic}
                    />
                </>
            )}
            {!check.run && (
                <DeepPickCard
                    deep={deep}
                    quality={check.options.quality}
                    overview={overview}
                    hasQuickPick={hasQuickPick}
                    isBusy={isBusy}
                    onChange={onChange}
                    onQuickPick={onQuickPick}
                />
            )}
        </div>
    );
}

function SetupCard({ check, isZapretRunning, isBusy, mainQuic }: ISetupCardProps) {
    const { options, setOptions } = check;
    const scenario = scenarioOf(options.scenario);
    const totalSeconds = scenario.seconds * (options.compare ? 2 : 1);
    const minutes = scenario.seconds / SECONDS_IN_MINUTE;
    return (
        <div className="card wide">
            <h3>
                {t('Проверка видео YouTube')}
                <span className="hint">{t('как в браузере')}</span>
            </h3>
            <p className="muted small">
                {t(
                    'Приложение откроет плеер YouTube в отдельном окне и посмотрит, какие запросы видео зависают и где видео останавливается. Звук выключен.'
                )}
            </p>
            <div className="scenarios">
                {SCENARIOS.map((item) => (
                    <button
                        key={item.id}
                        className={`scenario ${options.scenario === item.id ? 'on' : ''}`}
                        onClick={() => setOptions({ ...options, scenario: item.id })}
                    >
                        <b>
                            {t(item.title)} ·{' '}
                            {t('{minutes} мин', { minutes: item.seconds / SECONDS_IN_MINUTE })}
                        </b>
                        <span className="muted small">{t(item.description)}</span>
                    </button>
                ))}
            </div>
            <div className="opts">
                <span>
                    {t('Качество')}{' '}
                    <span className="seg">
                        {QUALITIES.map((quality) => (
                            <button
                                key={quality}
                                className={options.quality === quality ? 'on' : ''}
                                onClick={() => setOptions({ ...options, quality })}
                            >
                                {quality}
                            </button>
                        ))}
                    </span>
                </span>
                <label
                    className="opt"
                    title={
                        mainQuic
                            ? t(
                                  'QUIC — быстрый способ соединения, через который браузер обычно качает видео YouTube. Обход для него часто работает хуже, и браузер теряет время на каждом новом видео. Проверка повторится без QUIC, и итог покажет разницу.'
                              )
                            : t(
                                  'Проверка повторится с QUIC — как было до его выключения, — и итог покажет разницу.'
                              )
                    }
                >
                    <input
                        type="checkbox"
                        checked={options.compare}
                        onChange={() => setOptions({ ...options, compare: !options.compare })}
                    />
                    <span>
                        {mainQuic ? t('Сравнить без QUIC') : t('Сравнить с QUIC')}
                        <span className="muted small">
                            {' '}
                            {mainQuic
                                ? t('— не из-за него ли видео тупит на старте (+{minutes} мин)', {
                                      minutes,
                                  })
                                : t('— что дало выключение QUIC (+{minutes} мин)', { minutes })}
                        </span>
                    </span>
                </label>
            </div>
            {!isZapretRunning && (
                <div className="banner info">
                    {t('zapret сейчас выключен — проверка покажет, как видео идёт без обхода.')}
                </div>
            )}
            {!mainQuic && (
                <p className="muted small">
                    {t('QUIC в браузерах выключен — проверка тоже идёт без QUIC, как ваш браузер.')}
                </p>
            )}
            <div className="btns">
                <button
                    className="btn primary"
                    disabled={isBusy}
                    onClick={() => {
                        check.start(mainQuic).catch(() => undefined);
                    }}
                >
                    {t('Начать')} · {formatClock(totalSeconds)}
                </button>
                {isBusy && (
                    <span className="muted small">
                        {t('Дождитесь конца подбора или обновления')}
                    </span>
                )}
            </div>
        </div>
    );
}

function RunCard({ check }: { check: IVideoCheck }) {
    const run = check.run;
    if (!run) {
        return null;
    }
    const scenario = scenarioOf(check.options.scenario);
    const phase = run.phases[run.phaseIndex];
    const progress = run.progress;
    const totalMs = progress?.totalMs ?? scenario.seconds * MS_IN_SECOND;
    const elapsedMs = progress?.elapsedMs ?? 0;
    const allMs = totalMs * run.phases.length;
    const doneMs = run.phaseIndex * totalMs + elapsedMs;
    const stats = progress?.trace.stats;
    return (
        <div className="card wide">
            <h3>
                {t('Проверка видео YouTube')}
                <span className="hint">
                    {t(scenario.title)} · {check.options.quality}
                </span>
            </h3>
            <div>
                <b>
                    {t('Этап {n} из {total}: {name}', {
                        n: run.phaseIndex + 1,
                        total: run.phases.length,
                        name: phaseTitle(phaseQuic(phase, run.mainQuic)),
                    })}
                </b>
            </div>
            <div className="muted small">
                {progress
                    ? t('{elapsed} из {total} · осталось ~{left}', {
                          elapsed: formatClock(elapsedMs / MS_IN_SECOND),
                          total: formatClock(totalMs / MS_IN_SECOND),
                          left: formatClock((allMs - doneMs) / MS_IN_SECOND),
                      })
                    : t('Открываю окно плеера…')}
            </div>
            <div className="progress">
                <div style={{ width: `${(doneMs / allMs) * PERCENT}%` }} />
            </div>
            {stats && (
                <div className="muted small">
                    {t('Запросов видео: {requests} · зависло: {hung} · остановок видео: {stalls}', {
                        requests: stats.requests,
                        hung: stats.hung,
                        stalls: stats.stalls,
                    })}
                </div>
            )}
            {progress && (
                <VideoTimeline trace={progress.trace} totalMs={totalMs} nowMs={elapsedMs} />
            )}
            {progress && <VideoTimelineLegend />}
            <div className="btns">
                <button className="btn" onClick={check.cancel}>
                    {t('Прервать')}
                </button>
            </div>
        </div>
    );
}

function ReportCard({ report, onClose }: { report: IVideoCheckReport; onClose: () => void }) {
    const [view, setView] = useState<VideoCheckPhase>('main');
    const phases = (Object.keys(report.runs) as VideoCheckPhase[]).filter(
        (phase) => report.runs[phase]
    );
    const main = report.runs.main;
    if (!main) {
        return null;
    }
    const verdict = videoVerdict(main, report.runs.compare);
    const shown = report.runs[view] ?? main;
    const time = new Date(report.finishedAt).toLocaleTimeString(dateLocale(), {
        hour: '2-digit',
        minute: '2-digit',
    });
    return (
        <div className="card wide">
            <h3>
                {t('Итог проверки')}
                <span className="hint">
                    {t(scenarioOf(report.options.scenario).title)} · {report.options.quality} ·{' '}
                    {time}
                </span>
            </h3>
            {report.cancelled && (
                <div className="banner info">
                    {t('Проверку прервали — итог по тому, что успели.')}
                </div>
            )}
            {phases.length > 1 && (
                <div className="seg phase-switch">
                    {phases.map((phase) => (
                        <button
                            key={phase}
                            className={view === phase ? 'on' : ''}
                            onClick={() => setView(phase)}
                        >
                            {phaseTitle(phaseQuic(phase, report.mainQuic))}
                        </button>
                    ))}
                </div>
            )}
            <Metrics stats={shown.trace.stats} servers={shown.trace.servers.length} />
            <VideoTimeline trace={shown.trace} totalMs={shown.trace.durationMs} nowMs={null} />
            <VideoTimelineLegend />
            <div className={`verdict ${verdict.problems.length ? '' : 'ok'}`}>
                {verdict.problems.length ? (
                    <>
                        <b>{t('Что нашли')}</b>
                        <VerdictList lines={verdict.problems} />
                        <b>{t('Что делать')}</b>
                        <VerdictList lines={verdict.advice} />
                    </>
                ) : (
                    <>
                        {t('Видео стартует быстро, зависаний и остановок нет.')}
                        <VerdictList lines={verdict.notes} />
                    </>
                )}
            </div>
            {phases.length > 1 && <CompareTable report={report} phases={phases} />}
            <div className="btns">
                <button className="btn link" onClick={onClose}>
                    {t('Закрыть итог')}
                </button>
            </div>
        </div>
    );
}

function Metrics({ stats, servers }: { stats: IVideoStats; servers: number }) {
    const median = stats.startMedianMs ?? 0;
    const worst = stats.startWorstMs ?? 0;
    return (
        <div className="metrics">
            <div className="metric">
                <span className="muted small">{t('Старт видео')}</span>
                <div className={`value ${severity(median, SLOW_START_MS, BAD_START_MS)}`}>
                    {stats.startMedianMs === null ? '—' : formatSeconds(median)}
                </div>
                <span className={`small ${worst >= BAD_START_MS ? 'error-text' : 'muted'}`}>
                    {t('худший {time}', {
                        time: stats.startWorstMs === null ? '—' : formatSeconds(worst),
                    })}
                </span>
            </div>
            <div className="metric">
                <span className="muted small">{t('Остановки видео')}</span>
                <div className={`value ${severity(stats.stalls, 1, 2)}`}>{stats.stalls}</div>
                <span className="muted small">
                    {stats.stalls
                        ? t('всего {time}', { time: formatSeconds(stats.stallMs) })
                        : t('не было')}
                </span>
            </div>
            <div className="metric">
                <span className="muted small">{t('Зависшие запросы')}</span>
                <div className={`value ${severity(stats.hung, 1, 2)}`}>{stats.hung}</div>
                <span className="muted small">{t('из {total}', { total: stats.requests })}</span>
            </div>
            <div className="metric">
                <span className="muted small">QUIC</span>
                <div
                    className={`value ${stats.quic === 'failed' ? 'bad' : stats.quic === 'ok' ? 'ok' : ''}`}
                >
                    {stats.quic ? t(QUIC_TEXTS[stats.quic]) : '—'}
                </div>
                <span className="muted small">
                    {stats.quic === 'off'
                        ? t('для сравнения')
                        : t('серверов видео: {n}', { n: servers })}
                </span>
            </div>
        </div>
    );
}

function VerdictList({ lines }: { lines: IVerdictLine[] }) {
    if (!lines.length) {
        return null;
    }
    return (
        <ul>
            {lines.map((line) => (
                <li key={line.lead}>
                    <b>{line.lead}</b> {line.text}
                </li>
            ))}
        </ul>
    );
}

function CompareTable({
    report,
    phases,
}: {
    report: IVideoCheckReport;
    phases: VideoCheckPhase[];
}) {
    return (
        <table className="compare">
            <thead>
                <tr>
                    <th>{t('Этап')}</th>
                    <th>{t('Старт видео')}</th>
                    <th>{t('Худший старт')}</th>
                    <th>{t('Остановки')}</th>
                    <th>{t('Зависшие запросы')}</th>
                </tr>
            </thead>
            <tbody>
                {phases.map((phase) => {
                    const stats = report.runs[phase]?.trace.stats;
                    if (!stats) {
                        return null;
                    }
                    return (
                        <tr key={phase}>
                            <td>{phaseTitle(phaseQuic(phase, report.mainQuic))}</td>
                            <td>
                                {stats.startMedianMs === null
                                    ? '—'
                                    : formatSeconds(stats.startMedianMs)}
                            </td>
                            <td>
                                {stats.startWorstMs === null
                                    ? '—'
                                    : formatSeconds(stats.startWorstMs)}
                            </td>
                            <td>{stats.stalls}</td>
                            <td>
                                {t('{hung} из {total}', {
                                    hung: stats.hung,
                                    total: stats.requests,
                                })}
                            </td>
                        </tr>
                    );
                })}
            </tbody>
        </table>
    );
}
