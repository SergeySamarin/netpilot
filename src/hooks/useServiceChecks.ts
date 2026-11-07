import { useCallback, useEffect, useRef, useState } from 'react';
import { checkServices, IServiceCheck } from '../api/checks';
import { errorText } from '../utils/format';

export interface IServiceChecks {
    checks: IServiceCheck[] | null;
    isChecking: boolean;
    /** EN: Services being checked now; null — all. RU: Какие сервисы проверяются сейчас; null — все. */
    checkingIds: string[] | null;
    error: string | null;
    /** EN: ids — check only these and keep the other results. RU: ids — проверить только эти, остальные результаты сохранить. */
    run: (ids?: string[]) => void;
}

/**
 * EN: Service checks. Live at the App level, so the results survive tab switches. The check runs
 *     again only when stateKey changes (bypass state or strategy) or by the "Check now" button.
 *     Only the latest check counts: an older one that finishes later is ignored.
 * RU: Проверки сервисов. Живут на уровне App, чтобы результаты не терялись при переключении вкладок.
 *     Проверка запускается заново только при смене stateKey (состояние обхода или стратегия) или по
 *     кнопке «Проверить сейчас». Учитывается только последняя проверка: более ранняя, закончившаяся
 *     позже, отбрасывается.
 */
export function useServiceChecks(stateKey: string | null, isPaused: boolean): IServiceChecks {
    const [checks, setChecks] = useState<IServiceCheck[] | null>(null);
    const [isChecking, setIsChecking] = useState(false);
    const [checkingIds, setCheckingIds] = useState<string[] | null>(null);
    const [error, setError] = useState<string | null>(null);
    const lastRequest = useRef(0);

    const run = useCallback((ids?: string[]) => {
        lastRequest.current += 1;
        const request = lastRequest.current;
        const isLatest = (): boolean => request === lastRequest.current;
        setIsChecking(true);
        setCheckingIds(ids ?? null);
        setError(null);
        checkServices(ids)
            .then((result: IServiceCheck[]) => {
                if (!isLatest()) {
                    return;
                }
                setChecks((prev: IServiceCheck[] | null) => {
                    if (!ids || !prev) {
                        return result;
                    }
                    return prev.map(
                        (check: IServiceCheck) =>
                            result.find((next: IServiceCheck) => next.id === check.id) ?? check
                    );
                });
            })
            .catch((e: unknown) => {
                if (isLatest()) {
                    setError(errorText(e));
                }
            })
            .finally(() => {
                if (isLatest()) {
                    setIsChecking(false);
                    setCheckingIds(null);
                }
            });
    }, []);

    useEffect(() => {
        // EN: No state yet (first run screen), the service is changing or a pick is running — the result would be wrong.
        // RU: Состояния ещё нет (экран первого запуска), служба меняется или идёт подбор — результат был бы неверным.
        if (stateKey !== null && !isPaused) {
            run();
        }
    }, [stateKey, isPaused, run]);

    return { checks, isChecking, checkingIds, error, run };
}
