import { AutostartMode } from '../api/app';
import { IAutostart } from '../hooks/useAutostart';
import { t, tk } from '../i18n';
import { Switch } from './Switch';

interface IAutostartCardsProps {
    autostart: IAutostart;
    /** EN: A pick or an update changes the service now. RU: Сейчас службу меняет подбор или обновление. */
    isLocked: boolean;
}

interface IModeInfo {
    id: AutostartMode;
    title: string;
    text: string;
    benefits: string[];
    note: string;
}

// EN: Texts are translation keys, t() is called at render time. RU: Тексты — ключи перевода, t() вызывается при отрисовке.
const MODES: IModeInfo[] = [
    {
        id: 'zapret',
        title: tk('Как в zapret'),
        text: tk(
            'Служба zapret запускается вместе с Windows, ещё до входа в систему. NetPilot для этого не нужен — его можно не открывать вовсе.'
        ),
        benefits: [
            tk('Настроил один раз и забыл'),
            tk('Работает, даже если NetPilot закрыт или удалён'),
            tk('Обход есть сразу после включения ПК'),
        ],
        note: tk('Подходит, если вы всегда в России и вам не нужен значок в трее.'),
    },
    {
        id: 'app',
        title: tk('Через NetPilot'),
        text: tk(
            'При входе в Windows NetPilot запускается в трее, проверяет страну и сам включает zapret. Служба сама больше не стартует — ею управляет NetPilot.'
        ),
        benefits: [
            tk('Вне России обход не включается — за границей zapret не нужен и только мешает'),
            tk('Вернулись в Россию — обход включится сам'),
            tk('Значок в трее: видно, работает ли обход, вкл/выкл в два клика'),
            tk('Понадобится для зарубежных сервисов через прокси, когда они появятся'),
        ],
        note: tk(
            'Подходит, если вы бываете за границей или хотите видеть состояние обхода в трее.'
        ),
    },
];

/**
 * EN: "Autostart" and "Window" cards of the settings. The autostart switch is the same as on the main tab.
 * RU: Карточки «Автозапуск» и «Окно» в настройках. Переключатель автозапуска — тот же, что на главной.
 */
export function AutostartCards({ autostart, isLocked }: IAutostartCardsProps) {
    const { status, options } = autostart;
    const mode = status?.mode ?? null;
    const isOn = mode !== null;
    const isDisabled = autostart.isBusy || isLocked || !status?.serviceInstalled;
    let subtitle = t('Как именно — выберите ниже');
    if (!status?.serviceInstalled) {
        subtitle = t('Сначала установите службу: подберите или выберите стратегию');
    } else if (!isOn) {
        subtitle = t('Выключен: zapret не стартует сам при включении ПК, NetPilot при входе тоже');
    }

    return (
        <>
            <div className="card wide">
                <div className="autostart-head">
                    <div>
                        <b>{t('Автозапуск')}</b>
                        <div className="muted small">{subtitle}</div>
                    </div>
                    <Switch
                        checked={isOn}
                        disabled={isDisabled}
                        title={t(
                            'Включить — режим «Как в zapret». Другой режим можно выбрать ниже'
                        )}
                        onChange={autostart.setEnabled}
                    />
                </div>
                <div className={`modes ${isOn && !isDisabled ? '' : 'off'}`}>
                    {MODES.map((item: IModeInfo) => (
                        <button
                            key={item.id}
                            className={`mode ${mode === item.id ? 'on' : ''}`}
                            disabled={!isOn || isDisabled}
                            onClick={() => {
                                if (mode !== item.id) {
                                    autostart.setMode(item.id);
                                }
                            }}
                        >
                            <span className="mode-title">
                                <span className={`rb ${mode === item.id ? 'on' : ''}`} />
                                {t(item.title)}
                                {item.id === 'zapret' && (
                                    <span className="tag">{t('по умолчанию')}</span>
                                )}
                            </span>
                            <span className="muted small">{t(item.text)}</span>
                            <ul>
                                {item.benefits.map((benefit: string) => (
                                    <li key={benefit}>{t(benefit)}</li>
                                ))}
                            </ul>
                            <span className="mode-note">{t(item.note)}</span>
                        </button>
                    ))}
                </div>
                {autostart.isBusy && <p className="muted small">{t('Меняю автозапуск…')}</p>}
                {autostart.error && <div className="banner error">{autostart.error}</div>}
            </div>

            {options && (
                <div className="card wide">
                    <h3>{t('Окно')}</h3>
                    <div className="setting-line">
                        <div>
                            <div>{t('Крестик сворачивает в трей')}</div>
                            <div className="muted small">
                                {t('Выйти совсем — через меню значка в трее')}
                            </div>
                        </div>
                        <Switch
                            checked={options.closeToTray}
                            onChange={(closeToTray: boolean) =>
                                autostart.setOptions({ ...options, closeToTray })
                            }
                        />
                    </div>
                    <div className={`setting-line ${mode === 'app' ? '' : 'off'}`}>
                        <div>
                            <div>{t('Включать zapret только в России')}</div>
                            <div className="muted small">
                                {mode === 'app'
                                    ? t(
                                          'За границей обход не включается — вернётесь, включится сам'
                                      )
                                    : t('Работает только в режиме «Через NetPilot»')}
                            </div>
                        </div>
                        <Switch
                            checked={options.russiaOnly}
                            disabled={mode !== 'app'}
                            onChange={(russiaOnly: boolean) =>
                                autostart.setOptions({ ...options, russiaOnly })
                            }
                        />
                    </div>
                </div>
            )}
        </>
    );
}
