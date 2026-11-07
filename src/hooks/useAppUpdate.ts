import { getVersion } from '@tauri-apps/api/app';
import { relaunch } from '@tauri-apps/plugin-process';
import { check, DownloadEvent, Update } from '@tauri-apps/plugin-updater';
import { useCallback, useEffect, useRef, useState } from 'react';
import { errorText } from '../utils/format';

/** EN: How often to check in the background. RU: Как часто проверять в фоне. */
const MS_IN_DAY = 86400000;
const CHECKED_AT_KEY = 'netpilot.appUpdateCheckedAt';

export interface IAppUpdateInfo {
    version: string;
    /** EN: Release text from GitHub. RU: Текст релиза с GitHub. */
    notes: string;
}

export interface IAppUpdate {
    /** EN: Version of the running app. RU: Версия запущенного приложения. */
    version: string | null;
    /** EN: A newer version, null — none found. RU: Более новая версия, null — не найдено. */
    available: IAppUpdateInfo | null;
    /** EN: Time of the last check, ms; null — not checked yet. RU: Время последней проверки, мс; null — ещё не проверяли. */
    checkedAt: number | null;
    isChecking: boolean;
    isInstalling: boolean;
    /** EN: Download progress 0–1, null — unknown size. RU: Прогресс скачивания 0–1, null — размер неизвестен. */
    progress: number | null;
    /** EN: No releases yet on GitHub. RU: На GitHub ещё нет релизов. */
    hasNoReleases: boolean;
    error: string | null;
    checkNow: () => void;
    install: () => void;
}

function readCheckedAt(): number | null {
    try {
        const value = Number(window.localStorage.getItem(CHECKED_AT_KEY));
        return value > 0 ? value : null;
    } catch {
        return null;
    }
}

function writeCheckedAt(value: number): void {
    try {
        window.localStorage.setItem(CHECKED_AT_KEY, String(value));
    } catch {
        // EN: Without storage the check just runs again next time. RU: Без хранилища проверка просто повторится в следующий раз.
    }
}

/**
 * EN: NetPilot's own updates from GitHub Releases (the updater plugin checks the signature). Checked once a
 *     day in the background and by the button; installing restarts the app.
 * RU: Обновления самого NetPilot из GitHub Releases (плагин обновлений проверяет подпись). Проверка раз в
 *     сутки в фоне и по кнопке; установка перезапускает приложение.
 */
export function useAppUpdate(): IAppUpdate {
    const [version, setVersion] = useState<string | null>(null);
    const [available, setAvailable] = useState<IAppUpdateInfo | null>(null);
    const [checkedAt, setCheckedAt] = useState<number | null>(readCheckedAt);
    const [isChecking, setIsChecking] = useState(false);
    const [isInstalling, setIsInstalling] = useState(false);
    const [progress, setProgress] = useState<number | null>(null);
    const [hasNoReleases, setHasNoReleases] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const update = useRef<Update | null>(null);

    const run = useCallback((isSilent: boolean) => {
        setIsChecking(true);
        setError(null);
        check()
            .then((found: Update | null) => {
                update.current = found;
                setAvailable(found ? { version: found.version, notes: found.body ?? '' } : null);
                setHasNoReleases(false);
                const now = Date.now();
                setCheckedAt(now);
                writeCheckedAt(now);
            })
            .catch((e: unknown) => {
                const text = errorText(e);
                // EN: Before the first release latest.json is not there. RU: До первого релиза latest.json нет.
                const isMissing = /404|valid release JSON/i.test(text);
                setHasNoReleases(isMissing);
                if (!isSilent && !isMissing) {
                    setError(text);
                }
            })
            .finally(() => setIsChecking(false));
    }, []);

    useEffect(() => {
        getVersion()
            .then(setVersion)
            .catch(() => undefined);
        const last = readCheckedAt();
        if (last === null || Date.now() - last > MS_IN_DAY) {
            run(true);
        }
    }, [run]);

    const checkNow = useCallback(() => run(false), [run]);

    const install = useCallback(() => {
        const found = update.current;
        if (!found) {
            return;
        }
        setIsInstalling(true);
        setError(null);
        let total = 0;
        let downloaded = 0;
        found
            .downloadAndInstall((event: DownloadEvent) => {
                if (event.event === 'Started') {
                    total = event.data.contentLength ?? 0;
                } else if (event.event === 'Progress') {
                    downloaded += event.data.chunkLength;
                    setProgress(total > 0 ? downloaded / total : null);
                }
            })
            .then(() => relaunch())
            .catch((e: unknown) => {
                setError(errorText(e));
                setIsInstalling(false);
                setProgress(null);
            });
    }, []);

    return {
        version,
        available,
        checkedAt,
        isChecking,
        isInstalling,
        progress,
        hasNoReleases,
        error,
        checkNow,
        install,
    };
}
