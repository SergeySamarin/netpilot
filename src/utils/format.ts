import { dateLocale, t } from '../i18n';

const BYTES_IN_KB = 1024;
const BYTES_IN_MB = BYTES_IN_KB * BYTES_IN_KB;

/** EN: 1518649 -> "1.4 MB" / "1,4 МБ". RU: 1518649 -> "1,4 МБ" / "1.4 MB". */
export function formatSize(bytes: number): string {
    if (bytes >= BYTES_IN_MB) {
        const value = (bytes / BYTES_IN_MB).toLocaleString(dateLocale(), {
            maximumFractionDigits: 1,
        });
        return t('{value} МБ', { value });
    }
    return t('{value} КБ', { value: Math.round(bytes / BYTES_IN_KB) });
}

/** EN: ISO date -> "20.10.2026" / "10/20/2026". RU: ISO-дата -> "20.10.2026" / "10/20/2026". */
export function formatDate(iso: string): string {
    return new Date(iso).toLocaleDateString(dateLocale());
}

const MS_IN_SECOND = 1000;
const SECONDS_IN_MINUTE = 60;
const ROUND_TO_SECONDS = 15;

/**
 * EN: Seconds -> "≈ 45 s", "≈ 3 min", "≈ 3 min 15 s" (rounded up to 15 seconds).
 * RU: Секунды -> «≈ 45 с», «≈ 3 мин», «≈ 3 мин 15 с» (с округлением вверх до 15 секунд).
 */
export function formatRemaining(seconds: number): string {
    const rounded = Math.max(
        ROUND_TO_SECONDS,
        Math.ceil(seconds / ROUND_TO_SECONDS) * ROUND_TO_SECONDS
    );
    if (rounded < SECONDS_IN_MINUTE) {
        return t('≈ {seconds} с', { seconds: rounded });
    }
    const minutes = Math.floor(rounded / SECONDS_IN_MINUTE);
    const rest = rounded % SECONDS_IN_MINUTE;
    return rest
        ? t('≈ {minutes} мин {seconds} с', { minutes, seconds: rest })
        : t('≈ {minutes} мин', { minutes });
}

/**
 * EN: Seconds left. The average step time is counted over finished steps only, and the time inside the
 *     current step is subtracted — so the estimate goes down smoothly. Until minDone steps are finished
 *     there is no estimate (null): one or two steps give a poor estimate.
 * RU: Сколько секунд осталось. Среднее время шага считается только по законченным шагам, а время внутри
 *     текущего шага вычитается — так оценка плавно убывает. Пока законченных шагов меньше minDone,
 *     оценки нет (null): по одному-двум шагам она неточная.
 */
export function estimateRemainingSeconds(
    done: number,
    total: number,
    startedAt: number,
    lastStepAt: number,
    minDone: number
): number | null {
    if (done < minDone) {
        return null;
    }
    const average = (lastStepAt - startedAt) / MS_IN_SECOND / done;
    const left = total - done;
    const inCurrentStep = (Date.now() - lastStepAt) / MS_IN_SECOND;
    // EN: The current step takes longer than average — don't go below the estimate for the remaining steps.
    // RU: Текущий шаг затянулся дольше среднего — не уходим ниже оценки для оставшихся шагов.
    return Math.max(average * left - inCurrentStep, average * (left - 1));
}

/** EN: "DE" -> "Germany" / "Германия". RU: "DE" -> «Германия» / "Germany". */
export function countryName(code: string): string {
    try {
        return new Intl.DisplayNames([dateLocale()], { type: 'region' }).of(code) ?? code;
    } catch {
        return code;
    }
}

/** EN: 75 -> "1:15". RU: 75 -> «1:15». */
export function formatClock(seconds: number): string {
    const whole = Math.floor(seconds);
    const rest = whole % SECONDS_IN_MINUTE;
    return `${Math.floor(whole / SECONDS_IN_MINUTE)}:${String(rest).padStart(2, '0')}`;
}

/** EN: 6281 ms -> "6.3 s" / "6,3 с". RU: 6281 мс -> «6,3 с» / "6.3 s". */
export function formatSeconds(ms: number): string {
    const value = (ms / MS_IN_SECOND).toLocaleString(dateLocale(), {
        minimumFractionDigits: 1,
        maximumFractionDigits: 1,
    });
    return t('{value} с', { value });
}

/**
 * EN: Error text from invoke(): Rust returns a string, Error is handled just in case.
 * RU: Текст ошибки из invoke(): Rust возвращает строку, Error обрабатываем на всякий случай.
 */
export function errorText(error: unknown): string {
    if (typeof error === 'string') {
        return error;
    }
    if (error instanceof Error) {
        return error.message;
    }
    return String(error);
}
