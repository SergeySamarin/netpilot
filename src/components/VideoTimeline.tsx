import { IVideoRequest, IVideoServer, IVideoTrace, QuicState, RequestKind } from '../api/diag';
import { t, tk } from '../i18n';
import { formatClock, formatSeconds } from '../utils/format';

interface IVideoTimelineProps {
    trace: IVideoTrace;
    /** EN: Full scale length, ms: during the check — the planned time. RU: Длина шкалы, мс: во время проверки — плановое время. */
    totalMs: number;
    /** EN: Current moment during the check, ms; null — the check is over. RU: Текущий момент во время проверки, мс; null — проверка закончена. */
    nowMs: number | null;
}

const WIDTH = 800;
const LABEL_WIDTH = 168;
const LANE_HEIGHT = 18;
const LANE_GAP = 7;
const TOP = 4;
const AXIS_HEIGHT = 18;
/** EN: Text baseline inside a lane. RU: Базовая линия текста внутри полосы. */
const TEXT_BASELINE = 13;
const AXIS_TEXT_RAISE = 5;
const QUIC_LABEL_GAP = 8;
const NOW_LINE_GAP = 10;
const MIN_BAR = 3;
const BAR_INSET = 3;
const LONG_SCALE_MS = 150000;
const MS_IN_SECOND = 1000;
const SHORT_STEP_SECONDS = 15;
const LONG_STEP_SECONDS = 60;
const MAX_HOST_LENGTH = 22;
/** EN: Kept from the start and the end of a long host name. RU: Сколько оставить от начала и конца длинного имени сервера. */
const HOST_HEAD = 4;
const HOST_TAIL = 14;
const VIDEO_HOST_SUFFIX = '.googlevideo.com';

const REQUEST_COLORS: Record<RequestKind, string> = {
    ok: '#4ade80',
    slow: '#facc15',
    hung: '#ef4444',
    failed: '#ef4444',
    cancelled: '#cbd5e1',
    pending: '#93c5fd',
};

const PLAYED_COLOR = '#bfdbfe';
const START_COLOR = '#fb923c';
const STALL_COLOR = '#ef4444';
const LANE_COLOR = '#f3f5f8';
const NOW_COLOR = '#2563eb';

const QUIC_LABELS: Record<QuicState, { text: string; color: string }> = {
    ok: { text: tk('QUIC ✓'), color: '#15803d' },
    failed: { text: tk('QUIC ✕'), color: '#b91c1c' },
    off: { text: tk('QUIC выкл.'), color: '#6b7385' },
    unknown: { text: '', color: '#6b7385' },
};

function shortHost(host: string): string {
    const name = host.endsWith(VIDEO_HOST_SUFFIX) ? host.slice(0, -VIDEO_HOST_SUFFIX.length) : host;
    // EN: Servers differ at the start (rr2) and at the end (…-8v1s), the middle is the same.
    // RU: Серверы различаются в начале (rr2) и в конце (…-8v1s), середина одинаковая.
    return name.length > MAX_HOST_LENGTH
        ? `${name.slice(0, HOST_HEAD)}…${name.slice(-HOST_TAIL)}`
        : name;
}

function requestTitle(request: IVideoRequest, server: IVideoServer): string {
    const wait = formatSeconds(request.waitMs);
    const texts: Record<RequestKind, string> = {
        ok: t('ответ за {wait}', { wait }),
        slow: t('медленно: ответ за {wait}', { wait }),
        hung: t('завис: нет ответа {wait}', { wait }),
        failed: t('ошибка: {error}', { error: request.error ?? '' }),
        cancelled: t('отменён плеером'),
        pending: t('ждёт ответа {wait}', { wait }),
    };
    return `${server.host}\n${texts[request.kind]}${request.quic ? ' · QUIC' : ''}`;
}

/**
 * EN: Timeline of the video check: the player lane (playing, start, stops) and a lane per video server
 *     with every video request colored by how long the response took.
 * RU: Шкала проверки видео: полоса плеера (играет, старт, остановки) и по полосе на каждый сервер видео,
 *     на которой каждый запрос видео окрашен по тому, сколько ждали ответа.
 */
export function VideoTimeline({ trace, totalMs, nowMs }: IVideoTimelineProps) {
    const plotWidth = WIDTH - LABEL_WIDTH - BAR_INSET;
    const scaleMs = Math.max(totalMs, trace.durationMs, 1);
    const now = nowMs ?? trace.durationMs;
    const lanes = 1 + trace.servers.length;
    const height = TOP + lanes * (LANE_HEIGHT + LANE_GAP) + AXIS_HEIGHT;
    const x = (ms: number): number => LABEL_WIDTH + (Math.min(ms, scaleMs) / scaleMs) * plotWidth;
    const laneY = (index: number): number => TOP + index * (LANE_HEIGHT + LANE_GAP);
    const barWidth = (from: number, to: number): number => Math.max(MIN_BAR, x(to) - x(from));
    const axisY = laneY(lanes - 1) + LANE_HEIGHT + AXIS_HEIGHT - AXIS_TEXT_RAISE;
    const stepMs =
        (scaleMs > LONG_SCALE_MS ? LONG_STEP_SECONDS : SHORT_STEP_SECONDS) * MS_IN_SECOND;
    const ticks: number[] = [];
    for (let ms = 0; ms <= scaleMs; ms += stepMs) {
        ticks.push(ms);
    }

    return (
        <svg className="timeline" viewBox={`0 0 ${WIDTH} ${height}`}>
            <text x={0} y={laneY(0) + TEXT_BASELINE} className="timeline-label strong">
                {t('Плеер')}
            </text>
            <rect
                x={LABEL_WIDTH}
                y={laneY(0)}
                width={plotWidth}
                height={LANE_HEIGHT}
                rx={3}
                fill={LANE_COLOR}
            />
            <rect
                x={LABEL_WIDTH}
                y={laneY(0)}
                width={Math.max(0, x(now) - LABEL_WIDTH)}
                height={LANE_HEIGHT}
                rx={3}
                fill={PLAYED_COLOR}
            />
            {trace.player.map((span) => (
                <rect
                    key={`${span.kind}-${span.startMs}`}
                    x={x(span.startMs)}
                    y={laneY(0)}
                    width={barWidth(span.startMs, span.endMs)}
                    height={LANE_HEIGHT}
                    rx={2}
                    fill={span.kind === 'stall' ? STALL_COLOR : START_COLOR}
                >
                    <title>
                        {span.kind === 'stall'
                            ? t('Видео остановилось: {time}', {
                                  time: formatSeconds(span.endMs - span.startMs),
                              })
                            : t('Старт видео: {time}', {
                                  time: formatSeconds(span.endMs - span.startMs),
                              })}
                    </title>
                </rect>
            ))}

            {trace.servers.map((server, index) => {
                const quic = QUIC_LABELS[server.quic];
                return (
                    <g key={server.host}>
                        <text x={0} y={laneY(index + 1) + TEXT_BASELINE} className="timeline-label">
                            <title>{server.host}</title>
                            {shortHost(server.host)}
                        </text>
                        {quic.text && (
                            <text
                                x={LABEL_WIDTH - QUIC_LABEL_GAP}
                                y={laneY(index + 1) + TEXT_BASELINE}
                                textAnchor="end"
                                className="timeline-label"
                                fill={quic.color}
                            >
                                <title>{server.quicError ?? ''}</title>
                                {t(quic.text)}
                            </text>
                        )}
                        <rect
                            x={LABEL_WIDTH}
                            y={laneY(index + 1)}
                            width={plotWidth}
                            height={LANE_HEIGHT}
                            rx={3}
                            fill={LANE_COLOR}
                        />
                    </g>
                );
            })}

            {trace.requests.map((request) => (
                <rect
                    key={`${request.server}:${request.startMs}:${request.endMs}:${request.kind}`}
                    x={x(request.startMs)}
                    y={laneY(request.server + 1) + BAR_INSET}
                    width={barWidth(request.startMs, request.endMs)}
                    height={LANE_HEIGHT - 2 * BAR_INSET}
                    rx={2}
                    fill={REQUEST_COLORS[request.kind]}
                >
                    <title>{requestTitle(request, trace.servers[request.server])}</title>
                </rect>
            ))}

            {trace.switches.map((ms) => (
                <line
                    key={ms}
                    x1={x(ms)}
                    x2={x(ms)}
                    y1={laneY(0) - 2}
                    y2={laneY(lanes - 1) + LANE_HEIGHT}
                    className="timeline-switch"
                />
            ))}

            {ticks.map((ms, index) => (
                <text
                    key={ms}
                    x={x(ms)}
                    y={axisY}
                    className="timeline-axis"
                    textAnchor={index === 0 ? 'start' : ms === scaleMs ? 'end' : 'middle'}
                >
                    {formatClock(ms / MS_IN_SECOND)}
                </text>
            ))}

            {nowMs !== null && nowMs < scaleMs && (
                <line
                    x1={x(nowMs)}
                    x2={x(nowMs)}
                    y1={0}
                    y2={axisY - NOW_LINE_GAP}
                    stroke={NOW_COLOR}
                    strokeWidth={1.5}
                />
            )}
        </svg>
    );
}

/**
 * EN: Legend under the timeline. RU: Подписи цветов под шкалой.
 */
export function VideoTimelineLegend() {
    const items: { color: string; text: string }[] = [
        { color: PLAYED_COLOR, text: t('видео играет') },
        { color: START_COLOR, text: t('старт видео') },
        { color: STALL_COLOR, text: t('видео остановилось / запрос завис') },
        { color: REQUEST_COLORS.ok, text: t('запрос видео: ответ быстро') },
        { color: REQUEST_COLORS.slow, text: t('медленно, больше 1 с') },
    ];
    return (
        <div className="legend">
            {items.map((item) => (
                <span key={item.text}>
                    <i style={{ background: item.color }} />
                    {item.text}
                </span>
            ))}
            <span>┆ {t('переключение видео')}</span>
        </div>
    );
}
