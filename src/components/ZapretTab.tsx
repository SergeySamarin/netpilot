import { isPickForDir } from '../api/checks';
import {
    IZapretOverview,
    ServiceState,
    strategyTitle,
    zapretInstall,
    zapretStart,
    zapretStop,
} from '../api/zapret';
import { AutostartMode, RUSSIA } from '../api/app';
import { IAutostart } from '../hooks/useAutostart';
import { IQuickPick } from '../hooks/useQuickPick';
import { useServiceAction } from '../hooks/useServiceAction';
import { IServiceChecks } from '../hooks/useServiceChecks';
import { IZapretUpdate } from '../hooks/useZapretUpdate';
import { t, tk } from '../i18n';
import { countryName } from '../utils/format';
import { IDeepPick } from '../hooks/useDeepPick';
import { IZapretServices } from '../hooks/useZapretServices';
import { QuickPickCard } from './QuickPickCard';
import { ServiceBanners } from './ServiceBanners';
import { ServicesCard } from './ServicesCard';
import { StrategyMenu } from './StrategyMenu';
import { UpdateBanners } from './UpdateBanners';
import { Switch } from './Switch';

interface IZapretTabProps {
    overview: IZapretOverview;
    isLoading: boolean;
    pick: IQuickPick;
    serviceChecks: IServiceChecks;
    update: IZapretUpdate;
    onForget: () => void;
    onChange: (overview: IZapretOverview) => void;
    deepPick: IDeepPick;
    onOpenDiag: () => void;
    services: IZapretServices;
    autostart: IAutostart;
}

// EN: Texts are translation keys: t() is called at render time, after the language is known.
// RU: Тексты — ключи перевода: t() вызывается при отрисовке, когда язык уже известен.
const STATE_TEXT: Record<ServiceState, { text: string; color: string }> = {
    running: { text: tk('Обход работает'), color: 'green' },
    stopped: { text: tk('Обход остановлен'), color: 'grey' },
    starting: { text: tk('Запускается…'), color: 'yellow' },
    stopping: { text: tk('Останавливается…'), color: 'yellow' },
    notInstalled: { text: tk('Служба zapret не установлена'), color: 'grey' },
    unknown: { text: tk('Состояние службы неизвестно'), color: 'yellow' },
};

/**
 * EN: "Russia · zapret" tab: service state and manual strategy control.
 * RU: Вкладка «Россия · zapret»: состояние службы и ручное управление стратегиями.
 */
export function ZapretTab({
    overview,
    isLoading,
    pick,
    serviceChecks,
    update,
    onForget,
    onChange,
    deepPick,
    onOpenDiag,
    services,
    autostart,
}: IZapretTabProps) {
    const { busyText, error, run: runAction } = useServiceAction(onChange);

    const { service } = overview;
    const state = STATE_TEXT[service.state];
    const isInstalled = service.state !== 'notInstalled';
    const isRunning = service.state === 'running';
    const autostartMode = autostart.status?.mode ?? null;
    const country = autostart.country;
    const isAbroad = autostartMode === 'app' && country !== null && country.code !== RUSSIA;
    const isBusy = busyText !== null || isLoading || pick.isRunning || update.isRunning;
    const currentStrategyId = overview.serviceFromOtherDir ? null : service.strategyId;
    const pickResult =
        pick.result && isPickForDir(pick.result, overview.zapretDir, overview.version)
            ? pick.result
            : null;
    const isPickStale = pick.result !== null && pickResult === null;
    const onlyNowHint = t('Только сейчас. Автозапуск не меняется');

    function handleInstall(strategyId: string): void {
        const text = t('Устанавливаю службу со стратегией {name}…', {
            name: strategyTitle(strategyId),
        });
        runAction(text, () => zapretInstall(strategyId));
    }

    if (overview.dirError) {
        return (
            <div className="banner error">
                <p>
                    {t('Папка zapret {path} недоступна: {error}.', {
                        path: overview.zapretDir ?? '',
                        error: overview.dirError,
                    })}
                </p>
                <div className="btns">
                    <button className="btn" onClick={onForget}>
                        {t('Указать заново')}
                    </button>
                </div>
            </div>
        );
    }

    return (
        <div className="grid zapret-grid">
            <ServiceBanners busyText={busyText} error={error ?? pick.error} />
            <UpdateBanners update={update} rollbackVersion={overview.rollbackVersion} />
            {isAbroad && (
                <div className="banner wide">
                    {t(
                        'Вы не в России (по IP — {country}). zapret выключен: за границей он не нужен. Вернётесь — включится сам.',
                        { country: countryName(country.code) }
                    )}
                </div>
            )}
            {/* EN: The pick and the update run winws.exe themselves while the service is stopped — not a conflict.
                RU: Подбор и обновление сами запускают winws.exe при остановленной службе — это не конфликт. */}
            {overview.standaloneWinws && !pick.isRunning && !update.isRunning && (
                <div className="banner wide">
                    {t(
                        'winws.exe запущен вручную (из bat-файла), а не службой. Две копии мешают друг другу — закройте окно zapret перед установкой службы.'
                    )}
                </div>
            )}
            {overview.serviceFromOtherDir && (
                <div className="banner wide">
                    {t(
                        'Служба zapret сейчас работает из другой папки: {serviceDir}. Приложение работает с {appDir}. При включении стратегии служба будет переустановлена на папку приложения.',
                        { serviceDir: service.zapretDir ?? '', appDir: overview.zapretDir ?? '' }
                    )}
                </div>
            )}

            <div className="card">
                <h3>{t('Состояние')}</h3>
                <div className="status">
                    <span className={`big-dot ${state.color}`} />
                    {t(state.text)}
                </div>
                <dl className="compact">
                    <dt>{t('Стратегия')}</dt>
                    <dd>
                        {service.strategyId ? (
                            <b>{strategyTitle(service.strategyId)}</b>
                        ) : (
                            <span className="muted">{t('не установлена')}</span>
                        )}
                    </dd>
                    <dt>{t('Автозапуск')}</dt>
                    <dd className="wrap">
                        <Switch
                            checked={autostartMode !== null}
                            disabled={isBusy || autostart.isBusy || !isInstalled}
                            title={t(
                                'Включить — режим «Как в zapret». Другой режим — в Настройках'
                            )}
                            onChange={autostart.setEnabled}
                        />
                        <span className={autostartMode !== null ? '' : 'muted'}>
                            {autostartMode !== null ? t('вкл.') : t('выкл.')}
                        </span>
                        <span className="muted small autostart-mode">
                            {autostartText(autostartMode)}
                        </span>
                    </dd>
                    <dt>zapret</dt>
                    <dd className="wrap">
                        <span title={overview.zapretDir ?? ''}>{overview.version ?? '—'}</span>
                        {overview.hasUpdate && overview.latestVersion && (
                            <>
                                <span className="tag warn">
                                    {t('есть {version}', { version: overview.latestVersion })}
                                </span>
                                <button
                                    className="btn link inline"
                                    disabled={isBusy}
                                    onClick={update.start}
                                >
                                    {t('Обновить')}
                                </button>
                            </>
                        )}
                    </dd>
                </dl>
                {!isInstalled && (
                    <p className="muted hint-text">
                        {t(
                            'Подберите стратегию или выберите её вручную — обход включится. Автозапуск можно включить потом, когда убедитесь, что стратегия подходит.'
                        )}
                    </p>
                )}
                <div className="btns">
                    <button
                        className="btn primary"
                        disabled={isBusy}
                        title={t(
                            'Перебрать все стратегии и проверить, открываются ли YouTube и Discord. Около 2–3 минут'
                        )}
                        onClick={pick.start}
                    >
                        🔍 {t('Подобрать стратегию')}
                    </button>
                    <StrategyMenu
                        strategies={overview.strategies}
                        currentStrategyId={currentStrategyId}
                        pickResults={pickResult?.results ?? []}
                        label={isInstalled ? t('Сменить') : t('Выбрать')}
                        disabled={isBusy}
                        onSelect={handleInstall}
                    />
                    {isInstalled && isRunning && (
                        <button
                            className="btn"
                            disabled={isBusy}
                            title={onlyNowHint}
                            onClick={() => runAction(t('Выключаю обход…'), zapretStop)}
                        >
                            ⏸ {t('Выключить обход')}
                        </button>
                    )}
                    {isInstalled && !isRunning && (
                        <button
                            className="btn"
                            disabled={isBusy}
                            title={onlyNowHint}
                            onClick={() => runAction(t('Включаю обход…'), zapretStart)}
                        >
                            ▶ {t('Включить обход')}
                        </button>
                    )}
                </div>
            </div>

            <QuickPickCard
                result={pickResult}
                isStale={isPickStale}
                strategies={overview.strategies}
                progress={pick.progress}
                timing={pick.timing}
                isRunning={pick.isRunning}
                currentStrategyId={currentStrategyId}
                disabled={isBusy}
                onCancel={pick.cancel}
                onApply={handleInstall}
                deepResult={deepPick.result}
                isDeepRunning={deepPick.isRunning}
                onOpenDiag={onOpenDiag}
            />

            <ServicesCard
                serviceChecks={serviceChecks}
                services={services}
                disabled={pick.isRunning}
                isBypassOn={isRunning}
            />
        </div>
    );
}

/** EN: What the autostart does in this mode. RU: Что делает автозапуск в этом режиме. */
function autostartText(mode: AutostartMode | null): string {
    if (mode === 'app') {
        return t('через NetPilot');
    }
    return mode === 'zapret' ? t('при включении ПК') : t('zapret не стартует сам при включении ПК');
}
