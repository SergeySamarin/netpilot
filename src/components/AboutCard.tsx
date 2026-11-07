import { openUrl } from '@tauri-apps/plugin-opener';
import { IAppUpdate } from '../hooks/useAppUpdate';
import { dateLocale, t } from '../i18n';

interface IAboutCardProps {
    update: IAppUpdate;
}

const REPOSITORY_URL = 'https://github.com/SergeySamarin/netpilot';
const PERCENT = 100;

/**
 * EN: "About": the version and NetPilot's own updates.
 * RU: «О программе»: версия и обновления самого NetPilot.
 */
export function AboutCard({ update }: IAboutCardProps) {
    const { available, isChecking, isInstalling, progress } = update;
    // EN: In development the installed app would be replaced, not the one being developed.
    // RU: В режиме разработки заменилось бы установленное приложение, а не разрабатываемое.
    const isDev = import.meta.env.DEV;
    const checkedText = update.checkedAt
        ? new Date(update.checkedAt).toLocaleString(dateLocale(), {
              day: '2-digit',
              month: '2-digit',
              hour: '2-digit',
              minute: '2-digit',
          })
        : null;

    let status = t('Обновления ещё не проверялись');
    if (isChecking) {
        status = t('Проверяю…');
    } else if (update.hasNoReleases) {
        status = t('Релизов на GitHub пока нет');
    } else if (available) {
        status = t('Доступна версия {version}', { version: available.version });
    } else if (checkedText) {
        status = t('У вас последняя версия · проверено {date}', { date: checkedText });
    }

    return (
        <div className="card wide" id="about">
            <h3>{t('О программе')}</h3>
            <dl>
                <dt>{t('Версия')}</dt>
                <dd>NetPilot {update.version ?? '—'}</dd>
                <dt>{t('Обновления')}</dt>
                <dd>{status}</dd>
            </dl>
            {available?.notes && <div className="release-notes">{available.notes}</div>}
            {isInstalling && (
                <>
                    <div className="progress">
                        <div style={{ width: `${(progress ?? 0) * PERCENT}%` }} />
                    </div>
                    <p className="muted small">
                        {t('Скачиваю и устанавливаю — приложение перезапустится само.')}
                    </p>
                </>
            )}
            {update.error && <div className="banner error">{update.error}</div>}
            <div className="btns">
                {available && (
                    <button
                        className="btn primary"
                        disabled={isInstalling || isDev}
                        title={
                            isDev
                                ? t('В режиме разработки обновление не устанавливается')
                                : undefined
                        }
                        onClick={update.install}
                    >
                        {t('Обновить до {version}', { version: available.version })}
                    </button>
                )}
                <button
                    className="btn"
                    disabled={isChecking || isInstalling}
                    onClick={update.checkNow}
                >
                    {t('Проверить обновления')}
                </button>
                <button
                    className="btn link"
                    onClick={() => {
                        openUrl(REPOSITORY_URL).catch(() => undefined);
                    }}
                >
                    {t('Страница на GitHub')}
                </button>
            </div>
        </div>
    );
}
