import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { call } from './call';

/** EN: zapret — the service starts with Windows, app — NetPilot starts at logon. RU: zapret — служба стартует с Windows, app — NetPilot стартует при входе. */
export type AutostartMode = 'zapret' | 'app';

export interface IAutostartStatus {
    /** EN: null — autostart is off. RU: null — автозапуск выключен. */
    mode: AutostartMode | null;
    serviceInstalled: boolean;
}

export interface IAppOptions {
    closeToTray: boolean;
    /** EN: In the "through NetPilot" mode turn zapret on only in Russia. RU: В режиме «через NetPilot» включать zapret только в России. */
    russiaOnly: boolean;
}

export interface ICountry {
    /** EN: ISO code: RU, DE… RU: Код ISO: RU, DE… */
    code: string;
    /** EN: Unix seconds. RU: Секунды Unix. */
    checkedAt: number;
}

export const RUSSIA = 'RU';

export function getAutostart(): Promise<IAutostartStatus> {
    return call('get_autostart');
}

/** EN: Administrator rights (UAC); null — off. RU: Права администратора (UAC); null — выключить. */
export function setAutostartMode(mode: AutostartMode | null): Promise<IAutostartStatus> {
    return call('set_autostart_mode', { mode });
}

export function getAppOptions(): Promise<IAppOptions> {
    return call('get_app_options');
}

export function setAppOptions(options: IAppOptions): Promise<IAppOptions> {
    return call('set_app_options', { ...options });
}

export function getCountry(): Promise<ICountry | null> {
    return call('get_country');
}

export function onCountryChanged(handler: (country: ICountry) => void): Promise<UnlistenFn> {
    return listen<ICountry>('country-changed', (event) => handler(event.payload));
}

/**
 * EN: The service was turned on or off outside the window (tray menu, country change).
 * RU: Службу включили или выключили не из окна (меню трея, смена страны).
 */
export function onZapretStateChanged(handler: () => void): Promise<UnlistenFn> {
    return listen('zapret-state-changed', () => handler());
}
