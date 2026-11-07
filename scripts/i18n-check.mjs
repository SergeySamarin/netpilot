// EN: Translation completeness check. Finds t('…'), tk('…'), plural(n, '…') in src/ and t("…"), tf("…")
//     in src-tauri/src/ and compares them with locales/en.json.
//     npm run i18n:check           — list missing and unused keys (exit code 1 if something is missing)
//     npm run i18n:check -- --json — print missing keys as JSON to paste into en.json
// RU: Проверка полноты перевода. Находит t('…'), tk('…'), plural(n, '…') в src/ и t("…"), tf("…")
//     в src-tauri/src/ и сравнивает с locales/en.json.
//     npm run i18n:check           — список недостающих и лишних ключей (код выхода 1, если чего-то нет)
//     npm run i18n:check -- --json — недостающие ключи в виде JSON, чтобы вставить в en.json

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(fileURLToPath(import.meta.url), '..', '..');

function listFiles(dir, extensions) {
    return readdirSync(dir).flatMap((name) => {
        const path = join(dir, name);
        if (statSync(path).isDirectory()) {
            return listFiles(path, extensions);
        }
        return extensions.some((ext) => name.endsWith(ext)) ? [path] : [];
    });
}

// EN: Turns a string literal body into the runtime string: \\ -> \, \' -> ', \n -> line break.
// RU: Превращает тело строкового литерала в строку как в рантайме: \\ -> \, \' -> ', \n -> перенос.
function unescape(text) {
    return text.replace(/\\(.)/g, (match, ch) => (ch === 'n' ? '\n' : ch));
}

const TS_PATTERNS = [
    /\b(?:t|tk)\(\s*'((?:[^'\\]|\\.)*)'/g,
    /\bplural\([^,]+,\s*'((?:[^'\\]|\\.)*)'/g,
];
const RUST_PATTERNS = [/\b(?:t|tf)\(\s*"((?:[^"\\]|\\.)*)"/g];

// EN: Comment lines are skipped: doc comments contain usage examples like t('…').
// RU: Строки комментариев пропускаются: в doc-комментариях есть примеры вида t('…').
function withoutComments(text) {
    return text
        .split('\n')
        .filter((line) => !/^\s*(\/\/|\/\*|\*)/.test(line))
        .join('\n');
}

function collect(files, patterns, keys) {
    for (const file of files) {
        const text = withoutComments(readFileSync(file, 'utf8'));
        for (const pattern of patterns) {
            for (const match of text.matchAll(pattern)) {
                const key = unescape(match[1]);
                if (!keys.has(key)) {
                    keys.set(key, relative(root, file));
                }
            }
        }
    }
}

const keys = new Map();
collect(listFiles(join(root, 'src'), ['.ts', '.tsx']), TS_PATTERNS, keys);
collect(listFiles(join(root, 'src-tauri', 'src'), ['.rs']), RUST_PATTERNS, keys);

const en = JSON.parse(readFileSync(join(root, 'locales', 'en.json'), 'utf8'));
// EN: Keys without Cyrillic (e.g. '{ms} мс' has it, 'zapret' doesn't) need no translation.
// RU: Ключи без кириллицы не требуют перевода.
const missing = [...keys.keys()].filter((key) => /[а-яё]/i.test(key) && !(key in en));
const unused = Object.keys(en).filter((key) => !keys.has(key));

if (process.argv.includes('--json')) {
    console.log(JSON.stringify(Object.fromEntries(missing.map((key) => [key, ''])), null, 3));
    process.exit(0);
}

for (const key of missing) {
    console.log(`missing: ${JSON.stringify(key)}  (${keys.get(key)})`);
}
for (const key of unused) {
    console.log(`unused:  ${JSON.stringify(key)}`);
}
console.log(`keys: ${keys.size}, missing: ${missing.length}, unused: ${unused.length}`);
process.exit(missing.length ? 1 : 0);
