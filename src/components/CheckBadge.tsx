import { CheckStatus, IServiceCheck } from '../api/checks';
import { t, tk } from '../i18n';

const STATUS_TEXT: Record<CheckStatus, { text: string; className: string }> = {
    ok: { text: tk('открывается'), className: 'ok' },
    partial: { text: tk('частично'), className: 'warn' },
    fail: { text: tk('не открывается'), className: 'bad' },
};

/** EN: Short service names for the narrow pick table. RU: Короткие имена сервисов для узкой таблицы подбора. */
const SHORT_NAMES: Record<string, string> = {
    youtube: 'YT',
    discord: 'DC',
    twitch: '😀',
};

interface ICheckBadgeProps {
    check: IServiceCheck;
    /**
     * EN: Show the short service name (YT, DC) instead of the status; the status is in the color and hint.
     * RU: Вместо статуса — короткое имя сервиса (YT, DC), статус виден по цвету и подсказке.
     */
    short?: boolean;
}

/**
 * EN: Service check badge. Yellow — opened, but not stable.
 * RU: Метка результата проверки сервиса. Жёлтая — открылся, но нестабильно.
 */
export function CheckBadge({ check, short }: ICheckBadgeProps) {
    const status = STATUS_TEXT[check.status];
    const statusText = t(status.text);
    const hint = check.flaky
        ? t('{name}: {status}, прошло {passed} из {total}, нестабильно: {flaky}', {
              name: check.name,
              status: statusText,
              passed: check.passed,
              total: check.total,
              flaky: check.flaky,
          })
        : t('{name}: {status}, прошло {passed} из {total}', {
              name: check.name,
              status: statusText,
              passed: check.passed,
              total: check.total,
          });
    const className = check.status === 'ok' && check.flaky ? 'warn' : status.className;
    return (
        <span className={`tag ${className}`} title={hint}>
            {short ? (SHORT_NAMES[check.id] ?? check.name) : statusText}
        </span>
    );
}
