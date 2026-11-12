/** Pendrives/HDs removíveis e pastas para buscar ROMs (aba Emuladores → Adicionar jogos). */
export interface RomMedia {
  media: { dev: string; label: string; mount: string | null; size: number; fs: string }[];
  home: string;
  romsDir: string;
}
export interface RomListing {
  path: string;
  parent: string | null;
  system: string | null;
  dirs: { name: string; path: string; system: string | null }[];
  files: { name: string; path: string; size: number; systems: string[] }[];
}
import { RpcClient } from './client';
import type { CustomApp, FsListing, SystemApp, Friend, GameSession, HardwareProfile, Hello, Item, LibraryPage, Notification, Parental, Prefs, Profile, RemotePairing, SessionInfo, SteamAccount } from './generated';

type Auth = { pin?: string; parentPin?: string; grant?: string };

export interface Pad { id: string; name: string; bus: 'usb' | 'bluetooth' | 'virtual' | 'outro'; vendor: number; product: number; mapped: boolean; connected: boolean; registered: boolean; nickname: string; player: number | null }
export type PadBinding = { kind: 'button'; code: number } | { kind: 'axis' | 'hat'; code: number; dir: number };
export interface EmuSystem { dir: string; name: string; exts: string[]; core: string; games: number; coreReady: boolean }
export interface EmuStatus { retroarch: boolean; romsDir: string; systems: EmuSystem[] }
export interface BtDevice { mac: string; name: string; gamepad: boolean }
export interface BtStatus { available: boolean; powered: boolean; paired: BtDevice[] }

/** Métodos tipados do Core. Cada um corresponde a uma rota em rpc.rs. */
export function createApi(c: RpcClient) {
  const call = c.call.bind(c);
  return {
    hello: () => call<Hello>('core.hello'),
    // controles físicos, mapeamento (emuladores) e Bluetooth
    pads: () => call<Pad[]>('controllers.list'),
    emuStatus: () => call<EmuStatus>('emu.status'),
    emuFetchCores: (all = false) => call<{ ok: boolean; done: string[]; failed: { core: string; error: string }[] }>('emu.fetch_cores', { all }, 30 * 60 * 1000),
    padSave: (id: string, p: { nickname?: string; player?: number | null }) => call<Pad>('controllers.save', { id, ...p }),
    padDelete: (id: string) => call<{ ok: true }>('controllers.delete', { id }),
    padActions: () => call<{ id: string; label: string }[]>('controllers.actions'),
    padCapture: (id: string, timeoutMs = 10000) => call<{ binding: PadBinding | null }>('controllers.capture', { id, timeoutMs }, timeoutMs + 5000),
    padMap: (id: string) => call<Record<string, PadBinding>>('controllers.get_map', { id }),
    padSetMap: (id: string, map: Record<string, PadBinding>) => call<{ ok: true; retroarch: string[] }>('controllers.set_map', { id, map }),
    btStatus: () => call<BtStatus>('bt.status'),
    btScan: (secs = 10) => call<{ devices: BtDevice[] }>('bt.scan', { secs }, (secs + 10) * 1000),
    btPair: (mac: string, a: Auth = {}) => call<{ ok: true }>('bt.pair', { mac, ...a }, 70000),
    btRemove: (mac: string, a: Auth = {}) => call<{ ok: true }>('bt.remove', { mac, ...a }),
    hardware: () => call<HardwareProfile>('hardware.profile'),
    power: (action: 'off' | 'restart' | 'suspend') => call<{ simulated: boolean }>('system.power', { action }),
    steamOpen: () => call<{ ok: true }>('steam.open'),
    steamStorePage: (id: string) => call<{ ok: true }>('steam.store_page', { id }),
    systemApps: () => call<SystemApp[]>('apps.system'),
    customApps: () => call<CustomApp[]>('apps.registered'),
    addApp: (fileId: string, kind: 'game' | 'app' | undefined, a: Auth) => call<CustomApp>('apps.add', { fileId, kind, ...a }),
    updateApp: (p: { id: string; name?: string | null; kind?: 'game' | 'app'; icon?: string | null; cover?: string | null; hero?: string | null } & Auth) => call<CustomApp>('apps.update', p),
    removeApp: (id: string, a: Auth) => call<{ ok: true }>('apps.remove', { id, ...a }),
    fsList: (path?: string, a: { grant?: string } = {}) => call<FsListing>('fs.list', { path, ...a }),
    romsMedia: () => call<RomMedia>('roms.media'),
    romsMount: (dev: string) => call<{ path: string }>('roms.mount', { dev }),
    romsBrowse: (path: string) => call<RomListing>('roms.browse', { path }),
    romsImport: (items: { path: string; system: string }[]) => call<{ ok: boolean; copied: string[]; failed: { name: string; error: string }[] }>('roms.import', { items }, 60 * 60 * 1000),
    romsUploadLink: () => call<{ url: string; qrSvg: string }>('roms.upload_link'),
    importAvatar: (path: string, a: { grant?: string } = {}) => call<{ avatar: string }>('profiles.avatar_import', { path, ...a }),
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
    guideClosed: () => call('guide.closed'),
    eulaAnswer: (accept: boolean) => call('steam.eula_answer', { accept }),
    notifications: () => call<Notification[]>('notifications.list'),
    clearNotifications: () => call('notifications.clear'),
    pairing: () => call<RemotePairing>('remote.pairing'),
    rotatePairing: () => call<RemotePairing>('remote.rotate'),
    disconnectRemote: (id: string) => call<RemotePairing>('remote.disconnect', { id }),
  };
}

export type Api = ReturnType<typeof createApi>;
