import { RpcClient } from './client';
import type { Friend, GameSession, HardwareProfile, Hello, Item, LibraryPage, Notification, Parental, Prefs, Profile, RemotePairing, SessionInfo, SteamAccount } from './generated';

type Auth = { pin?: string; parentPin?: string; grant?: string };

/** Métodos tipados do Core. Cada um corresponde a uma rota em rpc.rs. */
export function createApi(c: RpcClient) {
  const call = c.call.bind(c);
  return {
    hello: () => call<Hello>('core.hello'),
    hardware: () => call<HardwareProfile>('hardware.profile'),
    power: (action: 'off' | 'restart' | 'suspend') => call<{ simulated: boolean }>('system.power', { action }),
    steamAccount: () => call<SteamAccount>('steam.account'),
    steamSetKey: (key: string) => call<{ ok: true }>('steam.set_key', { key }),
    steamSync: () => call<{ count: number; hasKey: boolean }>('steam.sync'),
    profiles: () => call<Profile[]>('profiles.list'),
    createProfile: (p: { name: string; avatar: string; hue: number; kind: 'adult' | 'kid'; pin?: string; ageLimit?: number; dailyLimitMin?: number } & Auth) => call<Profile>('profiles.create', p),
    updateProfile: (p: { id: string; name?: string; avatar?: string; hue?: number; ageLimit?: number | null; dailyLimitMin?: number | null; pin?: string | null; currentPin?: string } & Omit<Auth, 'pin'>) => call<Profile>('profiles.update', p),
    deleteProfile: (p: { id: string } & Auth) => call<{ ok: true }>('profiles.delete', p),
    selectProfile: (p: { id: string } & Auth) => call<SessionInfo>('profiles.select', p),
    session: () => call<SessionInfo | null>('session.current'),
    logout: (a: Auth = {}) => call<{ ok: true }>('session.logout', a),
    parental: () => call<Parental>('parental.status'),
    parentalVerify: (pin: string) => call<{ grant: string }>('parental.verify', { pin }),
    parentalSet: (newPin: string | null, currentPin?: string) => call<Parental>('parental.set', { newPin, currentPin }),
    extend: (profileId: string, minutes: number, a: Auth) => call<{ remainingMin: number | null }>('parental.extend', { profileId, minutes, ...a }),
    allowed: (profileId: string) => call<string[]>('parental.allowed', { profileId }),
    allow: (profileId: string, itemId: string, allow: boolean, a: Auth) => call<string[]>('parental.allow', { profileId, itemId, allow, ...a }),
    setPrefs: (patch: Partial<Prefs>) => call<Prefs>('prefs.set', { patch }),
    library: () => call<LibraryPage>('library.list'),
    items: (ids: string[]) => call<Item[]>('library.get_many', { ids }),
    item: (id: string) => call<Item>('library.get', { id }),
    search: (q: string) => call<Item[]>('library.search', { q }),
    store: (category: string) => call<Item[]>('store.list', { category }),
    wallet: () => call<{ balance: number; currency: string } | null>('store.wallet'),
    friends: () => call<Friend[]>('social.friends'),
    launch: (id: string) => call<GameSession>('item.launch', { id }),
    install: (id: string) => call<{ ok: true }>('item.install', { id }),
    cancel: (id: string) => call<{ ok: true }>('item.cancel', { id }),
    uninstall: (id: string) => call<{ ok: true }>('item.uninstall', { id }),
    game: () => call<GameSession | null>('game.current'),
    closeGame: (force = false) => call<{ closing: boolean }>('game.close', { force }),
    showNexus: () => call('game.show_nexus'),
    resumeGame: () => call('game.resume'),
    notifications: () => call<Notification[]>('notifications.list'),
    clearNotifications: () => call('notifications.clear'),
    pairing: () => call<RemotePairing>('remote.pairing'),
    rotatePairing: () => call<RemotePairing>('remote.rotate'),
    disconnectRemote: (id: string) => call<RemotePairing>('remote.disconnect', { id }),
  };
}

export type Api = ReturnType<typeof createApi>;
