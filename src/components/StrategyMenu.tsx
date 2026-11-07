import { useEffect, useRef, useState } from 'react';
import { IServiceCheck, IStrategyResult } from '../api/checks';
import { IStrategy, strategyTitle } from '../api/zapret';
import { t } from '../i18n';
import { CheckBadge } from './CheckBadge';

interface IStrategyMenuProps {
    strategies: IStrategy[];
    currentStrategyId: string | null;
    pickResults: IStrategyResult[];
    label: string;
    disabled: boolean;
    onSelect: (strategyId: string) => void;
}

/**
 * EN: "Change ▾" button with a dropdown of all strategies.
 * RU: Кнопка «Сменить ▾» с выпадающим списком всех стратегий.
 */
export function StrategyMenu({
    strategies,
    currentStrategyId,
    pickResults,
    label,
    disabled,
    onSelect,
}: IStrategyMenuProps) {
    const [isOpen, setIsOpen] = useState(false);
    const rootRef = useRef<HTMLDivElement>(null);

    // EN: Close on a click outside the menu and on Escape.
    // RU: Закрываем по клику вне меню и по Escape.
    useEffect(() => {
        if (!isOpen) {
            return undefined;
        }
        function handleClick(event: MouseEvent): void {
            if (!rootRef.current?.contains(event.target as Node)) {
                setIsOpen(false);
            }
        }
        function handleKey(event: KeyboardEvent): void {
            if (event.key === 'Escape') {
                setIsOpen(false);
            }
        }
        document.addEventListener('mousedown', handleClick);
        document.addEventListener('keydown', handleKey);
        return () => {
            document.removeEventListener('mousedown', handleClick);
            document.removeEventListener('keydown', handleKey);
        };
    }, [isOpen]);

    function handleSelect(strategyId: string): void {
        setIsOpen(false);
        onSelect(strategyId);
    }

    return (
        <div className="menu-root" ref={rootRef}>
            <button className="btn" disabled={disabled} onClick={() => setIsOpen(!isOpen)}>
                {label} ▾
            </button>
            {isOpen && (
                <div className="dropdown">
                    {strategies.map((strategy: IStrategy) => {
                        const isCurrent = strategy.id === currentStrategyId;
                        const pick = pickResults.find(
                            (r: IStrategyResult) => r.strategyId === strategy.id
                        );
                        return (
                            <button
                                key={strategy.id}
                                className={`dropdown-item ${isCurrent ? 'current' : ''}`}
                                disabled={isCurrent}
                                onClick={() => handleSelect(strategy.id)}
                            >
                                <span>{strategyTitle(strategy.id)}</span>
                                {isCurrent && <span className="tag info">{t('текущая')}</span>}
                                <span className="right">
                                    {pick?.services.map((check: IServiceCheck) => (
                                        <CheckBadge key={check.id} check={check} short />
                                    ))}
                                    {pick?.error && <span className="tag bad">{t('ошибка')}</span>}
                                    <span className="ms">
                                        {pick?.latencyMs
                                            ? t('{ms} мс', { ms: pick.latencyMs })
                                            : ''}
                                    </span>
                                </span>
                            </button>
                        );
                    })}
                </div>
            )}
        </div>
    );
}
