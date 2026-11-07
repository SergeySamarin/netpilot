import { IListChange, UpdateStage } from '../api/zapret';
import { IZapretUpdate } from '../hooks/useZapretUpdate';
import { t, tk } from '../i18n';
import { formatSize } from '../utils/format';

interface IUpdateBannersProps {
    update: IZapretUpdate;
    rollbackVersion: string | null;
}

const STAGE_TEXT: Record<UpdateStage, string> = {
    original: tk('Скачиваю исходную копию текущей версии, чтобы найти ваши изменения…'),
    download: tk('Скачиваю новую версию…'),
    merge: tk('Переношу ваши изменения…'),
    apply: tk('Устанавливаю версию. Если Windows спросит разрешение — нажмите «Да».'),
};

/**
 * EN: Update progress, error and the report of what was moved to the new version.
 * RU: Ход обновления, ошибка и отчёт о том, что перенесено в новую версию.
 */
export function UpdateBanners({ update, rollbackVersion }: IUpdateBannersProps) {
    const { isRunning, stage, progress, result, error } = update;

    if (isRunning) {
        return (
            <div className="banner info wide">
                {stage ? t(STAGE_TEXT[stage]) : t('Обновляю zapret…')}
                {progress?.stage === 'download' && progress.total > 0 && (
                    <span className="muted small">
                        {' '}
                        {t('{done} из {total}', {
                            done: formatSize(progress.downloaded),
                            total: formatSize(progress.total),
                        })}
                    </span>
                )}
            </div>
        );
    }

    if (error) {
        return (
            <div className="banner error wide banner-row">
                {error}
                <button className="btn" onClick={update.dismiss}>
                    {t('Закрыть')}
                </button>
            </div>
        );
    }

    if (!result) {
        return null;
    }

    const { report } = result;
    const hasChanges =
        report.userFiles.length +
            report.mergedLists.length +
            report.settings.length +
            report.notTransferred.length >
            0 || report.ipsetMode !== null;

    return (
        <div className="card wide update-report">
            <h3>
                {t('zapret обновлён до {version}', { version: result.version })}
                <span className="hint">
                    <button className="btn link" onClick={update.dismiss}>
                        {t('Закрыть')}
                    </button>
                </span>
            </h3>
            <ul>
                {!hasChanges && (
                    <li>{t('Ваших изменений не было — установлена чистая новая версия.')}</li>
                )}
                {report.userFiles.length > 0 && (
                    <li>
                        {t('Ваши файлы перенесены: {files}', {
                            files: report.userFiles.join(', '),
                        })}
                    </li>
                )}
                {report.mergedLists.map((change: IListChange) => (
                    <li key={change.file}>
                        {t('{file}: ваши правки сохранены (добавлено {added}, удалено {removed})', {
                            file: change.file,
                            added: change.added,
                            removed: change.removed,
                        })}
                    </li>
                ))}
                {report.ipsetMode && (
                    <li>{t('Режим IPSet сохранён: {mode}', { mode: report.ipsetMode })}</li>
                )}
                {report.settings.length > 0 && (
                    <li>
                        {t('Настройки сохранены: {files}', { files: report.settings.join(', ') })}
                    </li>
                )}
                {report.notTransferred.length > 0 && (
                    <li className="warn-text">
                        {t(
                            'Вы меняли эти файлы, правки не перенесены — они остались в предыдущей версии: {files}',
                            { files: report.notTransferred.join(', ') }
                        )}
                    </li>
                )}
                {result.strategyMissing && (
                    <li className="warn-text">
                        {t(
                            'Стратегии {name} нет в новой версии — служба осталась на прежней. Подберите стратегию заново.',
                            { name: result.strategyMissing }
                        )}
                    </li>
                )}
                {rollbackVersion && (
                    <li>
                        {t('Версия {version} сохранена — откатиться можно в «Настройках».', {
                            version: rollbackVersion,
                        })}
                    </li>
                )}
            </ul>
        </div>
    );
}
