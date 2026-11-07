import { ask } from '@tauri-apps/plugin-dialog';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { IZapretOverview } from '../api/zapret';
import { IZapretUpdate } from '../hooks/useZapretUpdate';
import { dateLocale, t } from '../i18n';

interface IUpdatesCardProps {
    overview: IZapretOverview;
    update: IZapretUpdate;
    isLocked: boolean;
}

const MS_IN_SECOND = 1000;

/**
 * EN: zapret updates in "Settings": check, update, rollback.
 * RU: Обновления zapret в «Настройках»: проверка, обновление, откат.
 */
export function UpdatesCard({ overview, update, isLocked }: IUpdatesCardProps) {
    const isBusy = isLocked || update.isRunning;
    const checkedAt = overview.updateCheckedAt
        ? new Date(overview.updateCheckedAt * MS_IN_SECOND).toLocaleString(dateLocale())
        : null;

    async function handleRollback(): Promise<void> {
        const isConfirmed = await ask(
            t(
                'Вернуть zapret {version}? Служба будет переустановлена с текущей стратегией. Потом можно вернуться обратно тем же способом.',
                { version: overview.rollbackVersion ?? '' }
            ),
            {
                title: t('Откат zapret'),
                kind: 'warning',
                okLabel: t('Откатить'),
                cancelLabel: t('Отмена'),
            }
        );
        if (isConfirmed) {
            await update.rollback();
        }
    }

    return (
        <div className="card wide">
            <h3>{t('Обновления zapret')}</h3>
            <dl>
                <dt>{t('Установлена')}</dt>
                <dd>{overview.version ?? '—'}</dd>
                <dt>{t('На GitHub')}</dt>
                <dd>
                    {overview.latestVersion ?? t('не проверялось')}
                    {overview.hasUpdate && <span className="tag warn">{t('новая версия')}</span>}
                    {overview.latestVersion && !overview.hasUpdate && (
                        <span className="tag ok">{t('установлена последняя')}</span>
                    )}
                </dd>
                <dt>{t('Проверено')}</dt>
                <dd>{checkedAt ?? '—'}</dd>
                <dt>{t('Папка zapret')}</dt>
                <dd className="path">
                    {overview.zapretDir}
                    <OpenFolderLink path={overview.zapretDir} />
                </dd>
                <dt>{t('Предыдущая')}</dt>
                <dd className="path">
                    {overview.rollbackVersion && overview.rollbackDir ? (
                        <>
                            {overview.rollbackVersion} — {overview.rollbackDir}
                            <OpenFolderLink path={overview.rollbackDir} />
                        </>
                    ) : (
                        <span className="muted">{t('нет')}</span>
                    )}
                </dd>
                <dt>{t('Куда')}</dt>
                <dd className="muted">
                    {overview.managed
                        ? t('новая версия скачивается в отдельную папку рядом')
                        : t(
                              'ваша папка обновляется на месте, прежняя версия сохраняется для отката'
                          )}
                </dd>
            </dl>
            <div className="btns">
                <button
                    className="btn"
                    disabled={isBusy || update.isChecking}
                    onClick={() => update.check(false)}
                >
                    {update.isChecking ? t('Проверяю…') : t('Проверить обновления')}
                </button>
                {overview.hasUpdate && (
                    <button className="btn primary" disabled={isBusy} onClick={update.start}>
                        {t('Обновить до {version}', { version: overview.latestVersion ?? '' })}
                    </button>
                )}
                {overview.rollbackVersion && (
                    <button className="btn" disabled={isBusy} onClick={handleRollback}>
                        {t('Откатить на {version}', { version: overview.rollbackVersion })}
                    </button>
                )}
            </div>
            <p className="muted small">
                {t(
                    'При обновлении сохраняются ваши списки (*-user.txt), правки в списках авторов, режим IPSet и настройки из меню service.bat.'
                )}
            </p>
        </div>
    );
}

function OpenFolderLink({ path }: { path: string | null }) {
    if (!path) {
        return null;
    }
    return (
        <button
            className="btn link inline"
            onClick={() => revealItemInDir(path).catch(() => undefined)}
        >
            {t('открыть')}
        </button>
    );
}
