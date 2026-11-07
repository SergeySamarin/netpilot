import { call } from './call';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export type ServiceState =
    'notInstalled' | 'running' | 'stopped' | 'starting' | 'stopping' | 'unknown';
export type StartType = 'auto' | 'manual' | 'disabled' | 'unknown';

export interface IServiceInfo {
    state: ServiceState;
    startType: StartType;
    zapretDir: string | null;
    strategyId: string | null;
}

export interface IStrategy {
    id: string;
    title: string;
}

export interface IZapretOverview {
    zapretDir: string | null;
    managed: boolean;
    downloadBaseDir: string | null;
    detectedDir: string | null;
    dirError: string | null;
    version: string | null;
    strategies: IStrategy[];
    service: IServiceInfo;
    serviceFromOtherDir: boolean;
    standaloneWinws: boolean;
    latestVersion: string | null;
    hasUpdate: boolean;
    rollbackVersion: string | null;
    rollbackDir: string | null;
    updateCheckedAt: number | null;
}

export interface IListChange {
    file: string;
    added: number;
    removed: number;
}

export interface IMigrationReport {
    userFiles: string[];
    mergedLists: IListChange[];
    ipsetMode: string | null;
    settings: string[];
    notTransferred: string[];
}

export interface IUpdateResult {
    overview: IZapretOverview;
    version: string;
    report: IMigrationReport;
    strategyMissing: string | null;
}

export type UpdateStage = 'original' | 'download' | 'merge' | 'apply';

export interface IReleaseInfo {
    version: string;
    publishedAt: string;
    downloadUrl: string;
    size: number;
}

export interface IDownloadProgress {
    stage: 'download' | 'extract';
    downloaded: number;
    total: number;
}

export function getZapretOverview(): Promise<IZapretOverview> {
    return call('get_zapret_overview');
}

export function getLatestRelease(): Promise<IReleaseInfo> {
    return call('get_latest_release');
}

/**
 * EN: Downloads the latest release, or the given version (used to test updates).
 * RU: Скачивает последний релиз или указанную версию (для проверки обновлений).
 */
export function downloadZapret(version?: string): Promise<IZapretOverview> {
    return call('download_zapret', { version: version ?? null });
}

export function checkZapretUpdate(): Promise<IZapretOverview> {
    return call('check_zapret_update');
}

export function zapretUpdate(): Promise<IUpdateResult> {
    return call('zapret_update');
}

export function zapretRollback(): Promise<IZapretOverview> {
    return call('zapret_rollback');
}

export function onUpdateStage(handler: (stage: UpdateStage) => void): Promise<UnlistenFn> {
    return listen<UpdateStage>('zapret-update-stage', (event) => handler(event.payload));
}

export function setZapretDir(path: string): Promise<IZapretOverview> {
    return call('set_zapret_dir', { path });
}

export function forgetZapretDir(): Promise<IZapretOverview> {
    return call('forget_zapret_dir');
}

/**
 * EN: Service actions need administrator rights: Windows shows a UAC prompt.
 * RU: Действия со службой требуют прав администратора: Windows покажет окно UAC.
 */
export function zapretInstall(strategyId: string): Promise<IZapretOverview> {
    return call('zapret_install', { strategyId });
}

export function zapretStart(): Promise<IZapretOverview> {
    return call('zapret_start');
}

export function zapretStop(): Promise<IZapretOverview> {
    return call('zapret_stop');
}

export function zapretRemove(): Promise<IZapretOverview> {
    return call('zapret_remove');
}

export function onDownloadProgress(
    handler: (progress: IDownloadProgress) => void
): Promise<UnlistenFn> {
    return listen<IDownloadProgress>('zapret-download-progress', (event) => handler(event.payload));
}

/**
 * EN: "general (ALT2)" -> "ALT2", "general" -> "general". Same logic as in files.rs.
 * RU: "general (ALT2)" -> "ALT2", "general" -> "general". Та же логика, что в files.rs.
 */
export function strategyTitle(id: string): string {
    const match = /\((.+)\)/.exec(id);
    return match ? match[1] : id;
}
