import { openUrl } from '@tauri-apps/plugin-opener';
import { FormEvent, useEffect, useRef, useState } from 'react';
import { IServiceCheck, ITargetResult } from '../api/checks';
import { IZapretService } from '../api/services';
import { IServiceChecks } from '../hooks/useServiceChecks';
import { IZapretServices } from '../hooks/useZapretServices';
import { plural, t } from '../i18n';
import { CheckBadge } from './CheckBadge';

interface IServicesCardProps {
    serviceChecks: IServiceChecks;
    services: IZapretServices;
    disabled: boolean;
    /** EN: The bypass is running: without it the checkboxes change nothing. RU: Обход работает: без него галочки ничего не меняют. */
    isBypassOn: boolean;
}

/**
 * EN: YouTube goes through its own rule of every strategy that ignores the exclusion lists, so it can't be
 *     turned off by the lists. RU: YouTube идёт по отдельному правилу каждой стратегии, которое не читает
 *     списки исключений, поэтому выключить его списками нельзя.
 */
const LOCKED_SERVICES = ['youtube'];
/**
 * EN: A chat extension that loads the emotes of 7TV, BTTV and FFZ through its own servers — it works where their
 *     own servers are blocked. RU: Расширение для чатов, которое грузит смайлы 7TV, BTTV и FFZ через свои серверы, —
 *     работает там, где их собственные серверы заблокированы.
 */
const REYOHOHO_URL =
    'https://chromewebstore.google.com/detail/reyohoho-twitch-extension/peigjkggfaiddfnndocglngepppcfjia';

function openReyohoho(): void {
    openUrl(REYOHOHO_URL).catch(() => undefined);
}

/** EN: Domains shown collapsed. RU: Сколько доменов показывать свёрнуто. */
const SHOWN_DOMAINS = 6;
const SHOWN_EXCLUSIONS = 24;
/** EN: winws re-reads the lists in about 4 s — check after that. RU: winws перечитывает списки примерно за 4 с — проверяем после. */
const RECHECK_DELAY_MS = 5000;

/**
 * EN: "Services": which services go through zapret (checkboxes), their domains, own domains, exclusions,
 *     and the check whether YouTube and Discord open.
 * RU: «Сервисы»: какие сервисы идут через zapret (галочки), их домены, свои домены, исключения и
 *     проверка, открываются ли YouTube и Discord.
 */
export function ServicesCard({
    serviceChecks,
    services,
    disabled,
    isBypassOn,
}: IServicesCardProps) {
    const { checks, isChecking, checkingIds, error: checkError, run } = serviceChecks;
    const [isExclusionsOpen, setIsExclusionsOpen] = useState(false);
    // EN: Services toggled since the last check: their old result is not shown until the new check.
    // RU: Сервисы, переключённые после последней проверки: их старый результат не показывается до новой.
    const [pending, setPending] = useState<string[]>([]);
    const [isRecheckStarted, setIsRecheckStarted] = useState(false);
    const pendingRef = useRef<string[]>([]);
    const timer = useRef<number | null>(null);
    const state = services.state;
    const checkOf = (id: string) => checks?.find((check: IServiceCheck) => check.id === id) ?? null;

    useEffect(() => {
        if (isRecheckStarted && !isChecking) {
            pendingRef.current = [];
            setPending([]);
            setIsRecheckStarted(false);
        }
    }, [isRecheckStarted, isChecking]);

    useEffect(
        () => () => {
            if (timer.current !== null) {
                window.clearTimeout(timer.current);
            }
        },
        []
    );

    function handleToggle(id: string, enabled: boolean): void {
        services
            .setEnabled(id, enabled)
            .then((isSaved: boolean) => {
                if (!isSaved) {
                    return;
                }
                if (!pendingRef.current.includes(id)) {
                    pendingRef.current = [...pendingRef.current, id];
                    setPending(pendingRef.current);
                }
                if (timer.current !== null) {
                    window.clearTimeout(timer.current);
                }
                timer.current = window.setTimeout(() => {
                    timer.current = null;
                    run(pendingRef.current);
                    setIsRecheckStarted(true);
                }, RECHECK_DELAY_MS);
            })
            .catch(() => undefined);
    }

    return (
        <div className="card wide">
            <h3>
                {t('Сервисы')}
                <span className="hint">{t('галочка — пускать сервис через zapret')}</span>
            </h3>
            {checkError && <div className="banner error">{checkError}</div>}
            {services.error && <div className="banner error">{services.error}</div>}
            {!state && !services.error && <p className="muted">{t('Загрузка…')}</p>}
            {state && state.fresh.length > 0 && (
                <div className="banner info banner-row">
                    <span>
                        {plural(
                            state.fresh.length,
                            'После обновления zapret авторы добавили {n} домен — он подсвечен синим.|После обновления zapret авторы добавили {n} домена — они подсвечены синим.|После обновления zapret авторы добавили {n} доменов — они подсвечены синим.'
                        )}
                    </span>
                    <button className="btn" onClick={services.dismissFresh}>
                        {t('Понятно')}
                    </button>
                </div>
            )}
            {state?.services.map((service: IZapretService) => (
                <ServiceRow
                    key={service.id}
                    service={service}
                    check={checkOf(service.id)}
                    isChecking={
                        isChecking && (checkingIds === null || checkingIds.includes(service.id))
                    }
                    isPending={pending.includes(service.id)}
                    isLocked={LOCKED_SERVICES.includes(service.id)}
                    fresh={state.fresh}
                    disabled={services.isBusy || !isBypassOn}
                    onToggle={(enabled: boolean) => handleToggle(service.id, enabled)}
                />
            ))}
            {state && (
                <CustomDomains services={services} domains={state.custom} disabled={!isBypassOn} />
            )}
            {!isBypassOn && (
                <p className="muted small hint-text">
                    {t('Обход выключен — галочки заработают, когда он будет включён.')}
                </p>
            )}
            <p className="muted small hint-text">
                {t(
                    'Изменения применяются сами: zapret перечитывает списки за несколько секунд, перезапуск не нужен.'
                )}
            </p>
            <div className="btns">
                <button className="btn" disabled={disabled || isChecking} onClick={() => run()}>
                    {isChecking ? t('Проверяю…') : `↻ ${t('Проверить сейчас')}`}
                </button>
                <button className="btn link" onClick={() => setIsExclusionsOpen(!isExclusionsOpen)}>
                    {t('Исключения — их zapret не трогает')} {isExclusionsOpen ? '▴' : '▾'}
                </button>
            </div>
            {isExclusionsOpen && state && (
                <Exclusions
                    services={services}
                    authors={state.authorExclusions}
                    user={state.userExclusions}
                    disabled={!isBypassOn}
                />
            )}
        </div>
    );
}

interface IServiceRowProps {
    service: IZapretService;
    check: IServiceCheck | null;
    isChecking: boolean;
    /** EN: Toggled, waiting for the new check. RU: Переключён, ждёт новой проверки. */
    isPending: boolean;
    /** EN: Always through zapret, the checkbox can't be unchecked. RU: Всегда через zapret, галочку снять нельзя. */
    isLocked: boolean;
    /** EN: New author entries of the last update. RU: Новые записи авторов из последнего обновления. */
    fresh: string[];
    disabled: boolean;
    onToggle: (enabled: boolean) => void;
}

function ServiceRow({
    service,
    check,
    isChecking,
    isPending,
    isLocked,
    fresh,
    disabled,
    onToggle,
}: IServiceRowProps) {
    const [isOpen, setIsOpen] = useState(false);
    // EN: New domains go first, so they are visible when collapsed. RU: Новые домены — первыми, чтобы были видны свёрнуто.
    const domains = [
        ...service.domains.filter((domain: string) => fresh.includes(domain)),
        ...service.domains.filter((domain: string) => !fresh.includes(domain)),
    ];
    const shown = isOpen ? domains : domains.slice(0, SHOWN_DOMAINS);
    const rest = domains.length - shown.length;
    const shownCheck = isPending ? null : check;
    const failed = shownCheck ? failedHosts(shownCheck) : '';
    return (
        <div className={`svc-row ${service.enabled ? '' : 'off'}`}>
            <input
                type="checkbox"
                checked={service.enabled || isLocked}
                disabled={disabled || isLocked}
                title={checkboxTitle(service.enabled, isLocked)}
                onChange={() => onToggle(!service.enabled)}
            />
            <div className="svc-main">
                <div>
                    <b>{service.name}</b>
                    {service.id === 'discord' && (
                        <span
                            className="muted small"
                            title={t(
                                'Голосовые каналы Discord обходятся отдельным правилом по портам, галочка на них не влияет'
                            )}
                        >
                            {' '}
                            · {t('голосовые каналы — отдельным правилом')}
                        </span>
                    )}
                    {failed && (
                        <span className="muted small">
                            {' '}
                            · {t('не открылись: {hosts}', { hosts: failed })}
                        </span>
                    )}
                </div>
                <div className="chips">
                    {shown.map((domain: string) => (
                        <span
                            key={domain}
                            className={`chip ${fresh.includes(domain) ? 'new' : ''}`}
                            title={
                                domain.startsWith('^')
                                    ? t('Только этот домен, без поддоменов')
                                    : t('Домен и все его поддомены')
                            }
                        >
                            {domain.replace(/^\^/, '')}
                        </span>
                    ))}
                    {(rest > 0 || isOpen) && service.domains.length > SHOWN_DOMAINS && (
                        <button className="chip more" onClick={() => setIsOpen(!isOpen)}>
                            {isOpen ? t('свернуть') : `+${rest}`}
                        </button>
                    )}
                </div>
                {service.id === 'twitch' && (
                    <p className="muted small svc-note">
                        {t('Если смайлы пропадают, проще поставить расширение')}{' '}
                        <button className="btn link inline" onClick={openReyohoho}>
                            ReYohoho Twitch Extension
                        </button>
                        {t(': оно само грузит смайлы в обход блокировок.')}
                    </p>
                )}
            </div>
            <span className="right">
                {!service.enabled && <span className="tag">{t('не через zapret')}</span>}
                {isPending && check && (
                    <span className="muted small">
                        {isChecking ? t('Проверяю…') : t('проверю через несколько секунд…')}
                    </span>
                )}
                {shownCheck && (
                    <>
                        {shownCheck.flaky > 0 && (
                            <span
                                className="muted small"
                                title={t(
                                    'Адрес открылся только со второй попытки. Бывает, когда стратегия работает не идеально'
                                )}
                            >
                                {t('нестабильно: {count}', { count: shownCheck.flaky })}
                            </span>
                        )}
                        <CheckBadge check={shownCheck} />
                        <span className="ms">
                            {shownCheck.latencyMs !== null
                                ? t('{ms} мс', { ms: shownCheck.latencyMs })
                                : '—'}
                        </span>
                    </>
                )}
                {!check && isChecking && <span className="muted small">{t('Проверяю…')}</span>}
            </span>
        </div>
    );
}

interface ICustomDomainsProps {
    services: IZapretServices;
    domains: string[];
    /** EN: The bypass is off — lists can't be edited. RU: Обход выключен — списки не правятся. */
    disabled: boolean;
}

function CustomDomains({ services, domains, disabled }: ICustomDomainsProps) {
    const [input, setInput] = useState('');
    const added = services.lastAdded;
    const isDuplicate = domains.includes(roughDomain(input));

    function handleSubmit(event: FormEvent): void {
        event.preventDefault();
        if (!input.trim()) {
            return;
        }
        services
            .addCustom(input)
            .then((isOk: boolean) => {
                if (isOk) {
                    setInput('');
                }
            })
            .catch(() => undefined);
    }

    return (
        <div className="svc-row">
            <span className="svc-spacer" />
            <div className="svc-main">
                <b>{t('Свои домены')}</b>
                <span className="muted small"> · {t('сайты, которых нет в списках авторов')}</span>
                {domains.length > 0 && (
                    <div className="chips">
                        {domains.map((domain: string) => (
                            <span key={domain} className="chip">
                                {domain}
                                <button
                                    className="chip-x"
                                    title={t('Убрать')}
                                    disabled={services.isBusy || disabled}
                                    onClick={() => services.removeCustom(domain)}
                                >
                                    ✕
                                </button>
                            </span>
                        ))}
                    </div>
                )}
                <form className="add-domain" onSubmit={handleSubmit}>
                    <input
                        value={input}
                        disabled={disabled}
                        placeholder={t('например, rutracker.org')}
                        onChange={(event) => setInput(event.target.value)}
                    />
                    <button
                        className="btn"
                        type="submit"
                        disabled={services.isBusy || disabled || !input.trim() || isDuplicate}
                    >
                        {t('Добавить')}
                    </button>
                </form>
                {isDuplicate && (
                    <p className="muted small">
                        {t('{domain} уже есть в своих доменах.', { domain: roughDomain(input) })}
                    </p>
                )}
                {!isDuplicate && added?.kind === 'covered' && (
                    <p className="muted small">
                        {t(
                            '{domain} уже идёт через zapret: это «{service}» (в списке есть {by}).',
                            {
                                domain: added.domain,
                                service: added.service,
                                by: added.by.replace(/^\^/, ''),
                            }
                        )}
                    </p>
                )}
                {!isDuplicate && added?.kind === 'exists' && (
                    <p className="muted small">
                        {t('{domain} уже есть в своих доменах.', { domain: added.domain })}
                    </p>
                )}
                {!isDuplicate && added?.kind === 'added' && (
                    <p className="muted small">
                        {t('{domain} добавлен — zapret подхватит его за несколько секунд.', {
                            domain: added.domain,
                        })}
                    </p>
                )}
            </div>
        </div>
    );
}

interface IExclusionsProps {
    services: IZapretServices;
    authors: string[];
    user: string[];
    /** EN: The bypass is off — lists can't be edited. RU: Обход выключен — списки не правятся. */
    disabled: boolean;
}

function Exclusions({ services, authors, user, disabled }: IExclusionsProps) {
    const [input, setInput] = useState('');
    const [isAll, setIsAll] = useState(false);
    const shown = isAll ? authors : authors.slice(0, SHOWN_EXCLUSIONS);

    function handleSubmit(event: FormEvent): void {
        event.preventDefault();
        if (!input.trim()) {
            return;
        }
        services
            .addExclusion(input)
            .then((isOk: boolean) => {
                if (isOk) {
                    setInput('');
                }
            })
            .catch(() => undefined);
    }

    return (
        <div className="exclusions">
            <p className="muted small">
                {t(
                    'Сайты, которые работают и без обхода: zapret их не трогает, чтобы ничего не сломать. Свои исключения можно добавить и убрать.'
                )}
            </p>
            {user.length > 0 && (
                <div className="chips">
                    {user.map((domain: string) => (
                        <span key={domain} className="chip new">
                            {domain}
                            <button
                                className="chip-x"
                                title={t('Убрать')}
                                disabled={services.isBusy || disabled}
                                onClick={() => services.removeExclusion(domain)}
                            >
                                ✕
                            </button>
                        </span>
                    ))}
                </div>
            )}
            <form className="add-domain" onSubmit={handleSubmit}>
                <input
                    value={input}
                    disabled={disabled}
                    placeholder={t('домен, который zapret не должен трогать')}
                    onChange={(event) => setInput(event.target.value)}
                />
                <button
                    className="btn"
                    type="submit"
                    disabled={services.isBusy || disabled || !input.trim()}
                >
                    {t('Добавить исключение')}
                </button>
            </form>
            <div className="chips">
                {shown.map((domain: string) => (
                    <span key={domain} className="chip">
                        {domain.replace(/^\^/, '')}
                    </span>
                ))}
                {authors.length > SHOWN_EXCLUSIONS && (
                    <button className="chip more" onClick={() => setIsAll(!isAll)}>
                        {isAll
                            ? t('свернуть')
                            : t('ещё {count}', { count: authors.length - SHOWN_EXCLUSIONS })}
                    </button>
                )}
            </div>
        </div>
    );
}

/**
 * EN: Addresses that did not open — to show what exactly doesn't work.
 * RU: Адреса, которые не открылись, — чтобы было видно, что именно не работает.
 */
function failedHosts(check: IServiceCheck): string {
    const failed = check.targets.filter(
        (target: ITargetResult) => target.tls12Ms === null || target.tls13Ms === null
    );
    return failed.map((target: ITargetResult) => target.host).join(', ');
}

function checkboxTitle(isEnabled: boolean, isLocked: boolean): string {
    if (isLocked) {
        return t(
            'YouTube обходится отдельным правилом стратегии — выключить его можно только вместе со всем обходом'
        );
    }
    return isEnabled
        ? t('Снять — zapret перестанет трогать домены этого сервиса')
        : t('Отметить — пускать сервис через zapret');
}

/**
 * EN: The domain from the input as Rust will see it (scheme, path and "www." dropped) — to catch a
 *     duplicate before pressing the button. RU: Домен из поля так, как его увидит Rust (без схемы, пути и
 *     «www.»), — чтобы заметить повтор ещё до нажатия.
 */
function roughDomain(input: string): string {
    const host = input
        .trim()
        .toLowerCase()
        .replace(/^[a-z]+:\/\//, '')
        .split(/[/?#:]/)[0];
    return host.replace(/^www\./, '').replace(/\.$/, '');
}
