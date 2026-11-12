import { reconcile } from 'solid-js/store';
import { E, RpcError } from '@nexus/protocol';
import type { AppUpdate, Item, SessionInfo } from '@nexus/protocol';
import { api, client } from './api';
import { applyPrefs, askPin, closeAllOv, fail, findItem, fx, isKid, me, ovs, popOv, removeLib, S, setS, setMode, toast, upsertLib, topOv, askConfirm, guarded, hasOv, pushOv, closeOv, askChoice } from './state';
import { sfx } from '@nexus/ui';
import { fmtPlaytime } from './format';
import { loadStatus, startStatus } from './status';

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

/** Atualizações dos apps instalados (aviso, botão Atualizar, Restaurar versão anterior). */
export async function loadAppUpdates() {
  try {
    const u = await api.appUpdates();
    setS('appUpd', reconcile(Object.fromEntries(u.updates.map((x) => [x.id, x]))));
    setS('appRestorable', u.restorable);
    setS('appJobs', reconcile(Object.fromEntries(u.jobs.map((id) => [id, 'update' as const]))));
  } catch {
    /* sem sessão */
  }
}

export async function enterShell(info: SessionInfo) {
  setS('session', info);
  applyPrefs();
  await loadData();
  closeAllOv();
  setS({ tab: 'home', scr: 'shell', dockId: null, bgId: null, libFilter: 'todos', shopCat: 'inicio', emuSys: null });
  void api.emuStatus().then((st) => setS('emuReady', !!st.retroarch)).catch(() => {});
  void loadAppUpdates();
  startStatus();
  void api.notificationSettings().then((s) => setS('notifSilence', s.silenceInGame)).catch(() => {});
  void loadStatus();
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

/** Core fora do ar por mais que isso: troca o aviso "Reconectando…" pela tela de falha com ações. */
export const FAULT_AFTER_MS = 12000;
let faultT: ReturnType<typeof setTimeout> | undefined;
function watchFault() {
  if (faultT !== undefined) return; // o transporte avisa "caiu" a cada tentativa: conta do primeiro aviso
  faultT = setTimeout(() => {
    faultT = undefined;
    if (!S.conn && !hasOv('fault')) {
      setS('faultMsg', '');
      void pushOv('fault');
    }
  }, FAULT_AFTER_MS);
}

export function startEvents() {
  watchFault(); // também cobre o Core que nunca subiu
  client.onStatus((up) => {
    setS('conn', up);
    if (up) {
      clearTimeout(faultT);
      faultT = undefined;
      for (const o of [...ovs()]) if (o.kind === 'fault') closeOv(o.id);
      void resync();
    } else watchFault();
  });
  client.on('library.changed', async (p: { ids: string[]; removed: string[] }) => {
    if (!S.session) return;
    removeLib(p.removed);
    // app do Linux entrou/saiu: a lista de apps que podem voltar versão muda
    if ([...p.ids, ...p.removed].some((id) => id.startsWith('native:') && !S.lib.some((i) => i.id === id))) {
      void api.appUpdates().then((u) => setS('appRestorable', u.restorable)).catch(() => {});
    }
    if (p.ids.length) {
      try {
        upsertLib(await api.items(p.ids));
      } catch { /* sessão mudou no meio */ }
    }
  });
  client.on('game.launching', (p) => {
    setS('launchHint', '');
    setS('game', p.session);
  });
  client.on('drive.fetch', (p: { id: string; done: number; total: number }) => setS('fetch', p.done >= p.total ? null : p));
  client.on('game.waiting', (p: { hint: string }) => setS('launchHint', p.hint));
  client.on('game.started', (p) => {
    setS('game', p.session);
    sfx('launch');
  });
  client.on('game.exited', (p: { itemId: string; playSec: number }) => {
    const title = S.game?.title ?? findItem(p.itemId)?.title ?? 'Jogo';
    setS('game', null);
    setS('launchHint', '');
    for (const o of [...ovs()].reverse()) if (o.kind === 'launch') popOv(undefined, true);
    if (p.itemId.startsWith('drive:')) return; // janela do login do Google Drive: o painel avisa o resultado
    toast(`${title} foi encerrado`, p.playSec >= 60 ? fmtPlaytime(p.playSec) : 'Progresso salvo', 'check');
    if (S.session) void api.items([p.itemId]).then(upsertLib).catch(() => {});
    queueMicrotask(() => requestAnimationFrame(() => fx.f?.ensure()));
  });
  client.on('game.failed', (p: { reason: string }) => {
    setS('fetch', null);
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
  client.on('apps.updates', (list: AppUpdate[]) => {
    setS('appUpd', reconcile(Object.fromEntries(list.map((x) => [x.id, x]))));
    void api.appUpdates().then((u) => setS('appRestorable', u.restorable)).catch(() => {}); // apps adicionados depois do login
  });
  client.on('apps.job', (j: { id: string; op: 'update' | 'restore'; state: string }) => {
    if (j.state === 'running') setS('appJobs', j.id, j.op);
    else setS('appJobs', reconcile(Object.fromEntries(Object.entries(S.appJobs).filter(([k]) => k !== j.id))));
  });
  client.on('notification.added', (n) => {
    if (isKid()) return;
    setS('notifs', (l) => [n, ...l.filter((x) => x.id !== n.id)].slice(0, 30));
    setS('unread', true);
    // "silenciar durante jogos": com jogo aberto só os importantes (alert) aparecem; o resto espera no sino
    if (S.game && S.notifSilence && n.kind !== 'alert') return;
    toast(n.title, n.body, n.kind === 'alert' ? 'info' : n.kind);
  });
  client.on('parental.warning', (p: { minutes: number }) => toast(`Faltam ${p.minutes} minutos`, 'O tempo de jogo de hoje está acabando', 'clock'));
  client.on('parental.limit_reached', () => {
    if (!isKid()) return;
    void import('./actions').then((m) => m.showTimeUp());
  });
  // Instalação da Steam com contrato de licença: quem aceita é o usuário, na tela do Nexus (a Steam fica escondida).
  client.on('steam.eula', (p: { title: string }) => {
    void askConfirm({
      title: 'Contrato de licença',
      sub: `Para instalar ${p.title || 'este jogo'}, é preciso aceitar o contrato de licença (EULA) do jogo. Ao aceitar, você concorda com os termos dele na Steam.`,
      yes: 'Aceitar e instalar',
      no: 'Cancelar',
    }).then((ok) => api.eulaAnswer(ok).catch(() => {}));
  });
  // Saída de emergência: Guia (ou Select+Start) segurado por 3 s com um jogo aberto. O Core já trouxe a Central para a
  // frente; aqui só pergunta. Fechar à força mata o jogo inteiro (progresso não salvo se perde).
  client.on('input.emergency', (p: { title: string }) => {
    if (!S.game || hasOv('confirm')) return;
    void import('./actions').then(async (m) => {
      const ok = await askConfirm({
        title: 'Fechar o jogo agora?',
        sub: `${p.title || 'O jogo'} será encerrado à força e o que não foi salvo se perde. Use só se ele travou.`,
        yes: 'Fechar jogo',
        no: 'Continuar',
        danger: true,
        safe: true,
      });
      if (ok) {
        if (topOv()?.kind === 'guide') popOv(undefined, true);
        setS('frame', null);
        await m.closeGame(true);
      } else if (topOv()?.kind === 'guide') m.closeGuide();
    });
  });
  // Botão físico de energia (preferência "Perguntar"): escolhe entre suspender e desligar; Voltar não faz nada.
  client.on('power.button', () => {
    if (S.scr === 'standby' || hasOv('confirm')) return;
    void import('./actions').then(async (m) => {
      const c = await askChoice({ title: 'Botão de energia', sub: S.game ? `${S.game.title} continua aberto se você suspender.` : 'O que fazer agora?', yes: 'Suspender', no: 'Desligar' });
      if (c === true) await m.powerAction('suspend', false);
      else if (c === false) await m.powerAction('off', false);
    });
  });
  client.on('remote.devices', (p) => {
    if (S.pairing) setS('pairing', 'devices', reconcile(p.devices, { key: 'id' }));
  });
  // Menu rápido (Guia, Select+Start, F1, Super): o Core decide se abre por cima do jogo (com quadro), fecha
  // (voltou ao jogo) ou só alterna a Central no Nexus.
  client.on('input.guide', (p: { frame?: number | null; toggle?: boolean; close?: boolean }) => {
    void import('./actions').then((m) => {
      if (p?.close) {
        if (topOv()?.kind === 'guide') popOv(undefined, true);
        setS('frame', null);
        return;
      }
      if (p?.toggle) {
        if (topOv()?.kind === 'guide') m.closeGuide();
        else m.ACT.guide('');
        return;
      }
      (window as unknown as { __guideT0?: number }).__guideT0 = performance.now();
      setS('frame', p?.frame ?? null);
      if (topOv()?.kind !== 'guide') m.ACT.guide('');
    });
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
