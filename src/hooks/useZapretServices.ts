import { useCallback, useEffect, useState } from 'react';
import {
    addCustomDomain,
    AddResult,
    addUserExclusion,
    dismissFreshDomains,
    getZapretServices,
    IServicesChange,
    IServicesState,
    removeCustomDomain,
    removeUserExclusion,
    setZapretService,
} from '../api/services';
import { errorText } from '../utils/format';

export interface IZapretServices {
    state: IServicesState | null;
    isBusy: boolean;
    error: string | null;
    /** EN: Result of the last added own domain. RU: Итог последнего добавления своего домена. */
    lastAdded: AddResult | null;
    /** EN: true — the change is saved. RU: true — изменение сохранено. */
    setEnabled: (id: string, enabled: boolean) => Promise<boolean>;
    addCustom: (domain: string) => Promise<boolean>;
    removeCustom: (domain: string) => void;
    addExclusion: (domain: string) => Promise<boolean>;
    removeExclusion: (domain: string) => void;
    dismissFresh: () => void;
}

/**
 * EN: zapret lists as services. Re-read when the zapret folder or version changes (`listsKey`).
 * RU: Списки zapret как сервисы. Перечитываются при смене папки или версии zapret (`listsKey`).
 */
export function useZapretServices(listsKey: string): IZapretServices {
    const [state, setState] = useState<IServicesState | null>(null);
    const [isBusy, setIsBusy] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [lastAdded, setLastAdded] = useState<AddResult | null>(null);

    useEffect(() => {
        getZapretServices()
            .then(setState)
            .catch((e: unknown) => setError(errorText(e)));
    }, [listsKey]);

    // EN: Every change returns the new state; the error stays until the next change.
    // RU: Каждое изменение возвращает новое состояние; ошибка видна до следующего изменения.
    const run = useCallback(async (action: () => Promise<IServicesState>): Promise<boolean> => {
        setIsBusy(true);
        setError(null);
        try {
            setState(await action());
            return true;
        } catch (e) {
            setError(errorText(e));
            return false;
        } finally {
            setIsBusy(false);
        }
    }, []);

    const setEnabled = useCallback(
        (id: string, enabled: boolean) => run(() => setZapretService(id, enabled)),
        [run]
    );

    const addCustom = useCallback(
        (domain: string) =>
            run(async () => {
                const change: IServicesChange = await addCustomDomain(domain);
                setLastAdded(change.added);
                return change.state;
            }),
        [run]
    );

    const removeCustom = useCallback(
        (domain: string) => {
            setLastAdded(null);
            run(() => removeCustomDomain(domain)).catch(() => undefined);
        },
        [run]
    );

    const addExclusion = useCallback(
        (domain: string) => run(() => addUserExclusion(domain)),
        [run]
    );

    const removeExclusion = useCallback(
        (domain: string) => {
            run(() => removeUserExclusion(domain)).catch(() => undefined);
        },
        [run]
    );

    const dismissFresh = useCallback(() => {
        run(dismissFreshDomains).catch(() => undefined);
    }, [run]);

    return {
        dismissFresh,
        state,
        isBusy,
        error,
        lastAdded,
        setEnabled,
        addCustom,
        removeCustom,
        addExclusion,
        removeExclusion,
    };
}
