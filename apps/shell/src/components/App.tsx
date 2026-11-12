import { createEffect, createMemo, createSignal, For, on, onCleanup, onMount, Show, Switch, Match, type JSX } from 'solid-js';
import { Focus } from '@nexus/focus';
import { sfx } from '@nexus/ui';
import { ACT, THEME_LABEL, back, firstFocus, adjust, tab, tabsNow } from '../lib/actions';
import { api, artUrl } from '../lib/api';
import { installInput, startRemoteFallback, vcur } from '../lib/input';
import { bootstrap, startEvents } from '../lib/session';
import { bgFront, focusNow, fx, isKid, me, mode, ovs, prefs, S, setS, toasts, topOv, closeAllOv, findItem, setBg, hasOv, type Ov } from '../lib/state';
import { appBg, Avatar, Glyph, Icon } from './ui';
import { Home, Library, Media, Store, setStripItem } from './Views';
import { Emulators } from './Emu';
import { Roms } from './Roms';
import { Hub } from './Hub';
import { Confirm, Fault, Guide, Launch, Notifs, Search, Sheet, TimeUp, clock } from './Overlays';
import { PinPad, TextEntry } from './Entry';
import { Settings } from './Settings';
import { AppEditor, AppPicker, FilePicker } from './Apps';
import { PadEditor, PadMapper } from './Pads';
import { Boot, Onboarding, ProfileEditor, ProfilePicker, Standby } from './Profiles';

const FULL: Ov['kind'][] = ['fault', 'hub', 'search', 'settings', 'launch', 'profile', 'timeup', 'appicker', 'appedit', 'filepick', 'padmap', 'padedit', 'roms'];

function OvBody(p: { o: Ov }) {
  const o = p.o;
  return (
    <Switch>
      <Match when={o.kind === 'sheet'}><Sheet type={o.props.type} id={o.props.id} profileId={o.props.profileId} oid={o.id} /></Match>
      <Match when={o.kind === 'hub'}><Hub id={o.props.id} oid={o.id} /></Match>
      <Match when={o.kind === 'search'}><Search /></Match>
      <Match when={o.kind === 'settings'}><Settings /></Match>
      <Match when={o.kind === 'guide'}><Guide /></Match>
      <Match when={o.kind === 'notif'}><Notifs /></Match>
      <Match when={o.kind === 'launch'}><Launch id={o.props.id} /></Match>
      <Match when={o.kind === 'pin'}><PinPad title={o.props.title} sub={o.props.sub} confirm={o.props.confirm} oid={o.id} /></Match>
      <Match when={o.kind === 'text'}><TextEntry title={o.props.title} value={o.props.value} max={o.props.max} kind={o.props.kind} oid={o.id} /></Match>
      <Match when={o.kind === 'profile'}><ProfileEditor id={o.props.id} first={o.props.first} oid={o.id} /></Match>
      <Match when={o.kind === 'timeup'}><TimeUp /></Match>
      <Match when={o.kind === 'appicker'}><AppPicker oid={o.id} /></Match>
      <Match when={o.kind === 'appedit'}><AppEditor id={o.props.id} fresh={o.props.fresh} oid={o.id} /></Match>
      <Match when={o.kind === 'padedit'}><PadEditor pad={o.props.pad} oid={o.id} /></Match>
      <Match when={o.kind === 'padmap'}><PadMapper pad={o.props.pad} oid={o.id} /></Match>
      <Match when={o.kind === 'roms'}><Roms oid={o.id} /></Match>
      <Match when={o.kind === 'fault'}><Fault /></Match>
      <Match when={o.kind === 'filepick'}><FilePicker title={o.props.title} oid={o.id} /></Match>
      <Match when={o.kind === 'confirm'}><Confirm title={o.props.title} sub={o.props.sub} yes={o.props.yes} no={o.props.no} danger={o.props.danger} oid={o.id} /></Match>
    </Switch>
  );
}

function Background() {
  let a!: HTMLDivElement;
  let b!: HTMLDivElement;
  createEffect(() => {
    const it = findItem(S.bgId);
    const front = bgFront();
    const next = front === 'A' ? a : b;
    const prev = front === 'A' ? b : a;
    if (!it) {
      next.classList.remove('on');
      return;
    }
    next.style.background = !it.art.hero ? appBg(it) : `url("${artUrl(it.id, 'hero')}") 70% 40%/cover, linear-gradient(135deg,hsl(${it.hue[0]} 55% 30%),hsl(${it.hue[1]} 55% 14%))`;
    next.classList.add('on');
    prev.classList.remove('on');
  });
  return (
    <div id="bgs" classList={{ dim: S.tab !== 'home' }}>
      <div class="bg" ref={a} />
      <div class="bg" ref={b} />
      <div class="scrim" />
    </div>
  );
}

interface FocusInfo { verb: string; opt: boolean; adj: boolean; acts: boolean }
const [finfo, setFinfo] = createSignal<FocusInfo>({ verb: 'Selecionar', opt: false, adj: false, acts: false });

function Prompts() {
  const items = createMemo<(string[])[]>(() => {
    const top = topOv();
    const ov = top?.kind;
    const f = finfo();
    const out: string[][] = [];
    const sl = f.adj;
    if (S.scr === 'standby') out.push(['A', 'Ligar']);
    else if (S.scr === 'profiles' || S.scr === 'onboarding') out.push(['A', f.verb]);
    else if (S.scr === 'shell' || ov) {
      if (ov === 'launch') out.push(['A', f.verb], ['B', 'Início'], ['G', 'Central']);
      else if (ov === 'hub') out.push(['A', f.verb], ['B', 'Voltar'], ['OPT', 'Opções'], ['LB', 'RB', 'Abas']);
      else if (ov === 'settings') out.push(sl ? ['◀', 'Ajustar'] : ['A', f.verb], ['B', sl ? 'Sair do controle' : 'Voltar']);
      else if (ov === 'pin' || ov === 'text') out.push(['A', 'Digitar'], ['B', 'Cancelar']);
      else if (ov === 'filepick') out.push(['A', f.verb], ['B', 'Cancelar']);
      else if (ov === 'timeup') out.push(['A', f.verb]);
      else if (ov) out.push(sl ? ['◀', 'Ajustar'] : ['A', f.verb], ['B', 'Fechar']);
      else {
        out.push(['A', f.verb]);
        if (S.tab !== 'home' || f.acts) out.push(['B', 'Voltar']);
        if (f.opt) out.push(['OPT', 'Opções']);
        out.push(['Y', 'Busca'], ['LB', 'RB', 'Seções'], ['G', 'Central']);
      }
    }
    return out;
  });
  const needBack = () => S.scr === 'shell' && (ovs().length > 0 || S.tab !== 'home') && topOv()?.kind !== 'timeup';
  return (
    <div id="prompts">
      <For each={items()}>
        {(it) => (
          <span class="pr">
            <Show when={it[0] === '◀'} fallback={<For each={it.slice(0, -1)}>{(k) => <Glyph k={k as 'A'} />}</For>}>
              <i class="gl"><Icon n="left" /></i><i class="gl"><Icon n="right" /></i>
            </Show>
            {it[it.length - 1]}
          </span>
        )}
      </For>
      <div id="backBtn" class={`f ${needBack() ? 'show' : ''}`} data-a="goback"><Icon n="left" /> Voltar</div>
    </div>
  );
}

function Bar() {
  const t = createMemo(tabsNow);
  return (
    <header id="bar">
      <nav id="tabs" role="tablist" aria-label="Seções">
        <For each={t()}>
          {([k, l]) => (
            <div class={`tab f ${S.tab === k ? 'on' : ''}`} data-tab={k} data-a={`tabgo:${k}`} data-r="p" data-pad="6" data-verb="Entrar" role="tab" aria-selected={S.tab === k}>{l}</div>
          )}
        </For>
      </nav>
      <div id="sys">
        <Show when={isKid() && S.session?.remainingMin !== null && S.session?.remainingMin !== undefined}>
          <div class="np timepill"><Icon n="clock" /><span>{S.session!.remainingMin} min</span></div>
        </Show>
        <Show when={S.game}>
          <div class="np f" data-a="resume" data-r="p" data-pad="6" data-verb="Retomar"><span class="dot" /><span>{S.game!.title}</span></div>
        </Show>
        <div class="sb f" id="sbSearch" data-a="search" data-r="p" data-verb="Buscar" aria-label="Buscar"><Icon n="search" /></div>
        <Show when={!isKid()}>
          <div class="sb f" id="sbBell" data-a="notif" data-r="p" data-verb="Abrir" aria-label="Notificações"><Icon n="bell" /><Show when={S.unread}><i class="ndot" /></Show></div>
        </Show>
        <div class="sb f" id="sbGear" data-a="settings" data-r="p" data-verb="Abrir" aria-label="Ajustes"><Icon n="gear" /></div>
        <Show when={me()}>{(p) => <div class="sb av f" id="sbMe" data-a="guide" data-r="p" data-verb="Abrir" aria-label="Central de controle e perfil"><Avatar p={p()} class="sbav" /></div>}</Show>
        <span id="clock">{clock()}</span>
      </div>
    </header>
  );
}

function Shell() {
  return (
    <>
      <Background />
      <main id="view">
        <Switch>
          <Match when={S.tab === 'home'}><Home /></Match>
          <Match when={S.tab === 'biblioteca'}><Library /></Match>
          <Match when={S.tab === 'emu'}><Emulators /></Match>
          <Match when={S.tab === 'midia'}><Media /></Match>
          <Match when={S.tab === 'loja'}><Store /></Match>
        </Switch>
      </main>
      <Bar />
    </>
  );
}

export function App() {
  let stage!: HTMLDivElement;
  let ring!: HTMLDivElement;
  const [ready, setReady] = createSignal(false);
  const fit = () => document.documentElement.style.setProperty('--s', String(Math.min(innerWidth / 1920, innerHeight / 1080)));

  const root = (): HTMLElement | null => {
    const layers = document.querySelectorAll<HTMLElement>('#ovs > [data-oid]');
    if (layers.length) return layers[layers.length - 1];
    return document.getElementById(S.scr);
  };

  // a faixa de título mostra o item focado da aba atual: ao trocar de aba/seção/console, volta ao título dela
  createEffect(on(() => [S.tab, S.shopCat, S.libFilter, S.emuSys], () => setStripItem(null), { defer: true }));
  let tabT: ReturnType<typeof setTimeout> | undefined;
  let railT: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    fit();
    addEventListener('resize', fit);
    const f = new Focus({
      stage,
      ring,
      getRoot: root,
      // efeitos mínimos (sem GPU) também: o anel deslizando a poucos quadros por segundo ficava para trás
      reduceMotion: () => document.body.classList.contains('reduce'),
      onAdjust: adjust,
      onBump: () => sfx('bump'),
      onLost: () => queueMicrotask(() => requestAnimationFrame(() => fx.f?.ensure())),
      onFocus: (el, o) => {
        setFinfo({ verb: el.dataset.verb || 'Selecionar', opt: !!el.dataset.opt, adj: !!(el.dataset.slider || el.dataset.cyc), acts: !!el.closest('#acts') });
        if (!o.silent) sfx('move');
        if (el.classList.contains('tab') && el.dataset.tab !== S.tab) {
          clearTimeout(tabT);
          tabT = setTimeout(() => focusNow() === el && tab(el.dataset.tab!), 210);
        }
        if (el.dataset.rail && !el.classList.contains('on')) {
          clearTimeout(railT);
          railT = setTimeout(() => focusNow() === el && ACT.rail(el.dataset.rail!), 170);
        }
        const id = el.dataset.id;
        const dk = el.dataset.dock;
        if (dk === 'lib') setS('dockId', 'lib');
        else if (dk && id) {
          setS('dockId', id);
          setBg(findItem(id));
        } else if (id && S.scr === 'shell' && !ovs().length) {
          const it = findItem(id);
          setBg(it);
          setStripItem(it ?? null);
        }
      },
    });
    fx.f = f;
    f.start();
    const off = installInput(stage);
    startEvents();
    startRemoteFallback();
    const focusHome = () => f.set(firstFocus('home'), { silent: true, snap: true });
    document.addEventListener('nexus:focus-home', focusHome);
    const slider = (e: Event) => {
      const v = (e as CustomEvent<number>).detail;
      void import('../lib/state').then((m) => m.setPref({ vol: v }));
    };
    document.addEventListener('nexus:slider-set', slider);
    const started = Date.now();
    bootstrap()
      .then(async (to) => {
        const wait = Math.max(0, 2600 - (Date.now() - started));
        if (to !== 'shell') await new Promise((r) => setTimeout(r, wait));
        if (to !== 'shell') setS('scr', to);
        setReady(true);
      })
      .catch(() => setReady(true));
    const kidPoll = setInterval(() => {
      if (isKid()) void api.session().then((s) => s && S.session && setS('session', 'remainingMin', s.remainingMin)).catch(() => {});
    }, 30000);
    onCleanup(() => {
      off();
      clearInterval(kidPoll);
      f.stop();
      removeEventListener('resize', fit);
      document.removeEventListener('nexus:focus-home', focusHome);
      document.removeEventListener('nexus:slider-set', slider);
    });
  });

  // foco inicial ao trocar de tela principal
  createEffect(on(() => S.scr, (s) => {
    if (s === 'shell') return;
    queueMicrotask(() => requestAnimationFrame(() => fx.f?.first(document.getElementById(s))));
  }, { defer: true }));
  createEffect(on(() => S.scr, () => closeAllOv(), { defer: true }));

  return (
    <div id="stage" ref={stage} data-scr={S.scr} data-ready={ready() ? '1' : '0'}>
      <section id="standby" class={`scr ${S.scr === 'standby' ? 'on' : ''}`}><Show when={S.scr === 'standby'}><Standby /></Show></section>
      <section id="boot" class={`scr ${S.scr === 'boot' ? 'on' : ''}`}><Show when={S.scr === 'boot'}><Boot /></Show></section>
      <section id="onboarding" class={`scr ${S.scr === 'onboarding' ? 'on' : ''}`}><Show when={S.scr === 'onboarding'}><Onboarding /></Show></section>
      <section id="profiles" class={`scr ${S.scr === 'profiles' ? 'on' : ''}`}><Show when={S.scr === 'profiles'}><ProfilePicker /></Show></section>
      <section id="shell" class={`scr ${S.scr === 'shell' ? 'on' : ''}`}><Show when={S.scr === 'shell'}><Shell /></Show></section>
      <div id="ovs">
        <For each={ovs()}>
          {(o) => (
            <div class={`ov ${FULL.includes(o.kind) ? 'full' : ''} ${o.kind === 'hub' ? 'hub' : ''} k-${o.kind}`} data-oid={o.id} data-kind={o.kind} data-ov={o.kind}>
              <OvBody o={o} />
            </div>
          )}
        </For>
      </div>
      <Prompts />
      <div id="ring" ref={ring} />
      <div id="toasts" role="status" aria-live="polite">
        <For each={toasts()}>
          {(t) => (
            <div class={`toast ${t.out ? 'out' : ''}`}>
              <Icon n={t.icon} />
              <span>{t.title}{t.sub ? <> <small>{t.sub}</small></> : null}</span>
            </div>
          )}
        </For>
      </div>
      <Show when={vcur().show}><div id="vcur" style={{ left: `${vcur().x}px`, top: `${vcur().y}px` }} /></Show>
      <Show when={!S.conn && S.scr !== 'boot' && !hasOv('fault')}>
        <div class="offline"><span class="spin" />Reconectando ao Nexus Core…</div>
      </Show>
    </div>
  );
}

export { THEME_LABEL, back, mode, hasOv, ACT };
export type { JSX };
