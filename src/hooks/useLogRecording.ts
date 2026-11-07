import { useCallback, useEffect, useState } from 'react';
import { setUiLogging } from '../api/call';
import {
    getRecordingStatus,
    IRecordingResult,
    IRecordingStatus,
    onRecordingFinished,
    startRecording,
    stopRecording,
} from '../api/recording';
import { errorText } from '../utils/format';

const MS_IN_SECOND = 1000;

export interface ILogRecording {
    /** EN: Start time, Unix ms. null — no recording. RU: Время начала, мс Unix. null — запись не идёт. */
    startedAtMs: number | null;
    maxSeconds: number;
    /** EN: Seconds since the start, updated every second. RU: Секунд с начала, обновляется раз в секунду. */
    elapsedSeconds: number;
    /** EN: Starting or packing the archive. RU: Идёт запуск или сборка архива. */
    isBusy: boolean;
    result: IRecordingResult | null;
    error: string | null;
    start: () => Promise<void>;
    stop: () => Promise<void>;
    /** EN: Closes the result window. RU: Закрывает окно с итогом. */
    clearResult: () => void;
}

/**
 * EN: Log recording for support: start, timer, stop, auto stop by a limit and the resulting file.
 * RU: Запись журнала для поддержки: начало, таймер, остановка, автоостановка по пределу и итоговый файл.
 */
export function useLogRecording(): ILogRecording {
    const [status, setStatus] = useState<IRecordingStatus>({ startedAtMs: null, maxSeconds: 0 });
    const [now, setNow] = useState(Date.now());
    const [isBusy, setIsBusy] = useState(false);
    const [result, setResult] = useState<IRecordingResult | null>(null);
    const [error, setError] = useState<string | null>(null);

    // EN: The window may be reloaded during a recording — take the state from Rust.
    // RU: Окно могли перезагрузить во время записи — берём состояние из Rust.
    useEffect(() => {
        getRecordingStatus()
            .then(setStatus)
            .catch(() => undefined);
        const unlisten = onRecordingFinished((finished: IRecordingResult) => {
            setStatus((prev: IRecordingStatus) => ({ ...prev, startedAtMs: null }));
            setResult(finished);
        });
        return () => {
            unlisten.then((stopListening: () => void) => stopListening());
        };
    }, []);

    const isRecording = status.startedAtMs !== null;
    // EN: After "Finish" the recording is still being packed — the timer stops at the moment of the click.
    // RU: После «Закончить» запись ещё собирается — таймер замирает на моменте нажатия.
    const [isStopping, setIsStopping] = useState(false);
    useEffect(() => {
        setUiLogging(isRecording);
    }, [isRecording]);
    useEffect(() => {
        if (!isRecording || isStopping) {
            return undefined;
        }
        const timer = window.setInterval(() => setNow(Date.now()), MS_IN_SECOND);
        return () => window.clearInterval(timer);
    }, [isRecording, isStopping]);

    const start = useCallback(async (): Promise<void> => {
        setIsBusy(true);
        setError(null);
        setResult(null);
        try {
            const next = await startRecording();
            setNow(Date.now());
            setStatus(next);
        } catch (e) {
            setError(errorText(e));
        } finally {
            setIsBusy(false);
        }
    }, []);

    const stop = useCallback(async (): Promise<void> => {
        setNow(Date.now());
        setIsStopping(true);
        setIsBusy(true);
        setError(null);
        try {
            const finished = await stopRecording();
            setResult(finished);
        } catch (e) {
            setError(errorText(e));
        } finally {
            setStatus((prev: IRecordingStatus) => ({ ...prev, startedAtMs: null }));
            setIsStopping(false);
            setIsBusy(false);
        }
    }, []);

    const clearResult = useCallback(() => {
        setResult(null);
        setError(null);
    }, []);

    const elapsedSeconds =
        status.startedAtMs === null
            ? 0
            : Math.max(0, Math.floor((now - status.startedAtMs) / MS_IN_SECOND));

    return {
        startedAtMs: status.startedAtMs,
        maxSeconds: status.maxSeconds,
        elapsedSeconds,
        isBusy,
        result,
        error,
        start,
        stop,
        clearResult,
    };
}
