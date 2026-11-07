import { call } from './call';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export type CheckStatus = 'ok' | 'partial' | 'fail';

export interface ITargetResult {
    host: string;
    tls12Ms: number | null;
    tls13Ms: number | null;
    error: string | null;
    flaky: boolean;
}

export interface IServiceCheck {
    id: string;
    name: string;
    status: CheckStatus;
    passed: number;
    total: number;
    flaky: number;
    latencyMs: number | null;
    targets: ITargetResult[];
}

export interface IStrategyResult {
    strategyId: string;
    error: string | null;
    services: IServiceCheck[];
    passed: number;
    total: number;
    flaky: number;
    /** EN: Median over the rounds. RU: Медиана по кругам. */
    latencyMs: number | null;
    rounds: number;
    /** EN: Works as well as the recommended one. RU: Работает так же, как рекомендованная. */
    equivalent: boolean;
}

export interface IPickResult {
    finishedAt: number;
    cancelled: boolean;
    /**
     * EN: The recommended one goes first, then the equivalent ones, then the rest.
     * RU: Первой идёт рекомендованная, за ней равноценные, дальше остальные.
     */
    results: IStrategyResult[];
    /** EN: null — nothing works. RU: null — ничего не работает. */
    recommended: string | null;
    restoreError: string | null;
    zapretDir: string | null;
    version: string | null;
}

/**
 * EN: The pick is valid only for the folder and version it was made for: other versions have other strategies.
 * RU: Подбор действителен только для той папки и версии, для которых делался: в других версиях другие стратегии.
 */
export function isPickForDir(
    result: IPickResult,
    zapretDir: string | null,
    version: string | null
): boolean {
    const normalize = (path: string | null): string =>
        (path ?? '').replace(/\\+$/, '').toLowerCase();
    return normalize(result.zapretDir) === normalize(zapretDir) && result.version === version;
}

export interface IPickProgress {
    /**
     * EN: Checks done and planned over the whole pick, for the time estimate. The total grows after
     *     the first round, when the leaders are known.
     * RU: Сделано проверок и запланировано за весь подбор, для оценки времени. Общее число растёт
     *     после первого круга, когда известны лидеры.
     */
    done: number;
    total: number;
    current: string | null;
    round: number;
    rounds: number;
    /** EN: Checks done and planned in the current round. RU: Сделано проверок и запланировано в текущем круге. */
    roundDone: number;
    roundTotal: number;
}

/**
 * EN: Whether YouTube and Discord open now. No administrator rights needed.
 * RU: Открываются ли YouTube и Discord сейчас. Права администратора не нужны.
 */
/** EN: ids — only these services, without ids — all. RU: ids — только эти сервисы, без ids — все. */
export function checkServices(ids?: string[]): Promise<IServiceCheck[]> {
    return call('check_services', { ids: ids ?? null });
}

export function getLastPick(): Promise<IPickResult | null> {
    return call('get_last_pick');
}

/**
 * EN: Quick pick: a round over all strategies, then two more rounds for the leaders. One UAC prompt,
 *     about 4 minutes.
 * RU: Быстрый подбор: круг по всем стратегиям, затем ещё два круга для лидеров. Одно окно UAC,
 *     около 4 минут.
 */
export function zapretQuickPick(): Promise<IPickResult> {
    return call('zapret_quick_pick');
}

export function zapretCancelPick(): Promise<void> {
    return call('zapret_cancel_pick');
}

export function onPickProgress(handler: (progress: IPickProgress) => void): Promise<UnlistenFn> {
    return listen<IPickProgress>('zapret-pick-progress', (event) => handler(event.payload));
}
