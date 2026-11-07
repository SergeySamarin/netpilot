import { useCallback, useEffect, useState } from 'react';
import { forgetZapretDir, getZapretOverview, IZapretOverview } from './api/zapret';
import { DiagTab } from './components/DiagTab';
import { LanguageSwitch } from './components/LanguageSwitch';
import { LogRecorder } from './components/LogRecorder';
import { SettingsTab } from './components/SettingsTab';
import { Welcome } from './components/Welcome';
import { ZapretTab } from './components/ZapretTab';
import { useLogRecording } from './hooks/useLogRecording';
import { useQuickPick } from './hooks/useQuickPick';
import { useServiceChecks } from './hooks/useServiceChecks';
import { useVideoCheck } from './hooks/useVideoCheck';
import { useBrowsersQuic } from './hooks/useBrowsersQuic';
import { useDeepPick } from './hooks/useDeepPick';
import { useZapretServices } from './hooks/useZapretServices';
import { useAutostart } from './hooks/useAutostart';
import { useAppUpdate } from './hooks/useAppUpdate';
import { onZapretStateChanged, RUSSIA } from './api/app';
import { isPickForDir } from './api/checks';
import { useZapretUpdate } from './hooks/useZapretUpdate';
import { Lang, loadLanguage, saveLanguage, setLang as applyLang, t, tk } from './i18n';
import { logUi } from './api/call';
import { countryName, errorText } from './utils/format';
import './App.css';

type TabId = 'zapret' | 'proxy' | 'diag' | 'settings';

const MS_IN_SECOND = 1000;
const SECONDS_IN_DAY = 86400;

// EN: Titles are translation keys, t() is called at render time.
// RU: Заголовки — ключи перевода, t() вызывается при отрисовке.
const TABS: { id: TabId; title: string }[] = [
    { id: 'zapret', title: tk('Россия · zapret') },
    { id: 'proxy', title: tk('Зарубежные · прокси') },
    { id: 'diag', title: tk('Диагностика') },
    { id: 'settings', title: tk('Настройки') },
];

function App() {
    const [lang, setLang] = useState<Lang | null>(null);
    const [tab, setTab] = useState<TabId>('zapret');
    const [overview, setOverview] = useState<IZapretOverview | null>(null);
    const [isLoading, setIsLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const refresh = useCallback(() => {
        setIsLoading(true);
        getZapretOverview()
            .then((result: IZapretOverview) => {
                setOverview(result);
                setError(null);
            })
            .catch((e: unknown) => setError(errorText(e)))
            .finally(() => setIsLoading(false));
    }, []);

    // EN: The language comes first: until it is known nothing is drawn, so texts don't flicker.
    // RU: Сначала язык: пока он неизвестен, ничего не рисуем, чтобы тексты не мигали.
    useEffect(() => {
        loadLanguage()
            .catch(() => 'ru' as Lang)
            .then((loaded: Lang) => {
                applyLang(loaded);
                setLang(loaded);
                refresh();
            });
    }, [refresh]);

    // EN: The pick lives here, not in the tab: it must not be lost when switching tabs.
    // RU: Подбор живёт здесь, а не во вкладке: при переключении вкладок он не должен теряться.
    const pick = useQuickPick(refresh);
    const recording = useLogRecording();
    const videoCheck = useVideoCheck();
    const deepPick = useDeepPick(refresh);
    const browsersQuic = useBrowsersQuic();
    const autostart = useAutostart(refresh);
    const appUpdate = useAppUpdate();
    const zapretServices = useZapretServices(
        `${overview?.zapretDir ?? ''}|${overview?.version ?? ''}`
    );

    // EN: Checks are re-run only when the bypass state or the strategy changes.
    // RU: Проверки перезапускаются только при смене состояния обхода или стратегии.
    const checksKey =
        overview && overview.zapretDir
            ? `${overview.service.state}|${overview.service.strategyId ?? ''}`
            : null;
    const update = useZapretUpdate(setOverview);
    const isServiceChanging =
        overview?.service.state === 'starting' || overview?.service.state === 'stopping';
    const serviceChecks = useServiceChecks(
        checksKey,
        pick.isRunning || update.isRunning || deepPick.isRunning || isServiceChanging
    );

    // EN: Update check at most once a day, in the background.
    // RU: Проверка обновлений не чаще раза в сутки, в фоне.
    const isUpdateCheckDue =
        overview !== null &&
        overview.zapretDir !== null &&
        (overview.updateCheckedAt === null ||
            Date.now() / MS_IN_SECOND - overview.updateCheckedAt > SECONDS_IN_DAY);
    const { check: checkUpdate } = update;
    useEffect(() => {
        if (isUpdateCheckDue) {
            checkUpdate(true);
        }
    }, [isUpdateCheckDue, checkUpdate]);

    // EN: The tray menu or a country change turned the service on or off — re-read.
    // RU: Меню трея или смена страны включили или выключили службу — перечитываем.
    const { reload: reloadAutostart } = autostart;
    useEffect(() => {
        const unlisten = onZapretStateChanged(() => {
            refresh();
            reloadAutostart();
        });
        return () => {
            unlisten.then((stop: () => void) => stop());
        };
    }, [refresh, reloadAutostart]);

    // EN: The service may be changed outside (service.bat, services.msc) — re-read when the window gets focus.
    // RU: Службу могли изменить снаружи (service.bat, services.msc) — перечитываем при возврате в окно.
    useEffect(() => {
        window.addEventListener('focus', refresh);
        return () => window.removeEventListener('focus', refresh);
    }, [refresh]);

    const handleLangChange = useCallback(
        (next: Lang) => {
            applyLang(next);
            setLang(next);
            saveLanguage(next).catch((e: unknown) => setError(errorText(e)));
            // EN: Messages from Rust (folder errors etc.) are built in the language — re-read them.
            // RU: Сообщения из Rust (ошибки папки и т. п.) строятся на языке — перечитываем их.
            refresh();
        },
        [refresh]
    );

    const handleForget = useCallback(() => {
        forgetZapretDir()
            .then((result: IZapretOverview) => {
                setOverview(result);
                setTab('zapret');
            })
            .catch((e: unknown) => setError(errorText(e)));
    }, []);

    if (!lang) {
        return null;
    }

    const isFirstRun = overview !== null && overview.zapretDir === null;

    return (
        <div className="app">
            <header className="header">
                <div className="logo">N</div>
                <div>
                    <div className="app-name">NetPilot</div>
                    <div className="sub">{t('Обход ограничений')}</div>
                </div>
                <div className="header-right">
                    {autostart.status?.mode === 'app' && autostart.country && (
                        <div
                            className={`geo ${autostart.country.code === RUSSIA ? '' : 'foreign'}`}
                            title={t(
                                'Страна по внешнему IP — в режиме автозапуска «Через NetPilot»'
                            )}
                        >
                            <b>{countryName(autostart.country.code)}</b>
                            <span className="muted small">{t('по IP')}</span>
                        </div>
                    )}
                    <LogRecorder recording={recording} />
                    <LanguageSwitch lang={lang} onChange={handleLangChange} />
                </div>
            </header>

            {!isFirstRun && (
                <nav className="tabs">
                    {TABS.map((item: { id: TabId; title: string }) => (
                        <button
                            key={item.id}
                            className={`tab ${tab === item.id ? 'active' : ''}`}
                            onClick={() => {
                                logUi(`tab ${item.id}`);
                                setTab(item.id);
                            }}
                        >
                            {item.id === 'zapret' && overview && (
                                <span
                                    className={`dot ${pick.isRunning || update.isRunning || deepPick.isRunning ? 'yellow' : zapretDotColor(overview)}`}
                                    title={zapretDotTitle(
                                        overview,
                                        pick.isRunning,
                                        update.isRunning
                                    )}
                                />
                            )}
                            <span className="tab-label" data-text={t(item.title)}>
                                {t(item.title)}
                            </span>
                        </button>
                    ))}
                </nav>
            )}

            <main className="body">
                {error && <div className="banner error">{error}</div>}
                {appUpdate.available && tab !== 'settings' && (
                    <div className="banner info banner-row app-update-banner">
                        <span>
                            {t('Доступна новая версия NetPilot {version}.', {
                                version: appUpdate.available.version,
                            })}
                        </span>
                        <button className="btn" onClick={() => setTab('settings')}>
                            {t('Подробнее')}
                        </button>
                    </div>
                )}
                {!overview && !error && <div className="placeholder">{t('Загрузка…')}</div>}
                {overview && isFirstRun && <Welcome overview={overview} onReady={setOverview} />}
                {overview && !isFirstRun && (
                    <>
                        {tab === 'zapret' && (
                            <ZapretTab
                                overview={overview}
                                isLoading={isLoading || deepPick.isRunning}
                                pick={pick}
                                serviceChecks={serviceChecks}
                                update={update}
                                onForget={handleForget}
                                onChange={setOverview}
                                deepPick={deepPick}
                                onOpenDiag={() => setTab('diag')}
                                services={zapretServices}
                                autostart={autostart}
                            />
                        )}
                        {tab === 'proxy' && (
                            <div className="card placeholder">
                                {t('Прокси для зарубежных сервисов — позже.')}
                            </div>
                        )}
                        {tab === 'diag' && (
                            <DiagTab
                                check={videoCheck}
                                deep={deepPick}
                                browsersQuic={browsersQuic}
                                overview={overview}
                                hasQuickPick={
                                    pick.result !== null &&
                                    isPickForDir(pick.result, overview.zapretDir, overview.version)
                                }
                                isBusy={pick.isRunning || update.isRunning}
                                onChange={setOverview}
                                onQuickPick={() => {
                                    setTab('zapret');
                                    pick.start().catch(() => undefined);
                                }}
                            />
                        )}
                        {tab === 'settings' && (
                            <SettingsTab
                                overview={overview}
                                pick={pick}
                                update={update}
                                onChange={setOverview}
                                onForget={handleForget}
                                autostart={autostart}
                                appUpdate={appUpdate}
                            />
                        )}
                    </>
                )}
            </main>
        </div>
    );
}

/**
 * EN: Dot color on the "Russia" tab: green — bypass works, yellow — a transition state.
 * RU: Цвет точки на вкладке «Россия»: зелёная — обход работает, жёлтая — переходное состояние.
 */
function zapretDotColor(overview: IZapretOverview): string {
    switch (overview.service.state) {
        case 'running':
            return 'green';
        case 'starting':
        case 'stopping':
            return 'yellow';
        default:
            return 'grey';
    }
}

function zapretDotTitle(
    overview: IZapretOverview,
    isPicking: boolean,
    isUpdating: boolean
): string {
    if (isPicking) {
        return t('Идёт подбор стратегии');
    }
    if (isUpdating) {
        return t('Идёт обновление zapret');
    }
    return overview.service.state === 'running' ? t('Обход работает') : t('Обход выключен');
}

export default App;
