import { Lang } from '../i18n';

interface ILanguageSwitchProps {
    lang: Lang;
    onChange: (lang: Lang) => void;
}

const LANGS: { id: Lang; title: string }[] = [
    { id: 'ru', title: 'RU' },
    { id: 'en', title: 'EN' },
];

/** EN: RU | EN switch. RU: Переключатель RU | EN. */
export function LanguageSwitch({ lang, onChange }: ILanguageSwitchProps) {
    return (
        <div className="lang-switch" role="group">
            {LANGS.map((item: { id: Lang; title: string }) => (
                <button
                    key={item.id}
                    className={item.id === lang ? 'active' : ''}
                    aria-pressed={item.id === lang}
                    onClick={() => onChange(item.id)}
                >
                    {item.title}
                </button>
            ))}
        </div>
    );
}
