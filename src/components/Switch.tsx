interface ISwitchProps {
    checked: boolean;
    disabled?: boolean;
    title?: string;
    onChange: (checked: boolean) => void;
}

/** EN: On/off switch. RU: Переключатель вкл/выкл. */
export function Switch({ checked, disabled, title, onChange }: ISwitchProps) {
    return (
        <button
            type="button"
            role="switch"
            aria-checked={checked}
            className={`switch ${checked ? 'on' : ''}`}
            disabled={disabled}
            title={title}
            onClick={() => onChange(!checked)}
        />
    );
}
