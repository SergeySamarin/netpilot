import { invoke } from '@tauri-apps/api/core';
import enJson from '../../locales/en.json';

// EN: Translations: the key is the Russian text, English lives in locales/en.json (shared with Rust).
// RU: Переводы: ключ — русский текст, английский — в locales/en.json (общий с Rust).

export type Lang = 'ru' | 'en';

const EN: Record<string, string> = enJson;

let current: Lang = 'ru';

export function setLang(lang: Lang): void {
    current = lang;
}

export function getLang(): Lang {
    return current;
}

/** EN: Language from the settings or the system (decided by Rust). RU: Язык из настроек или системы (решает Rust). */
export function loadLanguage(): Promise<Lang> {
    return invoke('get_language');
}

/** EN: Saves the user's choice. RU: Сохраняет выбор пользователя. */
export function saveLanguage(language: Lang): Promise<void> {
    return invoke('set_language', { language });
}

/**
 * EN: Marks a translation key that is translated later via t(variable) — e.g. in constant maps.
 *     Returns the key as is; needed so that `npm run i18n:check` finds the key.
 * RU: Помечает ключ перевода, который переводится позже через t(переменная) — например, в
 *     константах-словарях. Возвращает ключ как есть; нужен, чтобы `npm run i18n:check` нашёл ключ.
 */
export function tk(key: string): string {
    return key;
}

type Params = Record<string, string | number>;

function substitute(text: string, params?: Params): string {
    if (!params) {
        return text;
    }
    return text.replace(/\{(\w+)\}/g, (match: string, name: string) =>
        name in params ? String(params[name]) : match
    );
}

/**
 * EN: Translates a text: `t('Проверяю {name}', { name })`. Without a translation returns the Russian text.
 * RU: Переводит текст: `t('Проверяю {name}', { name })`. Если перевода нет — возвращает русский текст.
 */
export function t(key: string, params?: Params): string {
    const text = current === 'en' ? (EN[key] ?? key) : key;
    return substitute(text, params);
}

const PLURAL_MOD_10 = 10;
const PLURAL_MOD_100 = 100;
const PLURAL_FEW_MIN = 2;
const PLURAL_FEW_MAX = 4;
const PLURAL_TEENS_MIN = 11;
const PLURAL_TEENS_MAX = 14;

/** EN: Russian plural form index: 1 стратегия, 2 стратегии, 5 стратегий. RU: Индекс формы: 1, 2–4, 5+. */
function ruPluralIndex(count: number): number {
    const mod10 = count % PLURAL_MOD_10;
    const mod100 = count % PLURAL_MOD_100;
    if (mod10 === 1 && mod100 !== PLURAL_TEENS_MIN) {
        return 0;
    }
    if (
        mod10 >= PLURAL_FEW_MIN &&
        mod10 <= PLURAL_FEW_MAX &&
        (mod100 < PLURAL_TEENS_MIN || mod100 > PLURAL_TEENS_MAX)
    ) {
        return 1;
    }
    return 2;
}

/**
 * EN: A word form for a number: `plural(5, '{n} стратегия|{n} стратегии|{n} стратегий')`.
 *     In en.json the same key maps to two forms: "{n} strategy|{n} strategies".
 * RU: Форма слова для числа: `plural(5, '{n} стратегия|{n} стратегии|{n} стратегий')`.
 *     В en.json тот же ключ переводится двумя формами: "{n} strategy|{n} strategies".
 */
export function plural(count: number, forms: string): string {
    const variants = t(forms).split('|');
    const index = current === 'ru' ? ruPluralIndex(count) : Number(count !== 1);
    return substitute(variants[Math.min(index, variants.length - 1)], { n: count });
}

/** EN: Locale for dates and numbers. RU: Локаль для дат и чисел. */
export function dateLocale(): string {
    return current === 'ru' ? 'ru-RU' : 'en-US';
}
