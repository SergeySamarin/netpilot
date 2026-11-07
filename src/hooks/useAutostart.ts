import { useCallback, useEffect, useState } from 'react';
import {
    AutostartMode,
    getAppOptions,
    getAutostart,
    getCountry,
    IAppOptions,
    IAutostartStatus,
    ICountry,
    onCountryChanged,
    setAppOptions,
    setAutostartMode,
} from '../api/app';
import { errorText } from '../utils/format';

export interface IAutostart {
    status: IAutostartStatus | null;
    options: IAppOptions | null;
    country: ICountry | null;
    isBusy: boolean;
    error: string | null;
    /** EN: On — always "as in zapret"; off — no autostart. RU: Вкл — всегда «как в zapret»; выкл — без автозапуска. */
    setEnabled: (enabled: boolean) => void;
    setMode: (mode: AutostartMode) => void;
    setOptions: (options: IAppOptions) => void;
    /** EN: Re-read: the service could change the start type (install, update). RU: Перечитать: служба могла сменить тип запуска (установка, обновление). */
    reload: () => void;
}

/**
 * EN: Autostart, window options and the country — shared by the main tab and the settings.
 * RU: Автозапуск, настройки окна и страна — общие для главной и настроек.
 */
export function useAutostart(onServiceChanged: () => void): IAutostart {
    const [status, setStatus] = useState<IAutostartStatus | null>(null);
    const [options, setOptionsState] = useState<IAppOptions | null>(null);
    const [country, setCountry] = useState<ICountry | null>(null);
    const [isBusy, setIsBusy] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const reload = useCallback(() => {
        getAutostart()
            .then(setStatus)
            .catch((e: unknown) => setError(errorText(e)));
    }, []);

    useEffect(() => {
        reload();
        getAppOptions()
            .then(setOptionsState)
            .catch((e: unknown) => setError(errorText(e)));
        getCountry()
            .then(setCountry)
            .catch(() => undefined);
        const unlisten = onCountryChanged(setCountry);
        return () => {
            unlisten.then((stop: () => void) => stop());
        };
    }, [reload]);

    const apply = useCallback(
        (mode: AutostartMode | null) => {
            setIsBusy(true);
            setError(null);
            setAutostartMode(mode)
                .then(setStatus)
                .catch((e: unknown) => setError(errorText(e)))
                .finally(() => {
                    setIsBusy(false);
                    onServiceChanged();
                });
        },
        [onServiceChanged]
    );

    const setEnabled = useCallback((enabled: boolean) => apply(enabled ? 'zapret' : null), [apply]);

    const setOptions = useCallback((next: IAppOptions) => {
        setOptionsState(next);
        setAppOptions(next)
            .then(setOptionsState)
            .catch((e: unknown) => setError(errorText(e)));
    }, []);

    return {
        status,
        options,
        country,
        isBusy,
        error,
        setEnabled,
        setMode: apply,
        setOptions,
        reload,
    };
}
