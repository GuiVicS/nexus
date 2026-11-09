import { reconcile } from 'solid-js/store';
import { E, RpcError } from '@nexus/protocol';
import type { Item, SessionInfo } from '@nexus/protocol';
import { api, client } from './api';
import { applyPrefs, askPin, closeAllOv, fail, findItem, fx, isKid, me, ovs, popOv, removeLib, S, setS, setMode, toast, upsertLib, topOv, askConfirm, guarded } from './state';
import { sfx } from '@nexus/ui';
import { fmtPlaytime } from './format';

export async function loadProfiles() {
  const [p, par] = await Promise.all([api.profiles(), api.parental()]);
  setS('profiles', reconcile(p, { key: 'id' }));
  setS('parental', par);
}

export async function loadData() {
  const [page, notifs] = await Promise.all([api.library(), api.notifications()]);
  setS('lib', reconcile(page.items, { key: 'id' }));
  setS('notifs', notifs);
  setS('unread', notifs.length > 0);
  api.friends().then((f) => setS('friends', f)).catch(() => setS('friends', []));
  api.wallet().then((w) => setS('wallet', w ? w.balance : null)).catch(() => {});
}

export async function enterShell(info: SessionInfo) {
  setS('session', info);
  applyPrefs();
  await loadData();
  closeAllOv();
  setS({ tab: 'home', scr: 'shell', dockId: null, bgId: null, libFilter: 'todos', shopCat: 'jogos' });
  sfx('profile');
  queueMicrotask(() => requestAnimationFrame(() => document.dispatchEvent(new CustomEvent('nexus:focus-home'))));
  const g = new Date().getHours();
  toast(`${g < 12 ? 'Bom dia' : g < 18 ? 'Boa tarde' : 'Boa noite'}, ${info.profile.name}`, info.remainingMin !== null ? `Hoje você ainda pode jogar ${info.remainingMin} min` : '', 'user');
}

/** Seleciona um perfil, pedindo PIN do perfil e/ou dos responsáveis conforme o Core exigir. */
export async function enterProfile(id: string) {
  let auth: Record<string, string> = {};
  const target = S.profiles.find((p) => p.id === id);
  for (let i = 0; i < 8; i++) {
    try {
      const info = await api.selectProfile({ id, ...auth });
      await enterShell(info);
      return;
    } catch (e) {
      if (!(e instanceof RpcError)) return fail(e);
      const scope = (e.data as { scope?: string } | undefined)?.scope;
      if (e.code === E.PIN_REQUIRED || e.code === E.PIN_INVALID) {
        if (e.code === E.PIN_INVALID) {
          sfx('deny');
          toast('PIN incorreto', `${(e.data as { remaining?: number })?.remaining ?? ''} tentativa(s) restante(s)`, 'lock');
        }
        const key = scope === 'parental' ? 'parentPin' : 'pin';
        const pin = await askPin({ title: scope === 'parental' ? 'PIN dos responsáveis' : `PIN de ${target?.name ?? 'perfil'}`, sub: scope === 'parental' ? 'Necessário para sair de um perfil infantil' : 'Digite o PIN para entrar' });
        if (pin === null) return;
        auth = { ...auth, [key]: pin };
        continue;
      }
      return fail(e);
    }
  }
}

export async function leaveProfile(): Promise<boolean> {
  if (S.game) {
    toast('Feche o jogo primeiro', S.game.title, 'info');
    return false;
  }
  const done = await guarded((a) => api.logout(a), 'Necessário para sair de um perfil infantil');
  if (!done) return false;
  setS('session', null);
  closeAllOv();
  await loadProfiles();
  setS({ scr: 'profiles', lib: [] });
  applyPrefs();
  return true;
}

export function startEvents() {
  client.onStatus((up) => {
    setS('conn', up);
    if (up) void resync();
  });
  client.on('library.changed', async (p: { ids: string[]; removed: string[] }) => {
    if (!S.session) return;
    removeLib(p.removed);
    if (p.ids.length) {
      try {
        upsertLib(await api.items(p.ids));
      } catch { /* sessão mudou no meio */ }
    }
  });
  client.on('game.launching', (p) => setS('game', p.session));
  client.on('game.started', (p) => {
    setS('game', p.session);
    sfx('launch');
  });
  client.on('game.exited', (p: { itemId: string; playSec: number }) => {
    const title = S.game?.title ?? findItem(p.itemId)?.title ?? 'Jogo';
    setS('game', null);
    for (const o of [...ovs()].reverse()) if (o.kind === 'launch') popOv(undefined, true);
    toast(`${title} foi encerrado`, p.playSec >= 60 ? fmtPlaytime(p.playSec) : 'Progresso salvo', 'check');
    if (S.session) void api.items([p.itemId]).then(upsertLib).catch(() => {});
    queueMicrotask(() => requestAnimationFrame(() => fx.f?.ensure()));
  });
  client.on('game.failed', (p: { reason: string }) => {
    for (const o of [...ovs()].reverse()) if (o.kind === 'launch') popOv(undefined, true);
    toast('Não foi possível abrir', p.reason, 'info');
    sfx('deny');
  });
  client.on('session.changed', (info: SessionInfo | null) => {
    if (info && S.session && info.profile.id === S.session.profile.id) {
      setS('session', info);
      applyPrefs();
    }
  });
  client.on('profiles.changed', () => void loadProfiles().catch(() => {}));
  client.on('notification.added', (n) => {
    if (isKid()) return;
    setS('notifs', (l) => [n, ...l].slice(0, 30));
    setS('unread', true);
    toast(n.title, n.body, n.kind === 'alert' ? 'info' : n.kind);
  });
  client.on('parental.warning', (p: { minutes: number }) => toast(`Faltam ${p.minutes} minutos`, 'O tempo de jogo de hoje está acabando', 'clock'));
  client.on('parental.limit_reached', () => {
    if (!isKid()) return;
    void import('./actions').then((m) => m.showTimeUp());
  });
  client.on('remote.devices', (p) => {
    if (S.pairing) setS('pairing', 'devices', reconcile(p.devices, { key: 'id' }));
  });
  client.on('input.guide', () => {
    void import('./actions').then((m) => m.ACT.guide(''));
  });
}

/** Depois de reconectar ao Core (ele pode ter reiniciado): confere sessão e jogo. */
async function resync() {
  try {
    const [hello, hw] = await Promise.all([api.hello(), api.hardware()]);
    setS({ hello, hw });
    applyPrefs();
    const [info, game] = await Promise.all([api.session(), api.game()]);
    setS('game', game);
    if (S.session && !info) {
      setS('session', null);
      closeAllOv();
      await loadProfiles();
      setS('scr', 'profiles');
      toast('Sessão encerrada', 'O Core foi reiniciado', 'info');
    } else if (S.session && info) await loadData();
  } catch { /* próxima reconexão tenta de novo */ }
}

export async function bootstrap(): Promise<'profiles' | 'onboarding' | 'shell'> {
  await client.ready();
  const [hello, hw] = await Promise.all([api.hello(), api.hardware()]);
  setS({ hello, hw });
  await loadProfiles();
  const [info, game] = await Promise.all([api.session(), api.game()]);
  setS('game', game);
  applyPrefs();
  if (info) {
    await enterShell(info);
    return 'shell';
  }
  return S.profiles.length ? 'profiles' : 'onboarding';
}

export function requireNoGame(): boolean {
  if (S.game) {
    toast('Há um jogo em execução', S.game.title, 'info');
    return false;
  }
  return true;
}

export { me, topOv, setMode, askConfirm };
export type { Item };
