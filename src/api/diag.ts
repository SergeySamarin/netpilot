import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { call } from './call';

export type VideoScenario = 'video' | 'playlist' | 'long';
export type VideoQuality = '1080p' | '1440p' | '2160p';

export interface IPlayerSpan {
    /** EN: start — until the video plays, stall — the video stopped. RU: start — до начала показа, stall — видео остановилось. */
    kind: 'start' | 'stall';
    /** EN: Times are ms from the check start. RU: Время — мс от начала проверки. */
    startMs: number;
    endMs: number;
    /** EN: Not finished by the end of the check. RU: Не закончился к концу проверки. */
    open: boolean;
    video: string;
}

export type RequestKind = 'ok' | 'slow' | 'hung' | 'failed' | 'cancelled' | 'pending';

export interface IVideoRequest {
    /** EN: Index in servers. RU: Индекс в servers. */
    server: number;
    startMs: number;
    endMs: number;
    /** EN: Wait for the response. RU: Ожидание ответа. */
    waitMs: number;
    kind: RequestKind;
    quic: boolean;
    error: string | null;
}

export type QuicState = 'ok' | 'failed' | 'off' | 'unknown';

export interface IVideoServer {
    host: string;
    quic: QuicState;
    quicError: string | null;
}

export interface IVideoStats {
    starts: number;
    startMedianMs: number | null;
    startWorstMs: number | null;
    stalls: number;
    stallMs: number;
    requests: number;
    /** EN: Hung and failed requests. RU: Зависшие и упавшие запросы. */
    hung: number;
    slow: number;
    quic: QuicState | null;
}

export interface IVideoTrace {
    durationMs: number;
    player: IPlayerSpan[];
    /** EN: Playlist switches, ms. RU: Переключения плейлиста, мс. */
    switches: number[];
    qualities: { atMs: number; quality: string }[];
    errors: { atMs: number; code: number; video: string }[];
    servers: IVideoServer[];
    requests: IVideoRequest[];
    stats: IVideoStats;
}

export interface IVideoCheckResult {
    scenario: VideoScenario;
    quality: VideoQuality;
    quic: boolean;
    startedAtMs: number;
    cancelled: boolean;
    trace: IVideoTrace;
}

export interface IVideoCheckProgress {
    elapsedMs: number;
    totalMs: number;
    trace: IVideoTrace;
}

/**
 * EN: Opens the player window and returns the result when the scenario ends or is stopped.
 * RU: Открывает окно плеера и возвращает итог, когда сценарий закончится или его остановят.
 */
export function runVideoCheck(
    scenario: VideoScenario,
    quality: VideoQuality,
    quic: boolean,
    lastStage: boolean
): Promise<IVideoCheckResult> {
    return call('diag_video_check', { scenario, quality, quic, lastStage });
}

export function cancelVideoCheck(): Promise<void> {
    return call('diag_video_cancel');
}

export function onVideoCheckProgress(
    handler: (progress: IVideoCheckProgress) => void
): Promise<UnlistenFn> {
    return listen<IVideoCheckProgress>('diag-video-progress', (event) => handler(event.payload));
}

export interface IDeepStrategyResult {
    strategyId: string;
    /** EN: winws.exe did not start or the check failed. RU: winws.exe не запустился или проверка упала. */
    error: string | null;
    stats: IVideoStats | null;
}

export interface IDeepPickResult {
    /** EN: Unix seconds. RU: Секунды Unix. */
    finishedAt: number;
    cancelled: boolean;
    /** EN: Best first. RU: Лучшие первыми. */
    results: IDeepStrategyResult[];
    recommended: string | null;
    /** EN: The service strategy before the pick. RU: Стратегия службы до подбора. */
    current: string | null;
    restoreError: string | null;
}

export interface IDeepPickProgress {
    /** EN: All strategies of the pick, in the order of checking. RU: Все стратегии подбора, в порядке проверки. */
    candidates: string[];
    index: number;
    total: number;
    strategyId: string | null;
    /** EN: Results so far, in the order of checking. RU: Результаты на сейчас, в порядке проверки. */
    results: IDeepStrategyResult[];
}

export function getLastDeepPick(): Promise<IDeepPickResult | null> {
    return call('get_last_deep_pick');
}

/**
 * EN: Checks video with each leader of the last quick pick (one UAC prompt). allStrategies — with all
 *     strategies instead of the leaders (when there was no quick pick).
 * RU: Проверяет видео с каждым лидером последнего быстрого подбора (одно окно UAC). allStrategies — со
 *     всеми стратегиями вместо лидеров (если быстрого подбора не было).
 */
export function runDeepPick(
    quality: VideoQuality,
    allStrategies: boolean
): Promise<IDeepPickResult> {
    return call('diag_deep_pick', { quality, allStrategies });
}

export function onDeepPickProgress(
    handler: (progress: IDeepPickProgress) => void
): Promise<UnlistenFn> {
    return listen<IDeepPickProgress>('diag-deep-progress', (event) => handler(event.payload));
}

export interface IBrowserQuic {
    id: string;
    name: string;
    /** EN: QUIC is turned off by the policy. RU: QUIC выключен политикой. */
    quicOff: boolean;
}

export interface IBrowsersQuic {
    /** EN: Installed supported browsers. RU: Установленные поддерживаемые браузеры. */
    browsers: IBrowserQuic[];
}

export function getBrowsersQuic(): Promise<IBrowsersQuic> {
    return call('get_browsers_quic');
}

/** EN: Administrator rights (UAC). RU: Права администратора (UAC). */
export function setBrowsersQuic(enabled: boolean): Promise<IBrowsersQuic> {
    return call('set_browsers_quic', { enabled });
}
