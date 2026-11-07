import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
// @ts-expect-error type error without @types/node package
import process from 'node:process';
const host = process.env.TAURI_DEV_HOST;

// EN: Vite options for Tauri, applied only in `tauri dev` and `tauri build`.
// RU: Настройки Vite для Tauri, действуют только в `tauri dev` и `tauri build`.
export default defineConfig(() => ({
    plugins: [react()],

    // EN: Two pages: the app and the player window of the video check.
    // RU: Две страницы: приложение и окно плеера проверки видео.
    build: {
        rollupOptions: {
            input: {
                main: 'index.html',
                diagPlayer: 'diag-player.html',
            },
        },
    },

    // EN: Don't clear the screen, so Rust errors stay visible.
    // RU: Не очищать экран, чтобы ошибки Rust оставались видны.
    clearScreen: false,
    server: {
        // EN: Tauri expects a fixed port; fail if it is busy.
        // RU: Tauri ждёт фиксированный порт; если он занят — ошибка.
        port: 1420,
        strictPort: true,
        host: host || false,
        hmr: host
            ? {
                  protocol: 'ws',
                  host,
                  port: 1421,
              }
            : undefined,
        watch: {
            // EN: Rust sources are watched by Tauri itself.
            // RU: За исходниками Rust следит сам Tauri.
            ignored: ['**/src-tauri/**'],
        },
    },
}));
