import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Lang, setLang, t } from '../i18n';

// EN: Player page of the video check (see src-tauri/src/diag/mod.rs). Two modes:
//     - check: plays the planned videos one after another and sends player events to Rust;
//     - view (`?view=<video>`): just plays a video from a moment, to keep watching after the check.
//     The plan, the player size and the mode come in the address.
// RU: Страница плеера проверки видео (см. src-tauri/src/diag/mod.rs). Два режима:
//     - проверка: проигрывает видео из плана одно за другим и шлёт события плеера в Rust;
//     - просмотр (`?view=<видео>`): просто показывает видео с нужного места, чтобы досмотреть после проверки.
//     План, размер плеера и режим приходят в адресе.

const PLAYER_EVENT = 'diag-player';
const API_URL = 'https://www.youtube.com/iframe_api';
const API_TIMEOUT_MS = 20000;
const MS_IN_SECOND = 1000;
/** EN: Error code when the YouTube player script did not load. RU: Код ошибки, если скрипт плеера YouTube не загрузился. */
const API_LOAD_ERROR = -1;
/** EN: Player size in the view mode. RU: Размер плеера в режиме просмотра. */
const VIEW_WIDTH = 1920;
const VIEW_HEIGHT = 1080;

/** EN: The minimum of the YouTube IFrame API that the page uses. RU: Минимум YouTube IFrame API, который использует страница. */
interface IYouTubePlayer {
    loadVideoById(videoId: string, startSeconds?: number): void;
    mute(): void;
    stopVideo(): void;
    getCurrentTime(): number;
}

interface IYouTubePlayerOptions {
    width: number;
    height: number;
    playerVars: Record<string, string | number>;
    events: {
        onReady: () => void;
        onStateChange?: (event: { data: number }) => void;
        onPlaybackQualityChange?: (event: { data: string }) => void;
        onError?: (event: { data: number }) => void;
    };
}

declare global {
    interface Window {
        YT?: { Player: new (elementId: string, options: IYouTubePlayerOptions) => IYouTubePlayer };
        onYouTubeIframeAPIReady?: () => void;
    }
}

interface IPlanItem {
    videoId: string;
    seconds: number;
}

type PlayerEvent =
    | { kind: 'load'; video: string }
    | { kind: 'state'; state: number }
    | { kind: 'quality'; quality: string }
    | { kind: 'error'; code: number }
    | { kind: 'done'; video: string | null; position: number | null };

const params = new URLSearchParams(window.location.search);
setLang((params.get('lang') as Lang | null) ?? 'ru');
const viewVideo = params.get('view');
const width = viewVideo ? VIEW_WIDTH : Number(params.get('width'));
const height = viewVideo ? VIEW_HEIGHT : Number(params.get('height'));
const plan: IPlanItem[] = (params.get('plan') ?? '')
    .split(',')
    .filter(Boolean)
    .map((item: string) => {
        const [videoId, seconds] = item.split(':');
        return { videoId, seconds: Number(seconds) };
    });

const frame = document.getElementById('frame') as HTMLElement;
const scaled = document.getElementById('scaled') as HTMLElement;
const caption = document.getElementById('caption') as HTMLElement;
const keepLabel = document.getElementById('keep-label') as HTMLElement;
const keep = document.getElementById('keep') as HTMLInputElement;
const keepText = document.getElementById('keep-text') as HTMLElement;
const fullButton = document.getElementById('full') as HTMLButtonElement;
const exitButton = document.getElementById('exit') as HTMLButtonElement;

let player: IYouTubePlayer | null = null;
let currentVideo: string | null = null;
let isFinished = false;
let isFull = false;

/**
 * EN: YouTube picks the quality by the player size: the player keeps its full size and is scaled to fit
 *     the frame, also in full screen — so full screen does not change the measurement.
 * RU: YouTube выбирает качество по размеру плеера: плеер сохраняет полный размер и вписывается в рамку
 *     уменьшением, в том числе на весь экран — так полный экран не меняет замер.
 */
function fitPlayer(): void {
    const scale = Math.min(frame.clientWidth / width, frame.clientHeight / height);
    const left = (frame.clientWidth - width * scale) / 2;
    const top = (frame.clientHeight - height * scale) / 2;
    scaled.style.transform = `translate(${left}px, ${top}px) scale(${scale})`;
}

async function setFull(next: boolean): Promise<void> {
    await getCurrentWindow().setFullscreen(next);
    isFull = next;
    document.body.classList.toggle('full', next);
    fitPlayer();
}

function send(event: PlayerEvent): void {
    emit(PLAYER_EVENT, { ...event, at: Date.now() }).catch(() => undefined);
}

function showCaption(index: number): void {
    if (viewVideo) {
        caption.textContent = '';
        return;
    }
    const text = t('Идёт проверка видео, звук выключен.');
    caption.textContent =
        plan.length > 1
            ? `${text} ${t('Видео {n} из {total}', { n: index + 1, total: plan.length })}`
            : text;
}

function finish(): void {
    if (isFinished) {
        return;
    }
    isFinished = true;
    const position = player ? player.getCurrentTime() : null;
    player?.stopVideo();
    send({ kind: 'done', video: currentVideo, position });
}

function play(index: number): void {
    if (index >= plan.length) {
        finish();
        return;
    }
    const item = plan[index];
    currentVideo = item.videoId;
    showCaption(index);
    send({ kind: 'load', video: item.videoId });
    player?.loadVideoById(item.videoId);
    window.setTimeout(() => play(index + 1), item.seconds * MS_IN_SECOND);
}

function createPlayer(): void {
    if (!window.YT) {
        return;
    }
    const events: IYouTubePlayerOptions['events'] = viewVideo
        ? { onReady: () => player?.loadVideoById(viewVideo, Number(params.get('start') ?? 0)) }
        : {
              onReady: () => {
                  player?.mute();
                  play(0);
              },
              onStateChange: (event: { data: number }) =>
                  send({ kind: 'state', state: event.data }),
              onPlaybackQualityChange: (event: { data: string }) =>
                  send({ kind: 'quality', quality: event.data }),
              onError: (event: { data: number }) => send({ kind: 'error', code: event.data }),
          };
    player = new window.YT.Player('player', {
        width,
        height,
        playerVars: {
            autoplay: 1,
            mute: viewVideo ? 0 : 1,
            playsinline: 1,
            origin: window.location.origin,
        },
        events,
    });
}

if (viewVideo) {
    document.body.classList.add('view');
} else {
    keepLabel.classList.toggle('hidden', !params.has('keep'));
    keep.checked = params.get('keep') === '1';
    keepText.textContent = t('Не закрывать после проверки — досмотреть видео');
    keep.addEventListener('change', () => {
        invoke('diag_keep_watching', { enabled: keep.checked }).catch(() => undefined);
    });
}
fullButton.textContent = t('На весь экран');
fullButton.addEventListener('click', () => {
    setFull(true).catch(() => undefined);
});
// EN: Keys and clicks inside the YouTube frame don't reach the page, so full screen has its own exit button.
// RU: Клавиши и клики внутри рамки YouTube не доходят до страницы, поэтому у полного экрана своя кнопка выхода.
exitButton.textContent = t('Выйти из полного экрана');
exitButton.addEventListener('click', () => {
    setFull(false).catch(() => undefined);
});
document.addEventListener('keydown', (event: KeyboardEvent) => {
    if (event.key === 'Escape' && isFull) {
        setFull(false).catch(() => undefined);
    }
});
window.addEventListener('resize', fitPlayer);
fitPlayer();
showCaption(0);

window.onYouTubeIframeAPIReady = createPlayer;
const script = document.createElement('script');
script.src = API_URL;
document.head.appendChild(script);
if (!viewVideo) {
    window.setTimeout(() => {
        if (!player) {
            send({ kind: 'error', code: API_LOAD_ERROR });
            finish();
        }
    }, API_TIMEOUT_MS);
}
