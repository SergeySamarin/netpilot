import { IVideoCheckResult, IVideoStats } from '../api/diag';
import { plural, t } from '../i18n';
import { formatSeconds } from './format';

/** EN: A verdict line: the bold beginning and the rest. RU: Строка вывода: жирное начало и остальное. */
export interface IVerdictLine {
    lead: string;
    text: string;
}

export interface IVideoVerdict {
    problems: IVerdictLine[];
    advice: IVerdictLine[];
    /** EN: Worth knowing, but the video is fine. RU: Стоит знать, но видео в порядке. */
    notes: IVerdictLine[];
}

/** EN: Start longer than this is noticeable. RU: Старт дольше этого уже заметен. */
export const SLOW_START_MS = 1500;
/** EN: Start longer than this is a problem. RU: Старт дольше этого — проблема. */
export const BAD_START_MS = 3000;
/**
 * EN: Without QUIC the start is faster by this much and this many times — QUIC costs time.
 * RU: Без QUIC старт быстрее на столько и во столько раз — QUIC отнимает время.
 */
const QUIC_COST_MS = 300;
const QUIC_COST_RATIO = 1.5;
/** EN: The player could not load the YouTube script. RU: Плеер не смог загрузить скрипт YouTube. */
const API_LOAD_ERROR = -1;

/** EN: The video went smoothly. RU: Видео шло гладко. */
export function isSmooth(stats: IVideoStats): boolean {
    return (
        stats.starts > 0 &&
        stats.hung === 0 &&
        stats.stalls === 0 &&
        (stats.startWorstMs ?? 0) < BAD_START_MS
    );
}

/**
 * EN: What was found and what to do, in plain words. `main` — as the user's browsers work,
 *     `compare` — the same scenario with QUIC the other way (if it was run).
 * RU: Что нашли и что делать, простыми словами. `main` — как работают браузеры пользователя,
 *     `compare` — тот же сценарий с QUIC наоборот (если запускали).
 */
export function videoVerdict(main: IVideoCheckResult, compare?: IVideoCheckResult): IVideoVerdict {
    const withQuic = [main, compare].find((run) => run?.quic);
    const noQuic = [main, compare].find((run) => run && !run.quic);
    const { stats, servers, errors } = main.trace;
    const problems: IVerdictLine[] = [];
    const advice: IVerdictLine[] = [];
    const notes: IVerdictLine[] = [];

    if (errors.some((error) => error.code === API_LOAD_ERROR)) {
        problems.push({
            lead: t('Плеер YouTube не загрузился.'),
            text: t('Сайт youtube.com не открывается — видео проверить нельзя.'),
        });
    }
    if (stats.starts === 0 && errors.length === 0) {
        problems.push({
            lead: t('Видео так и не запустилось.'),
            text: '',
        });
    }
    const quicFailed = servers.filter((server) => server.quic === 'failed').length;
    if (stats.quic === 'failed') {
        // EN: Without other problems a broken QUIC is not felt: the browser uses the ordinary connection.
        // RU: Без других проблем сломанный QUIC не заметен: браузер идёт обычным соединением.
        (isSmooth(stats) ? notes : problems).push({
            lead: t('QUIC к серверам видео не работает'),
            text: t('({failed} из {total}) — браузер качает видео обычным соединением.', {
                failed: quicFailed,
                total: servers.length,
            }),
        });
    }
    if (stats.hung > 0) {
        const worstHung = Math.max(
            ...main.trace.requests
                .filter((request) => request.kind === 'hung' || request.kind === 'failed')
                .map((request) => request.waitMs)
        );
        problems.push({
            lead: plural(
                stats.hung,
                '{n} запрос видео завис|{n} запроса видео зависли|{n} запросов видео зависли'
            ),
            text: t('без ответа (до {time}) — плееру приходится ждать или искать другой сервер.', {
                time: formatSeconds(worstHung),
            }),
        });
    }
    if (stats.startWorstMs !== null && stats.startWorstMs >= BAD_START_MS) {
        problems.push({
            lead: t('Видео долго запускается:'),
            text: t('самый долгий старт — {time}.', { time: formatSeconds(stats.startWorstMs) }),
        });
    }
    if (stats.stalls > 0) {
        problems.push({
            lead: plural(
                stats.stalls,
                'Видео остановилось {n} раз|Видео останавливалось {n} раза|Видео останавливалось {n} раз'
            ),
            text: t('во время просмотра, всего на {time}.', { time: formatSeconds(stats.stallMs) }),
        });
    }

    const quicStart = withQuic?.trace.stats.startMedianMs ?? null;
    const noQuicStart = noQuic?.trace.stats.startMedianMs ?? null;
    if (
        quicStart !== null &&
        noQuicStart !== null &&
        quicStart - noQuicStart >= QUIC_COST_MS &&
        quicStart >= noQuicStart * QUIC_COST_RATIO
    ) {
        notes.push({
            lead: t('Без QUIC видео стартует быстрее:'),
            text: t(
                '{fast} против {slow}. Стратегия плохо справляется с QUIC — глубокий подбор ниже проверит, есть ли стратегия, где он работает.',
                { fast: formatSeconds(noQuicStart), slow: formatSeconds(quicStart) }
            ),
        });
    }

    if (problems.length === 0) {
        return { problems, advice, notes };
    }
    // EN: The QUIC advice makes sense only when the browsers use QUIC (main) and the run without it exists.
    // RU: Совет про QUIC имеет смысл, только когда браузеры с QUIC (main) и есть прогон без него.
    if (main.quic && noQuic && isSmooth(noQuic.trace.stats) && !isSmooth(stats)) {
        advice.push({
            lead: t('Без QUIC видео идёт гладко'),
            text: t(
                '— значит, мешает QUIC. Глубокий подбор ниже проверит, есть ли стратегия, при которой QUIC работает.'
            ),
        });
    } else if (main.quic && noQuic && !isSmooth(noQuic.trace.stats)) {
        advice.push({
            lead: t('Без QUIC тоже есть проблемы'),
            text: t(
                '— дело не только в QUIC. Попробуйте другую стратегию: «Подобрать стратегию» на вкладке «Россия · zapret».'
            ),
        });
    } else {
        advice.push({
            lead: t('Попробуйте другую стратегию:'),
            text: t('«Подобрать стратегию» на вкладке «Россия · zapret».'),
        });
    }
    return { problems, advice, notes };
}
