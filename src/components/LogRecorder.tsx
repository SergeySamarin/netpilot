import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { ReactNode, useState } from 'react';
import { ILogRecording } from '../hooks/useLogRecording';
import { t } from '../i18n';
import { errorText } from '../utils/format';

const SECONDS_IN_MINUTE = 60;

interface ILogRecorderProps {
    recording: ILogRecording;
}

function formatClock(seconds: number): string {
    const minutes = Math.floor(seconds / SECONDS_IN_MINUTE);
    const rest = seconds % SECONDS_IN_MINUTE;
    return `${String(minutes).padStart(2, '0')}:${String(rest).padStart(2, '0')}`;
}

/**
 * EN: Header control of the log recording: the button, the instruction window, the "recording" indicator
 *     and the window with the resulting file.
 * RU: Управление записью журнала в шапке: кнопка, окно с инструкцией, индикатор «идёт запись» и окно с
 *     итоговым файлом.
 */
export function LogRecorder({ recording }: ILogRecorderProps) {
    const [isIntroOpen, setIsIntroOpen] = useState(false);
    const [isCopied, setIsCopied] = useState(false);
    const [copyError, setCopyError] = useState<string | null>(null);
    const { startedAtMs, maxSeconds, elapsedSeconds, isBusy, result, error } = recording;
    const maxMinutes = Math.round(maxSeconds / SECONDS_IN_MINUTE);

    async function handleStart(): Promise<void> {
        await recording.start();
        setIsIntroOpen(false);
    }

    function handleClose(): void {
        setIsIntroOpen(false);
        setIsCopied(false);
        setCopyError(null);
        recording.clearResult();
    }

    function handleCopy(path: string): void {
        navigator.clipboard
            .writeText(path)
            .then(() => setIsCopied(true))
            .catch((e: unknown) => setCopyError(errorText(e)));
    }

    function handleReveal(path: string): void {
        revealItemInDir(path).catch((e: unknown) => setCopyError(errorText(e)));
    }

    const header =
        startedAtMs !== null ? (
            <div
                className="recording-indicator"
                title={t(
                    'Повторите действия, при которых возникает проблема, затем нажмите «Закончить». Запись остановится сама через {minutes} мин.',
                    { minutes: maxMinutes }
                )}
            >
                <span className="rec-dot" />
                {t('Запись {time}', { time: formatClock(elapsedSeconds) })}
                <button className="btn" disabled={isBusy} onClick={recording.stop}>
                    {isBusy ? t('Собираю файл…') : t('Закончить')}
                </button>
            </div>
        ) : (
            <button
                className="btn"
                disabled={isBusy}
                title={t(
                    'Если что-то работает не так: запишите журнал, повторите проблему и получите файл, который можно отправить вместе с описанием.'
                )}
                onClick={() => setIsIntroOpen(true)}
            >
                {t('Записать журнал')}
            </button>
        );

    let modal: ReactNode = null;
    if (result) {
        modal = (
            <>
                <h3>{t('Файл журнала готов')}</h3>
                {result.reason === 'timeLimit' && (
                    <div className="banner info">
                        {t('Запись остановилась сама: прошло {minutes} мин.', {
                            minutes: maxMinutes,
                        })}
                    </div>
                )}
                {result.reason === 'sizeLimit' && (
                    <div className="banner info">
                        {t('Запись остановилась сама: журнал достиг предельного размера.')}
                    </div>
                )}
                <p className="muted">
                    {t('Запись длилась {time}.', { time: formatClock(result.seconds) })}
                </p>
                <div className="file-path">{result.archivePath}</div>
                <p>
                    {t(
                        'Отправьте этот файл вместе с описанием: что делали, что ожидали и что произошло на самом деле.'
                    )}
                </p>
                <p className="muted small">
                    {t('Хранятся 3 последних файла журнала, более старые удаляются сами.')}
                </p>
                {copyError && <div className="banner error">{copyError}</div>}
                <div className="btns">
                    <button
                        className="btn primary"
                        onClick={() => handleReveal(result.archivePath)}
                    >
                        {t('Показать в папке')}
                    </button>
                    <button className="btn" onClick={() => handleCopy(result.archivePath)}>
                        {isCopied ? t('Путь скопирован') : t('Скопировать путь')}
                    </button>
                    <button className="btn link" onClick={handleClose}>
                        {t('Закрыть')}
                    </button>
                </div>
            </>
        );
    } else if (isIntroOpen) {
        modal = (
            <>
                <h3>{t('Запись журнала')}</h3>
                <ol className="steps">
                    <li>{t('Нажмите «Начать запись».')}</li>
                    <li>
                        {t(
                            'Повторите действия, при которых возникает проблема. Например, откройте видео и дождитесь, пока оно остановится.'
                        )}
                    </li>
                    <li>{t('Нажмите «Закончить» вверху окна.')}</li>
                    <li>
                        {t(
                            'Приложение соберёт файл журнала — отправьте его вместе с описанием проблемы.'
                        )}
                    </li>
                </ol>
                <p className="muted small">
                    {t(
                        'В журнал попадают настройки приложения, состояние службы и сети, результаты проверок. Пароли не записываются. Запись остановится сама через {minutes} мин.',
                        { minutes: maxMinutes }
                    )}
                </p>
                {error && <div className="banner error">{error}</div>}
                <div className="btns">
                    <button className="btn primary" disabled={isBusy} onClick={handleStart}>
                        {isBusy ? t('Начинаю…') : t('Начать запись')}
                    </button>
                    <button className="btn link" disabled={isBusy} onClick={handleClose}>
                        {t('Отмена')}
                    </button>
                </div>
            </>
        );
    } else if (error) {
        modal = (
            <>
                <h3>{t('Запись журнала')}</h3>
                <div className="banner error">{error}</div>
                <div className="btns">
                    <button className="btn link" onClick={handleClose}>
                        {t('Закрыть')}
                    </button>
                </div>
            </>
        );
    }

    return (
        <>
            {header}
            {modal && (
                <div className="modal-overlay">
                    <div className="modal card">{modal}</div>
                </div>
            )}
        </>
    );
}
