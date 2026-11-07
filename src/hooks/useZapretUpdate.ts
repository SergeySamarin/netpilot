import { useCallback, useState } from 'react';
import {
    checkZapretUpdate,
    IDownloadProgress,
    IUpdateResult,
    IZapretOverview,
    onDownloadProgress,
    onUpdateStage,
    UpdateStage,
    zapretRollback,
    zapretUpdate,
} from '../api/zapret';
import { errorText } from '../utils/format';

export interface IZapretUpdate {
    isRunning: boolean;
    isChecking: boolean;
    stage: UpdateStage | null;
    progress: IDownloadProgress | null;
    result: IUpdateResult | null;
    error: string | null;
    check: (isSilent: boolean) => Promise<void>;
    start: () => Promise<void>;
    rollback: () => Promise<void>;
    dismiss: () => void;
}

/**
 * EN: zapret update, rollback and update check. Lives at the App level, so it is not lost when switching tabs.
 * RU: Обновление zapret, откат и проверка обновлений. Живёт на уровне App, чтобы не терялось при смене вкладок.
 */
export function useZapretUpdate(onOverview: (overview: IZapretOverview) => void): IZapretUpdate {
    const [isRunning, setIsRunning] = useState(false);
    const [isChecking, setIsChecking] = useState(false);
    const [stage, setStage] = useState<UpdateStage | null>(null);
    const [progress, setProgress] = useState<IDownloadProgress | null>(null);
    const [result, setResult] = useState<IUpdateResult | null>(null);
    const [error, setError] = useState<string | null>(null);

    const check = useCallback(
        async (isSilent: boolean): Promise<void> => {
            setIsChecking(true);
            if (!isSilent) {
                setError(null);
            }
            try {
                onOverview(await checkZapretUpdate());
            } catch (e) {
                // EN: A background check fails quietly (no internet etc.). RU: Фоновая проверка падает молча (нет сети и т. п.).
                if (!isSilent) {
                    setError(errorText(e));
                }
            } finally {
                setIsChecking(false);
            }
        },
        [onOverview]
    );

    const start = useCallback(async (): Promise<void> => {
        setIsRunning(true);
        setError(null);
        setResult(null);
        setStage(null);
        setProgress(null);
        const unlistenStage = await onUpdateStage(setStage);
        const unlistenProgress = await onDownloadProgress(setProgress);
        try {
            const updateResult = await zapretUpdate();
            setResult(updateResult);
            onOverview(updateResult.overview);
        } catch (e) {
            setError(errorText(e));
        } finally {
            unlistenStage();
            unlistenProgress();
            setIsRunning(false);
            setStage(null);
            setProgress(null);
        }
    }, [onOverview]);

    const rollback = useCallback(async (): Promise<void> => {
        setIsRunning(true);
        setError(null);
        setResult(null);
        setStage('apply');
        try {
            onOverview(await zapretRollback());
        } catch (e) {
            setError(errorText(e));
        } finally {
            setIsRunning(false);
            setStage(null);
        }
    }, [onOverview]);

    const dismiss = useCallback(() => {
        setResult(null);
        setError(null);
    }, []);

    return {
        isRunning,
        isChecking,
        stage,
        progress,
        result,
        error,
        check,
        start,
        rollback,
        dismiss,
    };
}
