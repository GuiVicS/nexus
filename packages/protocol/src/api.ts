/** Atualização encontrada de um app instalado (Flatpak: o Nexus atualiza e restaura; Snap/apt: só avisa). */
export interface AppUpdate {
  id: string;
  title: string;
  source: string;
  current: string;
  available: string;
  canUpdate: boolean;
}
export interface AppUpdates {
  updates: AppUpdate[];
  /** apps que permitem voltar à versão anterior */
  restorable: string[];
  /** apps sendo atualizados/restaurados agora */
  jobs: string[];
}

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
export interface EmuSystem { dir: string; name: string; exts: string[]; core: string; defaultCore?: string; options?: string[]; games: number; coreReady: boolean }
export interface EmuStatus { retroarch: boolean; romsDir: string; extraDirs?: string[]; systems: EmuSystem[] }
/** B17: apps registrados com tamanho e origem. */
export interface AppDetail { id: string; name: string; kind: 'game' | 'app'; hidden: boolean; category: string | null; sizeBytes: number | null; origin: string; canUninstall: boolean; categories: string[] }
export interface ProgramInfo { kind: string; name: string; needsJava: boolean; javaOk: boolean; hint: string | null }
export interface CecStatus { installed: boolean; installCmd: string; adapters: { port: string; kind: string }[]; wake: boolean; sleep: boolean }
export interface PrivacyStatus { telemetry: boolean; connections: { name: string; what: string; when: string; on: boolean }[]; stores: { name: string; count: number; detail: string }[] }
export interface UsageReport { name: string; kid: boolean; limitMin: number | null; todaySeconds: number; days: { day: string; seconds: number }[]; top: { id: string; title: string; seconds: number; lastPlayed: number | null }[] }
export type BtKind = 'controle' | 'fone' | 'teclado' | 'mouse' | 'celular' | 'outro';
export interface BtDevice { mac: string; name: string; gamepad: boolean; kind?: BtKind; paired?: boolean; connected?: boolean; battery?: number | null }
export interface BtStatus { available: boolean; powered: boolean; paired: BtDevice[] }

export interface NetDevice { device: string; type: 'ethernet' | 'wifi'; state: string; connection: string }
export interface NetStatus {
  available: boolean; error?: string; connectivity?: 'full' | 'limited' | 'portal' | 'none' | 'unknown'; online?: boolean;
  wifiEnabled?: boolean; hasWifi?: boolean; devices?: NetDevice[]; main?: NetDevice | null;
  ip?: { address: string; gateway: string; dns: string[]; mac: string } | null; ipv4Method?: string; signal?: number | null;
}
export interface WifiNet { ssid: string; signal: number; secure: boolean; security: string; active: boolean; known: boolean }
export interface NetTest { connectivity: string; online: boolean; dns: boolean; latencyMs: number | null; elapsedMs: number }
export interface AudioNode { id: number; name: string; node: string; kind: 'hdmi' | 'analogica' | 'bluetooth' | 'usb'; default: boolean; volume: number; muted: boolean }
export interface DriveStatus { available: boolean; connected: boolean; mounted: boolean; folder: string; loggingIn: boolean; cacheBytes: number; cacheGb: number; about?: { total?: number; used?: number; free?: number } | null }
export interface AudioStatus { available: boolean; error?: string; volume: number; muted: boolean; sinks: AudioNode[]; sources: AudioNode[]; mic: { id: number; volume: number; muted: boolean } | null }
export interface DisplayMode { width: number; height: number; refresh: number }
export interface DisplayOutput { name: string; label: string; width: number | null; height: number | null; refresh: number | null; modes: DisplayMode[]; scale: number; transform: string; vrr: boolean | null; hdr: boolean | null }
export interface DisplayStatus { available: boolean; outputs: DisplayOutput[]; margin: number; pending?: number | null; confirmSecs?: number }
export interface LocaleStatus {
  timezone: string; ntp: boolean; synced: boolean; keyboard: string; language: string;
  timezones: { id: string; name: string }[]; keyboards: { id: string; name: string }[];
}

// Fase 3: atualizações (updates.rs), SSH (sshd.rs), Sobre e diagnóstico (about.rs), Recuperação (recovery.rs)
export interface UpdateHistory { ts: number; kind: 'nexus' | 'system'; from: string; to: string; ok: boolean; message: string }
export interface UpdateStatus {
  nexus: { version: string; packaged: boolean; running: string; latest: string | null; hasUpdate: boolean; needsRestart: boolean; revertTo: string | null };
  system: { os: string; kernel: string; upgradable: number; packages: string[]; needsReboot: boolean };
  checkedAt: number | null; checkError: string | null; history: UpdateHistory[]; gameRunning: boolean;
  job: { running: boolean; kind: string; progress: string; startedAt: number; done: { ok: boolean; message: string } | null };
}
export interface SshStatus {
  installed: boolean; active: boolean; enabled: boolean; user: string; host: string; ips: string[]; port: number;
  fingerprint: string | null; installCmd: string; enableCmd: string;
}
export interface AboutStatus {
  model: string; cpu: string; cores: number; arch: string; memoryMb: number; memoryUsedMb: number; gpu: string; tier: string;
  os: string; kernel: string; nexus: string; diskFreeBytes: number; diskTotalBytes: number; tempC: number | null;
  throttle: { now: string[]; past: string[] } | null; cpuPct: number; load: string[]; uptimeSecs: number;
  top: { name: string; cpu: number; memMb: number }[];
}
export interface SupportReport { name: string; size: number; dir: string; url: string; qrSvg: string }
export type RecoveryAction = 'restart_session' | 'repair_shell' | 'desktop_on' | 'desktop_off' | 'restore_defaults' | 'erase_profile' | 'factory_reset';

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
    btPower: (on: boolean) => call<{ ok: true }>('bt.power', { on }, 15000),
    btConnect: (mac: string) => call<{ ok: true }>('bt.connect', { mac }, 30000),
    btDisconnect: (mac: string) => call<{ ok: true }>('bt.disconnect', { mac }, 15000),
    hardware: () => call<HardwareProfile>('hardware.profile'),
    // rede (NetworkManager) e relógio/teclado — ver sysnet.rs e locale.rs
    netStatus: () => call<NetStatus>('net.status', {}, 15000),
    netTest: () => call<NetTest>('net.test', {}, 30000),
    wifiList: (rescan = false) => call<{ networks: WifiNet[] }>('net.wifi_list', { rescan }, 30000),
    wifiConnect: (ssid: string, password: string | undefined, hidden = false, a: Auth = {}) => call<{ ok: true }>('net.wifi_connect', { ssid, password, hidden, ...a }, 60000),
    netForget: (name: string, a: Auth = {}) => call<{ ok: true }>('net.forget', { name, ...a }),
    wifiEnable: (on: boolean, a: Auth = {}) => call<{ ok: true }>('net.wifi_enable', { on, ...a }),
    setIpv4: (p: { connection: string; method: 'auto' | 'manual'; address?: string; gateway?: string; dns?: string[] } & Auth) => call<{ ok: true }>('net.set_ipv4', p, 60000),
    localeStatus: () => call<LocaleStatus>('locale.status'),
    driveStatus: (about = false) => call<DriveStatus>('drive.status', { about }, 30000),
    driveLogin: (a: Auth = {}) => call<{ ok: true; url: string }>('drive.login', { ...a }, 30000),
    driveCancel: () => call<{ ok: true }>('drive.cancel'),
    driveCancelFetch: () => call<{ ok: true }>('drive.cancel_fetch'),
    driveDisconnect: (a: Auth = {}) => call<{ ok: true }>('drive.disconnect', { ...a }, 30000),
    driveMount: (a: Auth = {}) => call<{ ok: true }>('drive.mount', { ...a }, 30000),
    driveCache: (gb: number, a: Auth = {}) => call<{ ok: true; gb: number }>('drive.cache', { gb, ...a }, 30000),
    audioStatus: () => call<AudioStatus>('audio.status'),
    audioSet: (p: { id?: number; source?: boolean; volume?: number; muted?: boolean; default?: number }) => call<AudioStatus>('audio.set', p),
    audioTest: (id?: number) => call<{ ok: true }>('audio.test', id === undefined ? {} : { id }, 10000),
    updateStatus: () => call<UpdateStatus>('update.status'),
    updateCheck: (a: Auth = {}) => call<UpdateStatus>('update.check', a, 90000),
    updateInstall: (p: { kind: 'nexus' | 'system'; version?: string } & Auth) => call<{ started: true }>('update.install', p),
    updateRevert: (a: Auth = {}) => call<{ started: true }>('update.revert', a),
    sshStatus: () => call<SshStatus>('ssh.status', {}, 15000),
    sshSet: (on: boolean, a: Auth = {}) => call<SshStatus>('ssh.set', { on, ...a }, 90000),
    aboutStatus: () => call<AboutStatus>('about.status', {}, 20000),
    aboutReport: (a: Auth = {}) => call<SupportReport>('about.report', a, 60000),
    recoveryStatus: (a: Auth = {}) => call<{ desktopMode: boolean; gameRunning: boolean }>('recovery.status', a),
    recoveryRun: (p: { action: RecoveryAction; id?: string } & Auth) => call<{ ok: true; desktopMode: boolean }>('recovery.run', p, 30000),
    displayStatus: () => call<DisplayStatus>('display.status'),
    displaySet: (p: { output: string; mode?: string; scale?: number; transform?: string; hdr?: boolean } & Auth) => call<{ token: number; secs: number }>('display.set', p, 15000),
    displayConfirm: (token: number) => call<{ ok: boolean }>('display.confirm', { token }),
    displayRevert: (token: number) => call<{ ok: boolean }>('display.revert', { token }),
    displayVrr: (output: string, on: boolean, a: Auth = {}) => call<{ ok: true }>('display.vrr', { output, on, ...a }),
    displayMargin: (margin: number, a: Auth = {}) => call<{ margin: number }>('display.margin', { margin, ...a }),
    localeSet: (p: { timezone?: string; keyboard?: string; ntp?: boolean } & Auth) => call<LocaleStatus>('locale.set', p, 20000),
    power: (action: 'off' | 'restart' | 'suspend') => call<{ simulated: boolean }>('system.power', { action }),
    steamOpen: () => call<{ ok: true }>('steam.open'),
    steamStorePage: (id: string) => call<{ ok: true }>('steam.store_page', { id }),
    systemApps: () => call<SystemApp[]>('apps.system'),
    customApps: () => call<CustomApp[]>('apps.registered'),
    addApp: (fileId: string, kind: 'game' | 'app' | undefined, a: Auth) => call<CustomApp>('apps.add', { fileId, kind, ...a }),
    updateApp: (p: { id: string; name?: string | null; kind?: 'game' | 'app'; icon?: string | null; cover?: string | null; hero?: string | null; hidden?: boolean; category?: string | null } & Auth) => call<CustomApp>('apps.update', p),
    removeApp: (id: string, a: Auth) => call<{ ok: true }>('apps.remove', { id, ...a }),
    fsList: (path?: string, a: { grant?: string } = {}, mode?: 'image' | 'program' | 'folder') => call<FsListing>('fs.list', { path, mode, ...a }),
    romsMedia: () => call<RomMedia>('roms.media'),
    romsMount: (dev: string) => call<{ path: string }>('roms.mount', { dev }),
    romsBrowse: (path: string) => call<RomListing>('roms.browse', { path }),
    romsImport: (items: { path: string; system: string }[]) => call<{ ok: boolean; copied: string[]; failed: { name: string; error: string }[] }>('roms.import', { items }, 60 * 60 * 1000),
    romsUploadLink: () => call<{ url: string; qrSvg: string }>('roms.upload_link'),
    appUpdates: () => call<AppUpdates>('apps.updates'),
    appCheckUpdates: () => call<AppUpdate[]>('apps.check_updates'),
    appUpgrade: (id: string) => call<{ ok: true }>('apps.upgrade', { id }),
    appRestore: (id: string) => call<{ ok: true }>('apps.restore', { id }),
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
    parentalSet: (newPin: string | null, currentPin?: string) => call<Parental & { recoveryCode?: string | null }>('parental.set', { newPin, currentPin }),
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
    // ROM do Google Drive baixa antes de abrir: pode levar minutos (progresso em drive.fetch)
    launch: (id: string) => call<GameSession>('item.launch', { id }, 30 * 60_000),
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
    // Fase 4 (E): CEC, perfis (recuperar PIN, relatório), apps instalados, ROMs, privacidade
    cecStatus: () => call<CecStatus>('cec.status', {}, 30000),
    cecSet: (p: { wake?: boolean; sleep?: boolean }) => call<CecStatus>('cec.set', p, 30000),
    cecTv: (on: boolean) => call<{ ok: true }>('cec.tv', { on }, 30000),
    recoveryStatus2: () => call<{ has: boolean }>('parental.recovery_status'),
    recoveryNew: (a: Auth) => call<{ code: string }>('parental.recovery_new', { ...a }),
    recoverPin: (code: string, newPin: string) => call<{ code: string }>('parental.recover', { code, newPin }),
    usageReport: (profileId: string, a: Auth) => call<UsageReport>('parental.report', { profileId, ...a }),
    appDetails: () => call<AppDetail[]>('apps.details', {}, 60000),
    programInfo: (path: string) => call<ProgramInfo>('programs.info', { path }),
    addProgram: (p: { path: string; name?: string; kind?: 'game' | 'app' } & Auth) => call<CustomApp>('programs.add', p),
    uninstallApp: (id: string, a: Auth) => call<{ ok: true; name: string }>('apps.uninstall', { id, ...a }, 15 * 60 * 1000),
    romDirs: () => call<{ dirs: string[]; status: EmuStatus }>('roms.dirs'),
    romAddDir: (path: string) => call<{ dirs: string[]; status: EmuStatus }>('roms.add_dir', { path }, 60000),
    romRemoveDir: (path: string) => call<{ dirs: string[]; status: EmuStatus }>('roms.remove_dir', { path }, 60000),
    romRescan: () => call<{ dirs: string[]; status: EmuStatus }>('roms.rescan', {}, 60000),
    emuSetCore: (dir: string, core: string) => call<{ dirs: string[]; status: EmuStatus }>('emu.set_core', { dir, core }, 60000),
    privacyStatus: () => call<PrivacyStatus>('privacy.status'),
    privacyClear: (what: 'recent' | 'playtime' | 'cache', a: Auth = {}) => call<{ cleared: number }>('privacy.clear', { what, ...a }),
  };
}

export type Api = ReturnType<typeof createApi>;
