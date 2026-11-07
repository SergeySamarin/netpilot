import { useCallback, useEffect, useState } from 'react';
import { getBrowsersQuic, IBrowsersQuic, setBrowsersQuic } from '../api/diag';
import { errorText } from '../utils/format';

export interface IBrowsersQuicState {
    status: IBrowsersQuic | null;
    isBusy: boolean;
    error: string | null;
    /**
     * EN: Changed in this session: the browsers pick it up after a restart.
     * RU: Изменено в этот запуск: браузеры подхватят после перезапуска.
     */
    isChanged: boolean;
    setEnabled: (enabled: boolean) => Promise<void>;
}

/**
 * EN: QUIC in the browsers: the current state of their policies and the switch.
 * RU: QUIC в браузерах: текущее состояние их политик и переключатель.
 */
export function useBrowsersQuic(): IBrowsersQuicState {
    const [status, setStatus] = useState<IBrowsersQuic | null>(null);
    const [isBusy, setIsBusy] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [isChanged, setIsChanged] = useState(false);

    useEffect(() => {
        getBrowsersQuic()
            .then(setStatus)
            .catch((e: unknown) => setError(errorText(e)));
    }, []);

    const setEnabled = useCallback(async (enabled: boolean): Promise<void> => {
        setIsBusy(true);
        setError(null);
        try {
            setStatus(await setBrowsersQuic(enabled));
            setIsChanged(true);
        } catch (e) {
            setError(errorText(e));
        } finally {
            setIsBusy(false);
        }
    }, []);

    return { status, isBusy, error, isChanged, setEnabled };
}
