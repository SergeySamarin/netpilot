import { t } from '../i18n';

interface IServiceBannersProps {
    busyText: string | null;
    error: string | null;
}

/**
 * EN: Banners "action in progress, confirm in the Windows window" and "error".
 * RU: Плашки «идёт действие, подтвердите в окне Windows» и «ошибка».
 */
export function ServiceBanners({ busyText, error }: IServiceBannersProps) {
    return (
        <>
            {busyText && (
                <div className="banner info wide">
                    {busyText} {t('Если Windows спросит разрешение — нажмите «Да».')}
                </div>
            )}
            {error && <div className="banner error wide">{error}</div>}
        </>
    );
}
