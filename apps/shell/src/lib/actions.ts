import { createSignal } from 'solid-js';
import { RpcError, E } from '@nexus/protocol';
import type { Item } from '@nexus/protocol';
import { audioInit, sfx } from '@nexus/ui';
import { api, IS_ELECTRON, recover } from './api';
import { askConfirm, closeAllOv, fail, findItem, focusNow, fx, guarded, hasOv, isInstalled, isKid, me, ovs, popOv, prefs, pushOv, replaceTop, requireParental, S, setPref, setS, toast, topOv, upsertLib, patchItem, setMode } from './state';
import { enterProfile, leaveProfile, loadProfiles } from './session';
import { effectivePrice, priceNum } from './format';
import { removeParentalPin, setParentalPin } from './flows';

export const TABS: [string, string][] = [['home', 'Início'], ['biblioteca', 'Biblioteca'], ['emu', 'Emuladores'], ['midia', 'Mídia'], ['loja', 'Loja']];
/** Aba Emuladores só aparece com jogos de console (ROMs) ou com o RetroArch instalado. */
const hasEmu = () => S.emuReady || S.lib.some((i) => i.tags.includes('emu'));
const baseTabs = (): [string, string][] => TABS.filter(([k]) => k !== 'emu' || hasEmu());
export const kidTabs = (): [string, string][] => baseTabs().filter(([k]) => k !== 'loja');
/** Sem Steam possível (Raspberry Pi/ARM sem Steam) não há loja: a aba some. */
const noStore = () => !!S.hw && S.hw.architecture !== 'x86_64' && !S.steam?.installed;
export const tabsNow = () => (isKid() ? kidTabs() : noStore() ? baseTabs().filter(([k]) => k !== 'loja') : baseTabs());

export const [hubTab, setHubTab] = createSignal<string>('');

export const cycHandlers = new Map<string, (d: 1 | -1) => void>();
export type TextEvent = { kind: 'char' | 'del' | 'clear' | 'enter' | 'shift' | 'reveal'; ch?: string; screen?: boolean };
/** Quem está digitando (busca, texto, PIN). `enter` = a tecla Enter envia (em vez de ativar o foco). */
export const textSink: { cur: null | ((e: TextEvent) => void); enter: boolean } = { cur: null, enter: false };

function focusSoon(fn: () => void) {
  queueMicrotask(() => requestAnimationFrame(fn));
}

// ---------- navegação ----------
export function tab(k: string, o: { focus?: boolean } = {}) {
  if (k === 'loja' && (isKid() || noStore())) return;
  setS('tab', k as never);
  if (o.focus) focusSoon(() => fx.f?.set(firstFocus(k), { silent: true, snap: true }));
}

export function firstFocus(k: string): HTMLElement | null {
  const q = (s: string) => document.querySelector<HTMLElement>(s);
  if (k === 'home') return q(`#view .tl[data-k="${S.dockId}"]`) || q('#view .drow .tl') || q('.tab.on');
  return q('#view .sc .sbn') || q('#view .sc .tl') || q('#view .sc .sysc') || q('#view .sc .cp') || q('#view .ri.on') || q('.tab.on');
}

export function tabStep(d: 1 | -1) {
  if (S.scr !== 'shell' || ovs().length) return;
  const t = tabsNow();
  const i = t.findIndex((x) => x[0] === S.tab);
  const n = t[(i + d + t.length) % t.length][0];
  sfx('tab');
  tab(n);
  focusSoon(() => fx.f?.set(document.querySelector<HTMLElement>(`.tab[data-tab="${n}"]`), { silent: true }));
}

export function goHome() {
  closeAllOv();
  setS('dockId', null);
  tab('home', { focus: true });
}

export function back() {
  const top = topOv();
  const cur = focusNow();
  if (top) {
    if (top.kind === 'launch') return goHome();
    if (top.kind === 'timeup' || top.kind === 'fault') return sfx('bump');
    if (top.kind === 'settings' && cur?.closest('.setpane')) {
      sfx('back');
      return fx.f?.set(document.querySelector<HTMLElement>('#ovs .setrail .ri.on'), { silent: true });
    }
    if (top.kind === 'profile' && top.props.first) return sfx('bump');
    if (top.kind === 'guide') return closeGuide();
    return popOv(top.kind === 'pin' || top.kind === 'text' || top.kind === 'confirm' ? null : undefined);
  }
  if (S.scr === 'onboarding') return void document.dispatchEvent(new CustomEvent('nexus:onb', { detail: 'back' }));
  if (S.scr !== 'shell') return;
  if (S.tab === 'home') {
    if (cur?.closest('#acts')) {
      sfx('back');
      fx.f?.set(firstFocus('home'), { silent: true });
    }
    return;
  }
  sfx('back');
  if (S.tab === 'emu' && S.emuSys) {
    const d = S.emuSys;
    setS('emuSys', null);
    return focusSoon(() => fx.f?.set(document.querySelector<HTMLElement>(`#view .sysc[data-k="sys-${d}"]`) ?? firstFocus('emu'), { silent: true, snap: true }));
  }
  if (cur?.closest('.sc') && document.querySelector('#view .rail')) return fx.f?.set(document.querySelector<HTMLElement>('#view .rail .ri.on'), { silent: true });
  tab('home', { focus: true });
}

// ---------- itens ----------
export function verbOf(it: Item): string {
  if (S.game?.itemId === it.id) return 'Retomar';
  if (it.state === 'installing') return 'Aguarde';
  if (it.state === 'not_installed') return it.caps.install ? (priceNum(effectivePrice(it)) > 0 ? 'Comprar' : 'Instalar') : 'Ver';
  if (it.state === 'error') return 'Ver';
  if (it.kind === 'app') return 'Abrir';
  return it.progress ? 'Continuar' : 'Jogar';
}

export async function openHub(id: string, tk?: string) {
  let it = findItem(id);
  if (!it) {
    try {
      it = await api.item(id);
      setS('extra', id, it);
    } catch (e) {
      return fail(e);
    }
  } else if (!it.description && it.provider === 'steam') {
    // item da vitrine da loja: descrição, gêneros e estúdio chegam depois, sem atrasar a abertura
    void api.item(id).then(patchItem).catch(() => {});
  }
  const top = topOv();
  setHubTab(tk ?? '');
  if (top?.kind === 'hub') {
    top.props = { ...top.props, id };
    setOvsRefresh();
    return;
  }
  if (top?.kind === 'sheet') await replaceTop('hub', { id });
  else void pushOv('hub', { id });
}

function setOvsRefresh() {
  document.dispatchEvent(new CustomEvent('nexus:hub-refresh'));
}

export async function launch(id: string, resume = false) {
  const it = findItem(id);
  if (S.game) {
    if (S.game.itemId === id) {
      if (!hasOv('launch')) void pushOv('launch', { id });
      void api.resumeGame().catch(() => {});
      return;
    }
    return void toast('Já há algo em execução', `${S.game.title} — feche antes de abrir outro`, 'info');
  }
  if (it && !isInstalled(it)) return void openHub(id);
  try {
    const gs = await api.launch(id);
    setS('game', gs);
    sfx('launch');
    for (const o of ovs()) if (o.kind === 'sheet' || o.kind === 'hub') popOv(undefined, true);
    void pushOv('launch', { id });
  } catch (e) {
    if (e instanceof RpcError && e.code === E.LIMIT) return void showTimeUp();
    fail(e);
  }
  void resume;
}

export async function resumeGame() {
  if (!S.game) return;
  const top = topOv();
  if (top?.kind === 'guide') popOv(undefined, true);
  if (!hasOv('launch')) void pushOv('launch', { id: S.game.itemId });
  await api.resumeGame().catch(() => {});
}

export async function closeGame(force = false) {
  try {
    await api.closeGame(force);
  } catch (e) {
    fail(e);
  }
}

export async function install(id: string) {
  try {
    await api.install(id);
    toast(id.startsWith('steam:') ? 'Confirme na Steam' : 'Download iniciado', id.startsWith('steam:') ? `${findItem(id)?.title ?? 'O jogo'}: confirme a instalação na janela da Steam` : (findItem(id)?.title ?? ''), 'download');
  } catch (e) {
    fail(e);
  }
}

export function showTimeUp() {
  if (hasOv('timeup')) return;
  closeAllOv();
  void pushOv('timeup', {});
}

// ---------- ajustes e cadastros ----------
export function openSettings(cat?: string) {
  if (cat) setS('setCat', cat);
  if (!hasOv('settings')) void pushOv('settings', {});
}

export function openSearch() {
  if (S.scr !== 'shell' || hasOv('search')) return;
  void pushOv('search', {});
}

export async function toggleGuide() {
  if (S.scr !== 'shell') return;
  if (topOv()?.kind === 'guide') return closeGuide();
  void pushOv('guide', {});
}

/** Fecha a Central; se ela foi aberta por cima de um jogo, volta direto para o jogo. */
export function closeGuide() {
  popOv();
  if (S.frame && S.game) {
    setS('frame', null);
    void api.resumeGame().catch(() => {});
  }
  setS('frame', null);
}

export function toggleNotif() {
  if (S.scr !== 'shell') return;
  if (topOv()?.kind === 'notif') return popOv();
  setS('unread', false);
  void pushOv('notif', {});
}

export function editProfile(id: string | null, opts: { first?: boolean } = {}) {
  return pushOv('profile', { id, ...opts });
}

export async function powerAction(a: 'off' | 'restart') {
  const ok = await askConfirm({ title: a === 'off' ? 'Desligar o console?' : 'Reiniciar o console?', sub: S.game ? `${S.game.title} será fechado.` : 'Salve o que estiver fazendo.', yes: a === 'off' ? 'Desligar' : 'Reiniciar', danger: true });
  if (!ok) return;
  try {
    const r = await api.power(a);
    if (r.simulated) {
      closeAllOv();
      setS({ session: null, scr: a === 'off' ? 'standby' : 'boot' });
      void loadProfiles();
    }
  } catch (e) {
    fail(e);
  }
}

export function toggleFS() {
  try {
    if (document.fullscreenElement) void document.exitFullscreen();
    else void document.documentElement.requestFullscreen?.();
  } catch { /* sem tela cheia */ }
}

export function cycle(k: string, d: 1 | -1) {
  if (k === 'theme') {
    const order = ['dark', 'light', 'auto'];
    const i = order.indexOf(prefs().theme);
    setPref({ theme: order[(i + d + order.length) % order.length] });
    sfx('tab');
    return;
  }
  cycHandlers.get(k)?.(d);
}

export function slide(delta: number) {
  setPref({ vol: Math.max(0, Math.min(100, prefs().vol + delta)) });
  sfx('tick');
}

export function adjust(el: HTMLElement, dir: 1 | -1): boolean {
  if (el.dataset.slider) {
    if (el.dataset.slider === 'vol') slide(dir * 5);
    else cycHandlers.get(el.dataset.slider)?.(dir);
    return true;
  }
  if (el.dataset.cyc) {
    cycle(el.dataset.cyc, dir);
    return true;
  }
  return false;
}

export const THEME_LABEL: Record<string, string> = { dark: 'Escuro', light: 'Claro', auto: 'Automático' };

// ---------- tabela de ações (data-a="nome:arg") ----------
type Fn = (arg: string) => void | Promise<void>;

export const ACT: Record<string, Fn> = {
  noop() {},
  boot() {
    setS('scr', 'boot');
    sfx('boot');
  },
  prof(id) {
    sfx('select');
    void enterProfile(id);
  },
  profadd() {
    void requireParental('Necessário para criar perfis').then((ok) => {
      if (ok) void editProfile(null);
    });
  },
  profedit(id) {
    void editProfile(id);
  },
  tabgo(k) {
    if (S.tab !== k) tab(k);
    focusSoon(() => {
      const f = firstFocus(k);
      if (f) fx.f?.set(f, { silent: true });
    });
  },
  tab(k) {
    tab(k, { focus: true });
  },
  back() {
    back();
  },
  goback() {
    back();
  },
  guide() {
    void toggleGuide();
  },
  search() {
    openSearch();
  },
  notif() {
    toggleNotif();
  },
  settings(cat) {
    openSettings(cat || undefined);
  },
  toast(m) {
    toast(m, '', 'info');
  },
  appadd() {
    void pushOv('appicker', {});
  },
  appedit(id) {
    void pushOv('appedit', { id });
  },
  pickapp(a) {
    document.dispatchEvent(new CustomEvent('nexus:pickapp', { detail: a }));
  },
  ae(a) {
    document.dispatchEvent(new CustomEvent('nexus:ae', { detail: a }));
  },
  rs(a) {
    document.dispatchEvent(new CustomEvent('nexus:rs', { detail: a }));
  },
  fp(a) {
    document.dispatchEvent(new CustomEvent('nexus:fp', { detail: a }));
  },
  pads(a) {
    document.dispatchEvent(new CustomEvent('nexus:pads', { detail: a }));
  },
  net(a) {
    document.dispatchEvent(new CustomEvent('nexus:net', { detail: a }));
  },
  loc(a) {
    document.dispatchEvent(new CustomEvent('nexus:loc', { detail: a }));
  },
  bt(a) {
    document.dispatchEvent(new CustomEvent('nexus:bt', { detail: a }));
  },
  audio(a) {
    document.dispatchEvent(new CustomEvent('nexus:audio', { detail: a }));
  },
  emufetch(a) {
    document.dispatchEvent(new CustomEvent('nexus:emufetch', { detail: a }));
  },
  padedit(a) {
    document.dispatchEvent(new CustomEvent('nexus:padedit', { detail: a }));
  },
  padmap(a) {
    document.dispatchEvent(new CustomEvent('nexus:padmap', { detail: a }));
  },
  async steamopen() {
    try {
      await api.steamOpen();
      toast('Abrindo a Steam no modo TV…', S.steam?.persona ? 'Use o botão Guia para voltar ao Nexus' : 'Escaneie o QR code com o app Steam no celular', 'download');
    } catch (e) {
      fail(e);
    }
  },
  async steamstore(id) {
    try {
      await api.steamStorePage(id);
      toast('Abrindo a loja da Steam…', 'Use o botão Guia para voltar ao Nexus', 'bag');
    } catch (e) {
      fail(e);
    }
  },
  clearnotifs() {
    void api.clearNotifications().then(() => {
      setS('notifs', []);
      popOv();
    });
  },
  parentalpin() {
    void setParentalPin();
  },
  parentalrm() {
    void removeParentalPin();
  },
  allowsheet(pid) {
    const top = topOv();
    if (top?.kind === 'sheet') void replaceTop('sheet', { type: 'allow', profileId: pid });
    else void pushOv('sheet', { type: 'allow', profileId: pid });
  },
  allowkid() {},
  allowtoggle(id) {
    document.dispatchEvent(new CustomEvent('nexus:allow-toggle', { detail: id }));
  },
  async rrotate() {
    const ok = await askConfirm({ title: 'Gerar um novo código?', sub: 'Os aparelhos conectados serão desconectados e o QR atual deixa de funcionar.', yes: 'Gerar novo código' });
    if (!ok) return;
    try {
      setS('pairing', await api.rotatePairing());
      toast('Novo código gerado', '', 'refresh');
    } catch (e) {
      fail(e);
    }
  },
  async rdisc(id) {
    try {
      setS('pairing', await api.disconnectRemote(id));
    } catch (e) {
      fail(e);
    }
  },
  yes() {
    popOv(true);
  },
  no() {
    popOv(false);
  },
  timeup(a) {
    document.dispatchEvent(new CustomEvent('nexus:timeup', { detail: a }));
  },
  pe(a) {
    document.dispatchEvent(new CustomEvent('nexus:pe', { detail: a }));
  },
  onb(a) {
    document.dispatchEvent(new CustomEvent('nexus:onb', { detail: a }));
  },
  remote() {
    if (topOv()?.kind === 'guide') popOv(undefined, true);
    openSettings('remote');
  },
  nt(a) {
    popOv(undefined, true);
    const i = a.indexOf(':');
    const n = i < 0 ? a : a.slice(0, i);
    ACT[n]?.(i < 0 ? '' : a.slice(i + 1));
  },
  open(id) {
    void openHub(id);
  },
  hub(a) {
    const i = a.lastIndexOf(':');
    const maybeTab = a.slice(i + 1);
    if (i > 0 && ['atv', 'ach', 'ami', 'sobre'].includes(maybeTab)) void openHub(a.slice(0, i), maybeTab);
    else void openHub(a);
  },
  hubtab(t) {
    setHubTab(t);
  },
  launch(id) {
    void launch(id);
  },
  app(id) {
    const it = findItem(id);
    if (it && isInstalled(it)) void launch(id);
    else void openHub(id);
  },
  resume() {
    void resumeGame();
  },
  emusys(dir) {
    sfx('select');
    setS('emuSys', dir);
    focusSoon(() => fx.f?.set(firstFocus('emu'), { silent: true, snap: true }));
  },
  romsadd() {
    void pushOv('roms', {});
  },
  exitgame() {
    // "Ir ao Início": o jogo continua rodando em segundo plano (como num console); "Retomar" no topo do Início
    setS('frame', null);
    void api.guideClosed().catch(() => {});
    goHome();
  },
  home() {
    sfx('back');
    goHome();
  },
  closegame() {
    void closeGame();
    if (topOv()?.kind === 'guide') popOv(undefined, true);
  },
  forceclose() {
    void closeGame(true);
  },
  opts(id) {
    const top = topOv();
    if (top?.kind === 'sheet') return;
    void pushOv('sheet', { type: 'opts', id });
  },
  async buy(id) {
    // item vindo da busca na loja pode não estar em memória: pede ao Core (antes o botão não fazia nada)
    let it = findItem(id);
    if (!it) {
      try {
        it = await api.item(id);
        setS('extra', id, it);
      } catch (e) {
        return void fail(e);
      }
    }
    if (priceNum(effectivePrice(it)) > 0) {
      const top = topOv();
      if (top?.kind === 'sheet') void replaceTop('sheet', { type: 'buy', id });
      else void pushOv('sheet', { type: 'buy', id });
    } else {
      if (topOv()?.kind === 'sheet') popOv(undefined, true);
      void install(id);
    }
  },
  async confirm(id) {
    popOv(undefined, true);
    await install(id);
  },
  async cancel(id) {
    try {
      await api.cancel(id);
      toast('Download cancelado', findItem(id)?.title ?? '', 'x');
    } catch (e) {
      fail(e);
    }
  },
  async appupgrade(id) {
    const u = S.appUpd[id];
    if (topOv()?.kind === 'sheet') popOv(undefined, true);
    if (!u?.canUpdate) return void toast('Atualização pelo sistema', u ? `${u.title} vem do ${u.source}: o sistema instala esta atualização` : '', 'info');
    try {
      await api.appUpgrade(id);
      setS('appJobs', id, 'update');
      toast('Atualizando', `${u.title}${u.available ? ` · versão ${u.available}` : ''}`, 'download');
    } catch (e) {
      fail(e);
    }
  },
  async apprestore(id) {
    const it = findItem(id);
    popOv(undefined, true);
    if (!(await askConfirm({ title: 'Restaurar versão anterior?', sub: `${it?.title ?? 'O app'} volta para a versão de antes da última atualização. Dá para atualizar de novo quando quiser.`, yes: 'Restaurar' }))) return;
    try {
      await api.appRestore(id);
      setS('appJobs', id, 'restore');
      toast('Restaurando versão anterior', it?.title ?? '', 'refresh');
    } catch (e) {
      fail(e);
    }
  },
  async appcheck() {
    if (topOv()?.kind === 'sheet') popOv(undefined, true);
    toast('Procurando atualizações…', 'Isso pode levar alguns segundos', 'refresh');
    try {
      const l = await api.appCheckUpdates();
      toast(l.length ? `${l.length} ${l.length === 1 ? 'atualização encontrada' : 'atualizações encontradas'}` : 'Tudo atualizado', l.map((u) => u.title).join(', '), l.length ? 'download' : 'check');
    } catch (e) {
      fail(e);
    }
  },
  async uninstall(id) {
    const it = findItem(id);
    popOv(undefined, true);
    if (!(await askConfirm({ title: `Desinstalar ${it?.title ?? 'item'}?`, sub: 'Você poderá instalar de novo depois.', yes: 'Desinstalar', danger: true }))) return;
    try {
      await api.uninstall(id);
      toast('Desinstalado', it?.title ?? '', 'trash');
      void api.items([id]).then(upsertLib).catch(() => {});
    } catch (e) {
      fail(e);
    }
  },
  fav(id) {
    const f = prefs().favs.includes(id);
    setPref({ favs: f ? prefs().favs.filter((x) => x !== id) : [...prefs().favs, id] });
    sfx(f ? 'off' : 'on');
    toast(f ? 'Removido dos favoritos' : 'Adicionado aos favoritos', findItem(id)?.title ?? '', 'heart');
    popOv(undefined, true);
  },
  pin(id) {
    const f = prefs().pins.includes(id);
    setPref({ pins: f ? prefs().pins.filter((x) => x !== id) : [...prefs().pins, id] });
    sfx(f ? 'off' : 'on');
    toast(f ? 'Removido do início' : 'Fixado no início', findItem(id)?.title ?? '', 'plus');
    popOv(undefined, true);
    setS('dockId', null);
  },
  rail(a) {
    if (a.startsWith('shop:')) setS('shopCat', a.slice(5));
    else if (a.startsWith('cat:')) {
      setS('setCat', a.slice(4));
    } else setS('libFilter', a);
  },
  key(k) {
    const sink = textSink.cur;
    if (!sink) return;
    if (k === 'space') sink({ kind: 'char', ch: ' ', screen: true });
    else if (k === 'del') sink({ kind: 'del' });
    else if (k === 'clear') sink({ kind: 'clear' });
    else if (k === 'ok') sink({ kind: 'enter' });
    else if (k === 'shift') sink({ kind: 'shift' });
    else if (k === 'reveal') sink({ kind: 'reveal' });
    else sink({ kind: 'char', ch: k, screen: true });
  },
  tog(k) {
    const cur = prefs()[k as 'sfx'];
    setPref({ [k]: !cur } as never);
    sfx(!cur ? 'on' : 'off');
  },
  cyc(k) {
    cycle(k, 1);
  },
  fs() {
    toggleFS();
  },
  switch() {
    if (topOv()?.kind === 'guide') popOv(undefined, true);
    void leaveProfile();
  },
  restart() {
    void powerAction('restart');
  },
  /** Tela "o Nexus encontrou um problema" (Core fora do ar): ações fixas executadas pelo Electron. */
  async fault(a: string) {
    if (!IS_ELECTRON) return setS('faultMsg', 'Indisponível no navegador: reinicie o nexus-core no terminal.');
    if (a === 'retry') {
      setS('faultMsg', 'Reiniciando o serviço principal…');
      if (!(await recover('restart-core'))) setS('faultMsg', 'Não deu certo. Tente “Reiniciar o Nexus”.');
    } else if (a === 'session') {
      setS('faultMsg', 'Reiniciando o Nexus…');
      if (!(await recover('restart-session'))) setS('faultMsg', 'Não deu certo. Tente reiniciar o aparelho.');
    } else if (a === 'reboot') {
      if (!(await askConfirm({ title: 'Reiniciar o aparelho?', sub: 'Jogos abertos serão fechados.', yes: 'Reiniciar', no: 'Cancelar', danger: true }))) return;
      setS('faultMsg', 'Reiniciando o aparelho…');
      if (!(await recover('reboot'))) setS('faultMsg', 'Não foi possível reiniciar. Segure o botão de energia.');
    }
  },
  off() {
    void powerAction('off');
  },
  async steamsync() {
    try {
      const r = await api.steamSync();
      toast(r.hasKey ? `${r.count} jogos na conta` : 'Chave da Web API não configurada', r.hasKey ? 'Biblioteca atualizada' : 'Informe a chave em Ajustes → Steam', 'download');
      S.steam && setS('steam', await api.steamAccount());
    } catch (e) {
      fail(e);
    }
  },
  async steamkey() {
    const { askText } = await import('./state');
    const k = await askText({ title: 'Chave da Web API da Steam', max: 32, kind: 'key' });
    if (!k) return;
    try {
      await api.steamSetKey(k.trim());
      toast('Chave salva', 'Sincronizando…', 'check');
      await ACT.steamsync('');
    } catch (e) {
      fail(e);
    }
  },
};

const DEBUG = typeof localStorage !== 'undefined' && localStorage.getItem('nexus-debug') === '1';

export function activate(el: HTMLElement | null) {
  if (!el) return;
  const a = el.dataset.a || '';
  if (!a) return;
  const i = a.indexOf(':');
  const n = i < 0 ? a : a.slice(0, i);
  const arg = i < 0 ? '' : a.slice(i + 1);
  if (DEBUG) console.debug('[act]', n, arg);
  if (n === 'tabgo' || n === 'tab') sfx('tab');
  else if (!['tog', 'noop', 'fav', 'pin', 'key', 'rail', 'hubtab', 'cyc'].includes(n)) sfx('select');
  void ACT[n]?.(arg);
}

export function optionsFor(el: HTMLElement | null) {
  const id = el?.dataset.id;
  if (!id || !el.dataset.opt || !findItem(id)) return;
  const t = topOv();
  if (t && t.kind !== 'hub') return;
  ACT.opts(id);
}

export function hubStep(d: 1 | -1) {
  document.dispatchEvent(new CustomEvent('nexus:hub-step', { detail: d }));
}

export { audioInit, me, setMode };
