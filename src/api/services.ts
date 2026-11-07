import { call } from './call';

export interface IZapretService {
    id: string;
    name: string;
    enabled: boolean;
    /** EN: Entries of the author lists (`^` — without subdomains). RU: Записи списков авторов (`^` — без поддоменов). */
    domains: string[];
}

export interface IServicesState {
    services: IZapretService[];
    /** EN: Own domains. RU: Свои домены. */
    custom: string[];
    authorExclusions: string[];
    userExclusions: string[];
    /** EN: Author entries added in the last zapret update. RU: Записи авторов, добавленные последним обновлением zapret. */
    fresh: string[];
}

export type AddResult =
    | { kind: 'added'; domain: string }
    | { kind: 'exists'; domain: string }
    | { kind: 'covered'; domain: string; service: string; by: string };

export interface IServicesChange {
    state: IServicesState;
    added: AddResult | null;
}

export function getZapretServices(): Promise<IServicesState> {
    return call('get_zapret_services');
}

/**
 * EN: Turns a service on or off; zapret re-reads the lists by itself in a few seconds.
 * RU: Включает или выключает сервис; zapret сам перечитывает списки за несколько секунд.
 */
export function setZapretService(id: string, enabled: boolean): Promise<IServicesState> {
    return call('set_zapret_service', { id, enabled });
}

/** EN: "Got it" — stop highlighting new author domains. RU: «Понятно» — перестать подсвечивать новые домены авторов. */
export function dismissFreshDomains(): Promise<IServicesState> {
    return call('dismiss_fresh_domains');
}

export function addCustomDomain(domain: string): Promise<IServicesChange> {
    return call('add_custom_domain', { domain });
}

export function removeCustomDomain(domain: string): Promise<IServicesState> {
    return call('remove_custom_domain', { domain });
}

export function addUserExclusion(domain: string): Promise<IServicesState> {
    return call('add_user_exclusion', { domain });
}

export function removeUserExclusion(domain: string): Promise<IServicesState> {
    return call('remove_user_exclusion', { domain });
}
