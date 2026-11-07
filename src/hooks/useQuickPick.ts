import { useCallback, useEffect, useState } from 'react';
import {
    getLastPick,
    IPickProgress,
    IPickResult,
    onPickProgress,
    zapretCancelPick,
    zapretQuickPick,
} from '../api/checks';
import { errorText } from '../utils/format';

/**
 * EN: Pick timestamps for the remaining time estimate.
 * RU: Отметки времени подбора для оценки оставшегося времени.
 */
export interface IPickTiming {
    /**
     * EN: When the first progress came (after UAC confirmation), ms.
     * RU: Когда пришёл первый прогресс (после подтверждения UAC), мс.
     */
    startedAt: number;
    /**
     * EN: When the last checked strategy finished, ms.
     * RU: Когда закончилась последняя проверенная стратегия, мс.
     */
    lastStepAt: number;
    done: number;
}

export interface IQuickPick {
    result: IPickResult | null;
    progress: IPickProgress | null;
    timing: IPickTiming | null;
    isRunning: boolean;
    error: string | null;
    start: () => Promise<void>;
    cancel: () => void;
}

/**
 * EN: Quick strategy pick: start, progress, cancel and the last result.
 *     Lives at the App level, so the pick is not lost when switching tabs.
 * RU: Быстрый подбор стратегии: запуск, прогресс, отмена и последний результат.
 *     Живёт на уровне App, чтобы подбор не терялся при переключении вкладок.
 */
export function useQuickPick(onFinished: () => void): IQuickPick {
    const [result, setResult] = useState<IPickResult | null>(null);
    const [progress, setProgress] = useState<IPickProgress | null>(null);
    const [timing, setTiming] = useState<IPickTiming | null>(null);
    const [isRunning, setIsRunning] = useState(false);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        getLastPick()
            .then(setResult)
            .catch(() => setResult(null));
    }, []);

    const start = useCallback(async (): Promise<void> => {
        setIsRunning(true);
        setProgress(null);
        setTiming(null);
        setError(null);
        const unlisten = await onPickProgress((next: IPickProgress) => {
            setProgress(next);
            setTiming((prev: IPickTiming | null) => {
                const now = Date.now();
                if (!prev) {
                    return { startedAt: now, lastStepAt: now, done: next.done };
                }
                return next.done === prev.done
                    ? prev
                    : { ...prev, lastStepAt: now, done: next.done };
            });
        });
        try {
            setResult(await zapretQuickPick());
        } catch (e) {
            setError(errorText(e));
        } finally {
            unlisten();
            setIsRunning(false);
            setProgress(null);
            setTiming(null);
            // EN: The pick stops and starts the service again — re-read the state.
            // RU: Подбор останавливает и снова запускает службу — перечитываем состояние.
            onFinished();
        }
    }, [onFinished]);

    const cancel = useCallback(() => {
        zapretCancelPick().catch((e: unknown) => setError(errorText(e)));
    }, []);

    return { result, progress, timing, isRunning, error, start, cancel };
}
