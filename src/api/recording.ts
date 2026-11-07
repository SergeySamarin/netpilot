import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export interface IRecordingStatus {
    /** EN: Start time, Unix ms. null — no recording. RU: Время начала, мс Unix. null — запись не идёт. */
    startedAtMs: number | null;
    maxSeconds: number;
}

/** EN: Why the recording stopped. RU: Почему запись остановилась. */
export type StopReason = 'manual' | 'timeLimit' | 'sizeLimit';

export interface IRecordingResult {
    archivePath: string;
    reason: StopReason;
    seconds: number;
}

export function getRecordingStatus(): Promise<IRecordingStatus> {
    return invoke('log_recording_status');
}

/**
 * EN: Starts the recording and takes a system snapshot (a few seconds).
 * RU: Начинает запись и снимает снимок системы (несколько секунд).
 */
export function startRecording(): Promise<IRecordingStatus> {
    return invoke('log_recording_start');
}

/**
 * EN: Stops the recording, takes the end snapshot and packs the zip.
 * RU: Останавливает запись, снимает снимок в конце и собирает zip.
 */
export function stopRecording(): Promise<IRecordingResult> {
    return invoke('log_recording_stop');
}

/**
 * EN: The recording stopped by itself (time or size limit).
 * RU: Запись остановилась сама (предел времени или размера).
 */
export function onRecordingFinished(
    handler: (result: IRecordingResult) => void
): Promise<UnlistenFn> {
    return listen<IRecordingResult>('log-recording-finished', (event) => handler(event.payload));
}
