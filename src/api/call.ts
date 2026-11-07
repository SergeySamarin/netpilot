import { invoke, InvokeArgs } from '@tauri-apps/api/core';
import { errorText } from '../utils/format';

/** EN: Max length of the arguments in a log line. RU: Наибольшая длина аргументов в строке журнала. */
const MAX_ARGS_LENGTH = 300;

let isUiLogging = false;

/**
 * EN: Turned on by the log recording: until then logUi() does not even call Rust.
 * RU: Включается записью журнала: до этого logUi() даже не обращается к Rust.
 */
export function setUiLogging(isOn: boolean): void {
    isUiLogging = isOn;
}

/**
 * EN: A line about what happens in the window. Written only during a log recording.
 * RU: Строка о том, что происходит в окне. Пишется только во время записи журнала.
 */
export function logUi(message: string): void {
    if (isUiLogging) {
        invoke('log_ui', { message }).catch(() => undefined);
    }
}

/**
 * EN: Calls a Rust command. During a log recording writes the call and its error to the log.
 * RU: Вызывает команду Rust. Во время записи журнала пишет в журнал вызов и его ошибку.
 */
export async function call<T>(command: string, args?: InvokeArgs): Promise<T> {
    if (isUiLogging) {
        const argsText = args ? JSON.stringify(args).slice(0, MAX_ARGS_LENGTH) : '';
        logUi(`→ ${command} ${argsText}`);
    }
    try {
        return await invoke<T>(command, args);
    } catch (e) {
        logUi(`✗ ${command}: ${errorText(e)}`);
        throw e;
    }
}
