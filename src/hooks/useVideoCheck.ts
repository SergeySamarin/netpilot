import { useCallback, useRef, useState } from 'react';
import {
    cancelVideoCheck,
    IVideoCheckProgress,
    IVideoCheckResult,
    onVideoCheckProgress,
    runVideoCheck,
    VideoQuality,
    VideoScenario,
} from '../api/diag';
import { errorText } from '../utils/format';

/**
 * EN: main — as the user's browsers work (QUIC as set in them), compare — the same with QUIC the other way.
 * RU: main — как работают браузеры пользователя (QUIC как в них настроен), compare — то же с QUIC наоборот.
 */
export type VideoCheckPhase = 'main' | 'compare';

export interface IVideoCheckOptions {
    scenario: VideoScenario;
    quality: VideoQuality;
    /** EN: Also run the scenario with QUIC the other way. RU: Повторить сценарий с QUIC наоборот. */
    compare: boolean;
}

export interface IVideoCheckRun {
    /** EN: QUIC in the main stage. RU: QUIC на основном этапе. */
    mainQuic: boolean;
    phases: VideoCheckPhase[];
    phaseIndex: number;
    progress: IVideoCheckProgress | null;
}

export interface IVideoCheckReport {
    options: IVideoCheckOptions;
    mainQuic: boolean;
    runs: Partial<Record<VideoCheckPhase, IVideoCheckResult>>;
    cancelled: boolean;
    finishedAt: number;
}

export interface IVideoCheck {
    options: IVideoCheckOptions;
    setOptions: (options: IVideoCheckOptions) => void;
    run: IVideoCheckRun | null;
    report: IVideoCheckReport | null;
    error: string | null;
    /** EN: mainQuic — QUIC is on in the user's browsers. RU: mainQuic — QUIC включён в браузерах пользователя. */
    start: (mainQuic: boolean) => Promise<void>;
    cancel: () => void;
    clearReport: () => void;
}

/**
 * EN: The YouTube video check: phases one after another, progress, cancel and the report.
 *     Lives at the App level, so the check is not lost when switching tabs.
 * RU: Проверка видео YouTube: этапы один за другим, прогресс, отмена и итог.
 *     Живёт на уровне App, чтобы проверка не терялась при переключении вкладок.
 */
export function useVideoCheck(): IVideoCheck {
    const [options, setOptions] = useState<IVideoCheckOptions>({
        scenario: 'video',
        quality: '1440p',
        compare: false,
    });
    const [run, setRun] = useState<IVideoCheckRun | null>(null);
    const [report, setReport] = useState<IVideoCheckReport | null>(null);
    const [error, setError] = useState<string | null>(null);
    const isCancelled = useRef(false);

    const start = useCallback(
        async (mainQuic: boolean): Promise<void> => {
            const phases: VideoCheckPhase[] = options.compare ? ['main', 'compare'] : ['main'];
            const runs: Partial<Record<VideoCheckPhase, IVideoCheckResult>> = {};
            isCancelled.current = false;
            setError(null);
            setReport(null);
            setRun({ mainQuic, phases, phaseIndex: 0, progress: null });
            const unlisten = await onVideoCheckProgress((progress: IVideoCheckProgress) =>
                setRun((prev: IVideoCheckRun | null) => (prev ? { ...prev, progress } : prev))
            );
            try {
                for (const [phaseIndex, phase] of phases.entries()) {
                    setRun({ mainQuic, phases, phaseIndex, progress: null });
                    const result = await runVideoCheck(
                        options.scenario,
                        options.quality,
                        phase === 'main' ? mainQuic : !mainQuic,
                        phaseIndex === phases.length - 1
                    );
                    runs[phase] = result;
                    if (result.cancelled || isCancelled.current) {
                        break;
                    }
                }
                const cancelled =
                    isCancelled.current || Object.values(runs).some((r) => r.cancelled);
                setReport({ options, mainQuic, runs, cancelled, finishedAt: Date.now() });
            } catch (e) {
                setError(errorText(e));
            } finally {
                unlisten();
                setRun(null);
            }
        },
        [options]
    );

    const cancel = useCallback(() => {
        isCancelled.current = true;
        cancelVideoCheck().catch((e: unknown) => setError(errorText(e)));
    }, []);

    const clearReport = useCallback(() => setReport(null), []);

    return { options, setOptions, run, report, error, start, cancel, clearReport };
}
