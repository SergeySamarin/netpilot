import { IBrowserQuic } from '../api/diag';
import { IBrowsersQuicState } from '../hooks/useBrowsersQuic';
import { plural, t } from '../i18n';
import { Switch } from './Switch';

interface IBrowsersQuicStripProps {
    state: IBrowsersQuicState;
}

/** EN: Id of the strip, to scroll to it. RU: Id полосы, чтобы прокрутить к ней. */
export const BROWSERS_QUIC_ID = 'browsers-quic';

/**
 * EN: QUIC in browsers: a strip at the top of the "Diagnostics" tab with the state and the switch; the
 *     explanation and the browser names are in hints.
 * RU: QUIC в браузерах: полоса вверху вкладки «Диагностика» с состоянием и переключателем; объяснение и
 *     названия браузеров — в подсказках.
 */
export function BrowsersQuicStrip({ state }: IBrowsersQuicStripProps) {
    const browsers = state.status?.browsers ?? [];
    const offCount = browsers.filter((browser: IBrowserQuic) => browser.quicOff).length;
    const isOn = offCount === 0;
    let statusText = t('включён');
    let statusClass = 'ok';
    if (browsers.length > 0 && offCount === browsers.length) {
        statusText = t('выключен');
        statusClass = 'warn';
    } else if (offCount > 0) {
        statusText = t('выключен не везде');
        statusClass = 'warn';
    }
    const browserNames = browsers
        .map((browser: IBrowserQuic) =>
            browser.quicOff
                ? `${browser.name} — ${t('QUIC выключен')}`
                : `${browser.name} — ${t('QUIC включён')}`
        )
        .join('\n');
    const switchHint = isOn
        ? t(
              'Выключить QUIC: браузеры всегда будут ходить обычным соединением и не будут терять время на QUIC, если обход с ним не справляется. Действует на все сайты. В настройках браузера появится надпись «Управляется организацией» — это так браузер показывает любую такую настройку. Нужны права администратора.'
          )
        : t(
              'Включить QUIC обратно: браузеры будут работать как обычно. Нужны права администратора.'
          );

    return (
        <div className={`quic-strip ${isOn ? '' : 'off'}`} id={BROWSERS_QUIC_ID}>
            <div className="quic-strip-row">
                <b>{t('QUIC в браузерах:')}</b>
                <span className={`tag ${statusClass}`}>{statusText}</span>
                {browsers.length > 0 ? (
                    <span className="muted small help" title={browserNames}>
                        {plural(browsers.length, 'в {n} браузере|в {n} браузерах|в {n} браузерах')}
                    </span>
                ) : (
                    <span className="muted small">
                        {t('браузеры не найдены (Chrome, Edge, Яндекс Браузер, Brave, Firefox)')}
                    </span>
                )}
                <span
                    className="muted small help"
                    title={t(
                        'QUIC — быстрый способ соединения (HTTP/3), которым браузер ходит на YouTube, сервисы Google и сайты за Cloudflare. Если обход с ним не справляется, браузер теряет время на каждом новом видео или зависает. Без QUIC браузер ходит обычным соединением: сайты и видео работают так же, безопасность та же. Приложения (Discord и др.) это не затрагивает.'
                    )}
                >
                    {t('Что это?')}
                </span>
                <span className="quic-strip-right">
                    <span className="muted small">
                        {t('Если обход не справляется с QUIC, его рекомендуется выключить')}
                    </span>
                    <Switch
                        checked={isOn}
                        disabled={state.isBusy || browsers.length === 0}
                        title={switchHint}
                        onChange={(next: boolean) => {
                            state.setEnabled(next).catch(() => undefined);
                        }}
                    />
                </span>
            </div>
            {state.isChanged && (
                <div className="muted small">
                    {t('Перезапустите браузер, чтобы изменение заработало.')}
                </div>
            )}
            {state.error && <div className="error-text small">{state.error}</div>}
        </div>
    );
}
