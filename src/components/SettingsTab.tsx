import { useState } from 'react';
import { ask, open } from '@tauri-apps/plugin-dialog';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { IZapretOverview, setZapretDir, zapretRemove } from '../api/zapret';
import { IAppUpdate } from '../hooks/useAppUpdate';
import { IAutostart } from '../hooks/useAutostart';
import { IQuickPick } from '../hooks/useQuickPick';
import { useServiceAction } from '../hooks/useServiceAction';
import { IZapretUpdate } from '../hooks/useZapretUpdate';
import { t } from '../i18n';
import { errorText } from '../utils/format';
import { AboutCard } from './AboutCard';
import { AutostartCards } from './AutostartCards';
import { ServiceBanners } from './ServiceBanners';
import { UpdateBanners } from './UpdateBanners';
import { UpdatesCard } from './UpdatesCard';

interface ISettingsTabProps {
    overview: IZapretOverview;
    pick: IQuickPick;
    update: IZapretUpdate;
    onChange: (overview: IZapretOverview) => void;
    onForget: () => void;
    autostart: IAutostart;
    appUpdate: IAppUpdate;
}

/**
 * EN: "Settings" tab: zapret folder, updates and service removal.
 * RU: Вкладка «Настройки»: папка zapret, обновления и удаление службы.
 */
export function SettingsTab({
    overview,
    pick,
    update,
    onChange,
    onForget,
    autostart,
    appUpdate,
}: ISettingsTabProps) {
    const [error, setError] = useState<string | null>(null);
    const { busyText, error: serviceError, run } = useServiceAction(onChange);
    const isInstalled = overview.service.state !== 'notInstalled';
    // EN: The pick and the update use the current folder and the service — they can't be changed meanwhile.
    // RU: Подбор и обновление работают с текущей папкой и службой — менять их в это время нельзя.
    const isLocked = pick.isRunning || update.isRunning;
    const lockHint = isLocked ? t('Недоступно, пока идёт подбор или обновление') : undefined;

    async function handleRemove(): Promise<void> {
        const isConfirmed = await ask(
            t(
                'Служба zapret и драйвер WinDivert будут удалены из системы. Обход выключится и не включится после перезагрузки. Файлы zapret останутся.\n\nПродолжить?'
            ),
            {
                title: t('Удалить службу zapret'),
                kind: 'warning',
                okLabel: t('Удалить'),
                cancelLabel: t('Отмена'),
            }
        );
        if (isConfirmed) {
            await run(t('Удаляю службу…'), zapretRemove);
        }
    }

    async function handleChoose(): Promise<void> {
        const path = await open({ directory: true, title: t('Папка zapret') });
        if (typeof path !== 'string') {
            return;
        }
        try {
            setError(null);
            onChange(await setZapretDir(path));
        } catch (e) {
            setError(errorText(e));
        }
    }

    function handleReveal(): void {
        if (overview.zapretDir) {
            revealItemInDir(overview.zapretDir).catch((e: unknown) => setError(errorText(e)));
        }
    }

    return (
        <div className="grid">
            <AutostartCards autostart={autostart} isLocked={isLocked} />
            {pick.isRunning && (
                <div className="banner info wide banner-row">
                    {t('Идёт подбор стратегии — смена папки и удаление службы пока недоступны.')}
                    <button className="btn" onClick={pick.cancel}>
                        {t('Прервать подбор')}
                    </button>
                </div>
            )}

            <div className="card wide">
                <h3>zapret</h3>
                {error && <div className="banner error">{error}</div>}
                <dl>
                    <dt>{t('Папка')}</dt>
                    <dd className="path">{overview.zapretDir}</dd>
                    <dt>{t('Откуда')}</dt>
                    <dd>
                        {overview.managed ? t('скачана приложением с GitHub') : t('указана вами')}
                    </dd>
                </dl>
                <div className="btns">
                    <button
                        className="btn"
                        disabled={isLocked}
                        title={lockHint}
                        onClick={handleChoose}
                    >
                        {t('Сменить папку…')}
                    </button>
                    <button className="btn" onClick={handleReveal}>
                        {t('Открыть в проводнике')}
                    </button>
                    <button className="btn" disabled={isLocked} title={lockHint} onClick={onForget}>
                        {t('Забыть папку')}
                    </button>
                </div>
                <p className="muted small">
                    {t(
                        '«Забыть папку» — приложение снова покажет экран первого запуска. Файлы на диске не удаляются.'
                    )}
                </p>
            </div>

            <UpdateBanners update={update} rollbackVersion={overview.rollbackVersion} />
            <UpdatesCard overview={overview} update={update} isLocked={isLocked} />

            <ServiceBanners busyText={busyText} error={serviceError} />
            <div className="card wide">
                <h3>{t('Служба zapret')}</h3>
                <dl>
                    <dt>{t('Состояние')}</dt>
                    <dd>{isInstalled ? t('установлена в Windows') : t('не установлена')}</dd>
                </dl>
                <div className="btns">
                    <button
                        className="btn danger"
                        disabled={!isInstalled || busyText !== null || isLocked}
                        title={lockHint}
                        onClick={handleRemove}
                    >
                        {t('Удалить службу из Windows')}
                    </button>
                </div>
                <p className="muted small">
                    {t(
                        'Удаляет службу zapret и драйвер WinDivert. Обход выключится и не включится после перезагрузки. Файлы zapret остаются — вернуть можно, установив стратегию заново.'
                    )}
                </p>
            </div>
            <AboutCard update={appUpdate} />
        </div>
    );
}
