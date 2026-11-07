import { useCallback, useState } from 'react';
import { IZapretOverview } from '../api/zapret';
import { errorText } from '../utils/format';

interface IServiceAction {
    /**
     * EN: Text of the running action, null — nothing is running.
     * RU: Текст выполняемого действия, null — ничего не выполняется.
     */
    busyText: string | null;
    error: string | null;
    run: (text: string, action: () => Promise<IZapretOverview>) => Promise<void>;
}

/**
 * EN: A zapret service action (via UAC): shows what is running and catches the error.
 * RU: Действие со службой zapret (через UAC): показывает, что идёт, и ловит ошибку.
 */
export function useServiceAction(onChange: (overview: IZapretOverview) => void): IServiceAction {
    const [busyText, setBusyText] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);

    const run = useCallback(
        async (text: string, action: () => Promise<IZapretOverview>): Promise<void> => {
            setBusyText(text);
            setError(null);
            try {
                onChange(await action());
            } catch (e) {
                setError(errorText(e));
            } finally {
                setBusyText(null);
            }
        },
        [onChange]
    );

    return { busyText, error, run };
}
