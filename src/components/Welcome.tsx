import { useEffect, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import {
    downloadZapret,
    getLatestRelease,
    IDownloadProgress,
    IReleaseInfo,
    IZapretOverview,
    onDownloadProgress,
    setZapretDir,
} from '../api/zapret';
import { t } from '../i18n';
import { errorText, formatDate, formatSize } from '../utils/format';

interface IWelcomeProps {
    overview: IZapretOverview;
    onReady: (overview: IZapretOverview) => void;
}

const PERCENT = 100;

/**
 * EN: First run screen: there are no zapret files yet — download from GitHub or choose a folder.
 * RU: Экран первого запуска: файлов zapret ещё нет — скачать с GitHub или указать свою папку.
 */
export function Welcome({ overview, onReady }: IWelcomeProps) {
    const [release, setRelease] = useState<IReleaseInfo | null>(null);
    const [releaseError, setReleaseError] = useState<string | null>(null);
    const [progress, setProgress] = useState<IDownloadProgress | null>(null);
    const [isBusy, setIsBusy] = useState(false);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        getLatestRelease()
            .then(setRelease)
            .catch((e: unknown) => setReleaseError(errorText(e)));
    }, []);

    async function handleDownload(): Promise<void> {
        setIsBusy(true);
        setError(null);
        const unlisten = await onDownloadProgress(setProgress);
        try {
            const result = await downloadZapret();
            onReady(result);
        } catch (e) {
            setError(errorText(e));
        } finally {
            unlisten();
            setProgress(null);
            setIsBusy(false);
        }
    }

    async function applyDir(path: string): Promise<void> {
        setIsBusy(true);
        setError(null);
        try {
            const result = await setZapretDir(path);
            onReady(result);
        } catch (e) {
            setError(errorText(e));
        } finally {
            setIsBusy(false);
        }
    }

    async function handleChoose(): Promise<void> {
        const path = await open({ directory: true, title: t('Папка zapret') });
        if (typeof path === 'string') {
            await applyDir(path);
        }
    }

    const downloadTarget =
        release && overview.downloadBaseDir
            ? `${overview.downloadBaseDir}\\${release.version}`
            : null;

    return (
        <div className="welcome">
            <h2>{t('Первый запуск')}</h2>
            <p className="muted">
                {t(
                    'Для обхода замедлений нужны файлы zapret — сборка zapret-discord-youtube от Flowseal с GitHub. Выберите, откуда их взять.'
                )}
            </p>

            {error && <div className="banner error">{error}</div>}

            {overview.detectedDir && (
                <div className="card option">
                    <div>
                        <h3>{t('Использовать установленный zapret')}</h3>
                        <p className="muted">
                            {t('Служба zapret уже работает из папки')} <b>{overview.detectedDir}</b>
                        </p>
                    </div>
                    <button
                        className="btn primary"
                        disabled={isBusy}
                        onClick={() => applyDir(overview.detectedDir ?? '')}
                    >
                        {t('Использовать')}
                    </button>
                </div>
            )}

            <div className="card option">
                <div>
                    <h3>{t('Скачать с GitHub')}</h3>
                    {release && (
                        <p className="muted">
                            {t('Последний релиз {version} от {date}, {size}.', {
                                version: release.version,
                                date: formatDate(release.publishedAt),
                                size: formatSize(release.size),
                            })}
                            <br />
                            {t('Будет сохранён в {path}', { path: downloadTarget ?? '' })}
                        </p>
                    )}
                    {!release && !releaseError && (
                        <p className="muted">{t('Узнаю последнюю версию…')}</p>
                    )}
                    {releaseError && <p className="error-text">{releaseError}</p>}
                    {progress && <DownloadBar progress={progress} />}
                </div>
                <button
                    className="btn primary"
                    disabled={isBusy || !release}
                    onClick={handleDownload}
                >
                    {progress ? t('Скачиваю…') : t('Скачать')}
                </button>
            </div>

            <div className="card option">
                <div>
                    <h3>{t('Указать папку')}</h3>
                    <p className="muted">
                        {t(
                            'Если zapret уже скачан. В папке должны быть bin\\winws.exe и файлы general*.bat.'
                        )}
                    </p>
                </div>
                <button className="btn" disabled={isBusy} onClick={handleChoose}>
                    {t('Выбрать папку…')}
                </button>
            </div>

            <p className="muted small">
                {t(
                    'Антивирус иногда считает файлы zapret (winws.exe, WinDivert) подозрительными и удаляет их. Если после скачивания появится ошибка проверки — добавьте папку в исключения Защитника Windows.'
                )}
            </p>
        </div>
    );
}

function DownloadBar({ progress }: { progress: IDownloadProgress }) {
    if (progress.stage === 'extract') {
        return <p className="muted">{t('Распаковываю…')}</p>;
    }
    const percent = progress.total
        ? Math.round((progress.downloaded / progress.total) * PERCENT)
        : 0;
    return (
        <div className="progress-wrap">
            <div className="progress">
                <div style={{ width: `${percent}%` }} />
            </div>
            <span className="muted small">
                {t('{done} из {total}', {
                    done: formatSize(progress.downloaded),
                    total: formatSize(progress.total),
                })}
            </span>
        </div>
    );
}
