import { useCallback, useEffect, useState } from 'react';
import {
    cancelVideoCheck,
    getLastDeepPick,
    IDeepPickProgress,
    IDeepPickResult,
    IVideoCheckProgress,
    onDeepPickProgress,
    onVideoCheckProgress,
    runDeepPick,
    VideoQuality,
} from '../api/diag';
import { errorText } from '../utils/format';

export interface IDeepPick {
    result: IDeepPickResult | null;
    progress: IDeepPickProgress | null;
    /** EN: The video check of the current strategy. RU: Проверка видео текущей стратегии. */
    check: IVideoCheckProgress | null;
    isRunning: boolean;
    error: string | null;
    start: (quality: VideoQuality, allStrategies: boolean) => Promise<void>;
    cancel: () => void;
}

/**
 * EN: Deep pick: the video check with each leader of the quick pick. Lives at the App level, so the
 *     pick is not lost when switching tabs.
 * RU: Глубокий подбор: проверка видео с каждым лидером быстрого подбора. Живёт на уровне App, чтобы
 *     подбор не терялся при переключении вкладок.
 */
export function useDeepPick(onFinished: () => void): IDeepPick {
    const [result, setResult] = useState<IDeepPickResult | null>(null);
    const [progress, setProgress] = useState<IDeepPickProgress | null>(null);
    const [check, setCheck] = useState<IVideoCheckProgress | null>(null);
    const [isRunning, setIsRunning] = useState(false);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        getLastDeepPick()
            .then(setResult)
            .catch(() => setResult(null));
    }, []);

    const start = useCallback(
        async (quality: VideoQuality, allStrategies: boolean): Promise<void> => {
            setIsRunning(true);
            setProgress(null);
            setCheck(null);
            setError(null);
            const unlistenPick = await onDeepPickProgress((next: IDeepPickProgress) => {
                setProgress(next);
                setCheck(null);
            });
            const unlistenCheck = await onVideoCheckProgress(setCheck);
            try {
                setResult(await runDeepPick(quality, allStrategies));
            } catch (e) {
                setError(errorText(e));
            } finally {
                unlistenPick();
                unlistenCheck();
                setIsRunning(false);
                setProgress(null);
                setCheck(null);
                // EN: The pick stops and starts the service again — re-read the state.
                // RU: Подбор останавливает и снова запускает службу — перечитываем состояние.
                onFinished();
            }
        },
        [onFinished]
    );

    const cancel = useCallback(() => {
        cancelVideoCheck().catch((e: unknown) => setError(errorText(e)));
    }, []);

    return { result, progress, check, isRunning, error, start, cancel };
}
