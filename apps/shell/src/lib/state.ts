import { createSignal } from 'solid-js';
import { createStore, reconcile } from 'solid-js/store';
import { RpcError, E } from '@nexus/protocol';
import type { AppUpdate, Friend, GameSession, HardwareProfile, Hello, Item, Notification, Parental, Prefs, Profile, RemotePairing, SessionInfo, SteamAccount } from '@nexus/protocol';
import { configureSfx, sfx } from '@nexus/ui';
import type { Focus } from '@nexus/focus';
import { api } from './api';

export type Screen = 'boot' | 'standby' | 'onboarding' | 'profiles' | 'shell';
export type Tab = 'home' | 'biblioteca' | 'emu' | 'midia' | 'loja';
export type OvKind = 'sheet' | 'hub' | 'search' | 'settings' | 'guide' | 'notif' | 'launch' | 'pin' | 'text' | 'profile' | 'timeup' | 'confirm' | 'appicker' | 'appedit' | 'filepick' | 'padmap' | 'padedit' | 'roms' | 'fault' | 'margin';
export type Mode = 'key' | 'pad' | 'ptr';

export const DEFAULT_PREFS: Prefs = { theme: 'dark', sfx: true, vol: 60, reduce: false, hc: false, subs: false, invy: false, vib: true, pins: [], favs: [] };

interface State {
  conn: boolean;
  /** mensagem da tela de falha (Core fora do ar) */
  faultMsg: string;
  hello: Hello | null;
  hw: HardwareProfile | null;
  scr: Screen;
  tab: Tab;
  profiles: Profile[];
  parental: Parental;
  session: SessionInfo | null;
  lib: Item[];
  game: GameSession | null;
  /** quadro do jogo (timestamp) quando a Central foi aberta por cima dele */
  frame: number | null;
  /** assistente de mapeamento aberto: o controle físico não navega nos menus */
  capturing: boolean;
  notifs: Notification[];
  unread: boolean;
  libFilter: string;
  shopCat: string;
  /** aba Emuladores: console aberto (pasta em ~/ROMs) ou null = lista de consoles */
  emuSys: string | null;
  /** aba Mídia: série aberta (lista de episódios) ou null */
  mediaShow: string | null;
  /** RetroArch instalado (mostra a aba Emuladores mesmo sem jogos, para adicionar) */
  emuReady: boolean;
  /** atualizações dos apps instalados (id do item → atualização) */
  appUpd: Record<string, AppUpdate>;
  /** apps que podem voltar à versão anterior (Flatpak) */
  appRestorable: string[];
  /** app sendo atualizado/restaurado agora → operação */
  appJobs: Record<string, 'update' | 'restore'>;
  setCat: string;
  dockId: string | null;
  bgId: string | null;
  shop: Record<string, Item[]>;
  extra: Record<string, Item>;
  wallet: number | null;
  friends: Friend[];
  pairing: RemotePairing | null;
  steam: SteamAccount | null;
  grant: { token: string; at: number } | null;
  launchHint: string;
  /** ROM do Google Drive baixando antes de abrir */
  fetch: { id: string; done: number; total: number } | null;
}

export const [S, setS] = createStore<State>({
  conn: false,
  faultMsg: '',
  hello: null,
  hw: null,
  scr: 'boot',
  tab: 'home',
  profiles: [],
  parental: { hasPin: false },
  session: null,
  lib: [],
  game: null,
  frame: null,
  capturing: false,
  notifs: [],
  unread: false,
  libFilter: 'todos',
  shopCat: 'inicio',
  emuSys: null,
  mediaShow: null,
  emuReady: false,
  appUpd: {},
  appRestorable: [],
  appJobs: {},
  setCat: 'som',
  dockId: null,
  bgId: null,
  shop: {},
  extra: {},
  wallet: null,
  friends: [],
  pairing: null,
  steam: null,
  grant: null,
  launchHint: '',
  fetch: null,
});

export const prefs = (): Prefs => S.session?.prefs ?? DEFAULT_PREFS;
export const me = (): Profile | null => S.session?.profile ?? null;
export const isKid = () => S.session?.profile.kind === 'kid';

// ---------- foco (instância criada em App) ----------
export const fx: { f: Focus | null } = { f: null };
export const focusNow = () => fx.f?.cur ?? null;

// ---------- modo de entrada ----------
export const [mode, setModeSig] = createSignal<Mode>('key');
export function setMode(m: Mode) {
  if (mode() === m) return;
  setModeSig(m);
  document.body.classList.remove('m-pad', 'm-key', 'm-ptr');
  document.body.classList.add('m-' + m);
}

// ---------- toasts ----------
export interface Toast { id: number; title: string; sub: string; icon: string; out: boolean }
export const [toasts, setToasts] = createSignal<Toast[]>([]);
let toastId = 0;
export function toast(title: string, sub = '', icon = 'info') {
  const t: Toast = { id: ++toastId, title, sub, icon, out: false };
  setToasts((l) => [...l.slice(-3), t]);
  sfx('notify');
  setTimeout(() => setToasts((l) => l.map((x) => (x.id === t.id ? { ...x, out: true } : x))), 3200);
  setTimeout(() => setToasts((l) => l.filter((x) => x.id !== t.id)), 3600);
}

export function fail(e: unknown) {
  if (e instanceof RpcError) {
    if (e.code === E.LOCKED) {
      const secs = (e.data as { secs?: number } | undefined)?.secs;
      toast('Muitas tentativas', secs ? `Aguarde ${secs} s` : 'Tente de novo em instantes', 'lock');
    } else toast('Não foi possível', e.message, 'info');
  } else toast('Algo deu errado', String((e as Error)?.message ?? e), 'info');
  sfx('deny');
}

// ---------- camadas (overlays) ----------
export interface Ov { id: number; kind: OvKind; props: any; ret: HTMLElement | null; resolve: (v: any) => void }
export const [ovs, setOvs] = createSignal<Ov[]>([]);
let ovId = 0;

export function pushOv<T = unknown>(kind: OvKind, props: Record<string, any> = {}): Promise<T | undefined> {
  return new Promise((resolve) => {
    const ov: Ov = { id: ++ovId, kind, props, ret: focusNow(), resolve };
    setOvs((l) => [...l, ov]);
    sfx('open');
    queueMicrotask(() => requestAnimationFrame(() => focusLayer(ov.id)));
  });
}

export function focusLayer(id: number) {
  const el = document.querySelector<HTMLElement>(`#ovs [data-oid="${id}"]`);
  if (!el || !fx.f) return;
  const auto = el.querySelector<HTMLElement>('.f[data-autofocus]');
  if (auto) fx.f.set(auto, { silent: true, snap: true });
  else fx.f.first(el);
}

export function popOv(result?: unknown, quiet = false) {
  const l = ovs();
  const top = l[l.length - 1];
  if (!top) return;
  setOvs(l.slice(0, -1));
  top.resolve(result);
  if (!quiet) sfx('close');
  queueMicrotask(() =>
    requestAnimationFrame(() => {
      const f = fx.f;
      if (!f) return;
      const r = top.ret;
      const root = f.root();
      if (r && r.isConnected && root?.contains(r) && r.getClientRects().length) f.set(r, { silent: true, still: true, snap: true });
      else f.first(root);
    }),
  );
}

export function closeOv(id: number, result?: unknown) {
  const l = ovs();
  if (l[l.length - 1]?.id === id) return popOv(result, true);
  const o = l.find((x) => x.id === id);
  setOvs(l.filter((x) => x.id !== id));
  o?.resolve(result);
}

export function closeAllOv() {
  const l = ovs();
  setOvs([]);
  l.forEach((o) => o.resolve(undefined));
}

export const topOv = () => ovs()[ovs().length - 1] ?? null;
export const hasOv = (k: OvKind) => ovs().some((o) => o.kind === k);

/** Substitui a camada de cima por outra, preservando para onde o foco volta. */
export function replaceTop(kind: OvKind, props: Record<string, any>) {
  const l = ovs();
  const top = l[l.length - 1];
  const ret = top?.ret ?? null;
  if (top) {
    setOvs(l.slice(0, -1));
    top.resolve(undefined);
  }
  const p = pushOv(kind, props);
  const n = ovs();
  if (n.length) n[n.length - 1].ret = ret;
  return p;
}

// ---------- PIN e texto ----------
export const askPin = (o: { title: string; sub?: string; confirm?: boolean }) => pushOv<string | null>('pin', o).then((v) => v ?? null);
export const askText = (o: { title: string; sub?: string; value?: string; max?: number; kind?: 'name' | 'key' | 'secret' | 'ip' }) => pushOv<string | null>('text', o).then((v) => (v === undefined ? null : v));
export const askConfirm = (o: { title: string; sub?: string; yes?: string; no?: string; danger?: boolean }) => pushOv<boolean>('confirm', o).then((v) => !!v);
/** Duas opções de verdade (sim/não) + Voltar: true, false ou null (voltou sem escolher). */
export const askChoice = (o: { title: string; sub?: string; yes: string; no: string; danger?: boolean; countdown?: number }) => pushOv<boolean | null>('confirm', o).then((v) => (v === true || v === false ? v : null));

const GRANT_MS = 9 * 60 * 1000;
export const hasGrant = () => !!S.grant && Date.now() - S.grant.at < GRANT_MS;

/** Executa uma ação que pode exigir o PIN dos responsáveis; pede o PIN se o Core mandar. */
export async function guarded<T>(fn: (auth: { grant?: string }) => Promise<T>, reason = 'Confirme para continuar'): Promise<T | undefined> {
  let auth: { grant?: string } = hasGrant() ? { grant: S.grant!.token } : {};
  for (let tries = 0; tries < 8; tries++) {
    try {
      return await fn(auth);
    } catch (e) {
      if (!(e instanceof RpcError)) return void fail(e);
      const scope = (e.data as { scope?: string } | undefined)?.scope;
      if ((e.code === E.PIN_REQUIRED || e.code === E.PIN_INVALID) && scope === 'parental') {
        if (e.code === E.PIN_INVALID && auth.grant === undefined && tries > 0) {
          const rem = (e.data as { remaining?: number }).remaining;
          toast('PIN incorreto', rem ? `${rem} tentativa(s) restante(s)` : '', 'lock');
        }
        setS('grant', null);
        const pin = await askPin({ title: 'PIN dos responsáveis', sub: reason });
        if (pin === null) return undefined;
        try {
          const g = await api.parentalVerify(pin);
          setS('grant', { token: g.grant, at: Date.now() });
          auth = { grant: g.grant };
        } catch (e2) {
          if (e2 instanceof RpcError && e2.code === E.PIN_INVALID) {
            sfx('deny');
            toast('PIN incorreto', `${(e2.data as { remaining?: number })?.remaining ?? ''} tentativa(s) restante(s)`, 'lock');
            continue;
          }
          return void fail(e2);
        }
        continue;
      }
      return void fail(e);
    }
  }
  return undefined;
}

/** Garante a autorização dos responsáveis (pede o PIN se houver um definido). */
export async function requireParental(reason = 'Confirme para continuar'): Promise<boolean> {
  if (!S.parental.hasPin || hasGrant()) return true;
  for (let i = 0; i < 6; i++) {
    const pin = await askPin({ title: 'PIN dos responsáveis', sub: reason });
    if (pin === null) return false;
    try {
      const g = await api.parentalVerify(pin);
      setS('grant', { token: g.grant, at: Date.now() });
      return true;
    } catch (e) {
      if (e instanceof RpcError && e.code === E.PIN_INVALID) {
        sfx('deny');
        toast('PIN incorreto', `${(e.data as { remaining?: number })?.remaining ?? ''} tentativa(s) restante(s)`, 'lock');
        continue;
      }
      fail(e);
      return false;
    }
  }
  return false;
}

// ---------- itens ----------
export const isInstalled = (it: Item) => it.state === 'installed' || it.state === 'updating' || it.state === 'running';

export function findItem(id: string | null | undefined): Item | undefined {
  if (!id) return undefined;
  const l = S.lib.find((i) => i.id === id);
  if (l) return l;
  for (const k in S.shop) {
    const s = S.shop[k].find((i) => i.id === id);
    if (s) return s;
  }
  return S.extra[id];
}

export function upsertLib(items: Item[]) {
  for (const it of items) {
    const i = S.lib.findIndex((x) => x.id === it.id);
    if (i >= 0) setS('lib', i, reconcile(it));
    else setS('lib', S.lib.length, it);
    for (const k in S.shop) {
      const j = S.shop[k].findIndex((x) => x.id === it.id);
      if (j >= 0) setS('shop', k, j, reconcile(it));
    }
    if (S.extra[it.id]) setS('extra', it.id, reconcile(it));
  }
}

/** Atualiza um item onde ele já estiver (biblioteca, vitrines da loja, extras) sem incluí-lo na biblioteca. */
export function patchItem(it: Item) {
  const i = S.lib.findIndex((x) => x.id === it.id);
  if (i >= 0) setS('lib', i, reconcile(it));
  for (const k in S.shop) {
    const j = S.shop[k].findIndex((x) => x.id === it.id);
    if (j >= 0) setS('shop', k, j, reconcile(it));
  }
  if (S.extra[it.id]) setS('extra', it.id, reconcile(it));
}

export function removeLib(ids: string[]) {
  if (ids.length) setS('lib', (l) => l.filter((i) => !ids.includes(i.id)));
}

// ---------- preferências ----------
const mq = typeof matchMedia !== 'undefined' ? matchMedia('(prefers-color-scheme: light)') : null;

/** Desenho por software (llvmpipe/SwiftShader: VM sem 3D, driver faltando): efeitos mínimos, senão tudo trava. */
let soft: boolean | undefined;
export function softwareGL(): boolean {
  if (soft !== undefined) return soft;
  soft = false;
  try {
    const gl = document.createElement('canvas').getContext('webgl');
    if (!gl) return (soft = true);
    const ext = gl.getExtension('WEBGL_debug_renderer_info');
    const r = String(gl.getParameter(ext ? ext.UNMASKED_RENDERER_WEBGL : gl.RENDERER) ?? '');
    soft = /llvmpipe|softpipe|swiftshader|software/i.test(r);
    gl.getExtension('WEBGL_lose_context')?.loseContext();
  } catch { /* sem WebGL */ }
  return soft;
}

/** Nível de efeitos de fato aplicado: o do aparelho (Core), rebaixado se a GPU não estiver em uso. */
export function effectsLevel(): 'full' | 'reduced' | 'minimal' {
  if (softwareGL()) return 'minimal';
  const e = S.hw?.effects;
  return e === 'minimal' || e === 'reduced' ? e : 'full';
}

export function applyPrefs() {
  const p = prefs();
  const t = p.theme === 'auto' ? (mq?.matches ? 'light' : 'dark') : p.theme;
  document.documentElement.dataset.theme = t;
  try {
    localStorage.setItem('nexus-theme', JSON.stringify({ theme: p.theme }));
  } catch { /* sem armazenamento */ }
  document.querySelector('meta[name=theme-color]')?.setAttribute('content', t === 'light' ? '#f3f7f6' : '#0b1012');
  document.body.classList.toggle('reduce', p.reduce || effectsLevel() === 'minimal');
  document.body.classList.toggle('hc', p.hc);
  document.body.classList.toggle('kid', isKid());
  document.body.classList.toggle('fx-reduced', effectsLevel() !== 'full');
  configureSfx({ enabled: p.sfx, volume: p.vol });
}
mq?.addEventListener?.('change', () => {
  if (prefs().theme === 'auto') applyPrefs();
});

export function setPref(patch: Partial<Prefs>, quiet = false) {
  if (!S.session) return;
  for (const k of Object.keys(patch) as (keyof Prefs)[]) setS('session', 'prefs', k, patch[k] as never);
  applyPrefs();
  if (!quiet) void api.setPrefs(patch).catch(fail);
}

// ---------- fundo dinâmico ----------
export const [bgFront, setBgFront] = createSignal<'A' | 'B'>('A');
export function setBg(it: Item | undefined) {
  if (!it || S.bgId === it.id) return;
  setS('bgId', it.id);
  setBgFront((f) => (f === 'A' ? 'B' : 'A'));
}

export { sfx };
