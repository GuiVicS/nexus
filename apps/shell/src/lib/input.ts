import { createSignal } from 'solid-js';
import { audioInit, sfx } from '@nexus/ui';
import { ACT, activate, back, hubStep, optionsFor, tabStep, textSink, toggleFS } from './actions';
import { client } from './api';
import { focusNow, fx, hasOv, ovs, S, setMode, topOv } from './state';

export function input(k: string) {
  audioInit();
  const f = fx.f;
  if (!f) return;
  if (S.scr === 'standby') {
    if (k === 'A') ACT.boot('');
    return;
  }
  if (S.scr === 'boot') return;
  if (k.length === 1 && 'udlr'.includes(k)) return f.move(k as 'u');
  if (k === 'A') return activate(focusNow());
  if (k === 'B') return back();
  if (topOv()?.kind === 'timeup' || topOv()?.kind === 'pin' || topOv()?.kind === 'text') {
    if (k === 'Y' || k === 'OPT' || k === 'G') return;
  }
  if (k === 'Y') return ACT.search('');
  if (k === 'OPT') return optionsFor(focusNow());
  if (k === 'G') return ACT.guide('');
  if (k === 'LB' || k === 'RB') {
    if (topOv()?.kind === 'hub') return hubStep(k === 'RB' ? 1 : -1);
    return tabStep(k === 'RB' ? 1 : -1);
  }
  if (k === 'LT') return f.page(-1);
  if (k === 'RT') return f.page(1);
}

const KEYS: Record<string, string> = {
  ArrowUp: 'u', ArrowDown: 'd', ArrowLeft: 'l', ArrowRight: 'r', Enter: 'A', ' ': 'A', Escape: 'B', Backspace: 'B',
  s: 'Y', S: 'Y', m: 'OPT', M: 'OPT', o: 'OPT', q: 'LB', Q: 'LB', e: 'RB', E: 'RB', PageUp: 'LT', PageDown: 'RT', Tab: 'G', F1: 'G', g: 'G', G: 'G', Home: 'G',
};

// ---------- cursor virtual (celular sem /dev/uinput) ----------
export const [vcur, setVcur] = createSignal({ x: 960, y: 540, show: false });

function stagePoint() {
  const st = document.getElementById('stage')!;
  const r = st.getBoundingClientRect();
  const s = r.width / 1920;
  const { x, y } = vcur();
  return { cx: r.left + x * s, cy: r.top + y * s };
}

function elementAtCursor(): HTMLElement | null {
  const { cx, cy } = stagePoint();
  return document.elementFromPoint(cx, cy) as HTMLElement | null;
}

function rootContains(el: HTMLElement) {
  const r = fx.f?.root();
  return !!r && r.contains(el);
}

const repeats = new Map<string, { t1: number; t2: number }>();
function stopRepeat(k?: string) {
  for (const [key, r] of repeats) {
    if (k && key !== k) continue;
    clearTimeout(r.t1);
    clearInterval(r.t2);
    repeats.delete(key);
  }
}

export function startRemoteFallback() {
  client.on('remote.activity', () => setMode('pad'));
  client.on('remote.input', (p: { key?: string; down?: boolean; reset?: boolean }) => {
    if (p.reset) return stopRepeat();
    if (!p.key) return;
    setMode('pad');
    if (!p.down) return stopRepeat(p.key);
    input(p.key);
    if ('udlr'.includes(p.key)) {
      stopRepeat(p.key);
      const key = p.key;
      const t1 = window.setTimeout(() => {
        const t2 = window.setInterval(() => input(key), 120);
        repeats.set(key, { t1, t2 });
      }, 380);
      repeats.set(key, { t1, t2: 0 });
    }
  });
  client.on('remote.pointer', (p: { dx: number; dy: number }) => {
    setMode('ptr');
    setVcur((v) => ({ x: Math.max(0, Math.min(1919, v.x + p.dx * 2.2)), y: Math.max(0, Math.min(1079, v.y + p.dy * 2.2)), show: true }));
    const el = elementAtCursor()?.closest<HTMLElement>('.f');
    if (el && el !== focusNow() && rootContains(el)) fx.f?.set(el, { mouse: true, silent: true });
  });
  client.on('remote.button', (p: { b: string; down: boolean }) => {
    if (!p.down) return;
    audioInit();
    if (S.scr === 'standby') return input('A');
    if (p.b === 'right') return back();
    const t = elementAtCursor();
    if (t?.classList.contains('veil')) return back();
    const el = t?.closest<HTMLElement>('.f');
    if (!el || !rootContains(el)) return;
    if (el.dataset.slider) {
      const tr = el.querySelector('.tr')?.getBoundingClientRect();
      if (tr) {
        const { cx } = stagePoint();
        document.dispatchEvent(new CustomEvent('nexus:slider-set', { detail: Math.round(Math.max(0, Math.min(1, (cx - tr.left) / tr.width)) * 100) }));
      }
      return;
    }
    fx.f?.set(el, { mouse: true, silent: true });
    activate(el);
  });
  client.on('remote.scroll', (p: { v: number }) => {
    let el: HTMLElement | null = elementAtCursor();
    while (el && !(el.scrollHeight > el.clientHeight + 4 && /(auto|scroll)/.test(getComputedStyle(el).overflowY))) el = el.parentElement;
    const row = elementAtCursor()?.closest<HTMLElement>('.drow');
    if (row) row.scrollLeft += p.v * 60;
    else if (el) el.scrollTop += p.v * 60;
  });
  client.on('remote.key', (p: { key: string }) => {
    setMode('key');
    const map: Record<string, string> = { up: 'u', down: 'd', left: 'l', right: 'r', enter: 'A', esc: 'B', tab: 'G', backspace: 'B', pageup: 'LT', pagedown: 'RT', home: 'G', menu: 'OPT' };
    if (p.key === 'backspace' && textSink.cur) return textSink.cur({ kind: 'del' });
    if (p.key === 'enter' && textSink.cur && textSink.enter) return textSink.cur({ kind: 'enter' });
    if (p.key === 'space' && textSink.cur) return textSink.cur({ kind: 'char', ch: ' ' });
    const k = map[p.key];
    if (k) input(k);
  });
  client.on('remote.text', (p: { back: number; text: string }) => {
    const sink = textSink.cur;
    if (!sink) return;
    for (let i = 0; i < p.back; i++) sink({ kind: 'del' });
    for (const ch of p.text) sink(ch === '\n' ? { kind: 'enter' } : { kind: 'char', ch });
  });
}

export function installInput(stage: HTMLElement): () => void {
  const off: (() => void)[] = [];
  const on = <T extends EventTarget>(t: T, ev: string, fn: (e: any) => void, o?: AddEventListenerOptions) => {
    t.addEventListener(ev, fn, o);
    off.push(() => t.removeEventListener(ev, fn, o));
  };

  on(window, 'keydown', (e: KeyboardEvent) => {
    setMode('key');
    audioInit();
    const sink = textSink.cur;
    if (sink && !e.ctrlKey && !e.metaKey && !e.altKey) {
      if (e.key.length === 1) {
        e.preventDefault();
        sink({ kind: 'char', ch: e.key });
        sfx('key');
        return;
      }
      if (e.key === 'Backspace') {
        e.preventDefault();
        sink({ kind: 'del' });
        sfx('key');
        return;
      }
      if (e.key === 'Enter' && textSink.enter) {
        e.preventDefault();
        sink({ kind: 'enter' });
        return;
      }
    }
    if ((e.key === 'f' || e.key === 'F') && !e.ctrlKey && !e.metaKey) {
      e.preventDefault();
      toggleFS();
      return;
    }
    const k = KEYS[e.key];
    if (!k) return;
    e.preventDefault();
    if (e.repeat && !'udlr'.includes(k)) return;
    input(k);
  });

  on(stage, 'click', (e: MouseEvent) => {
    audioInit();
    if (S.scr === 'standby') return input('A');
    const t = e.target as HTMLElement;
    if (t.classList.contains('veil')) return back();
    const el = t.closest<HTMLElement>('.f');
    if (!el || !rootContains(el)) return;
    if (el.id === 'backBtn') {
      sfx('close');
      return back();
    }
    if (el.dataset.slider) {
      const tr = el.querySelector('.tr')!.getBoundingClientRect();
      document.dispatchEvent(new CustomEvent('nexus:slider-set', { detail: Math.round(Math.max(0, Math.min(1, (e.clientX - tr.left) / tr.width)) * 100) }));
      fx.f?.set(el, { mouse: true, silent: true });
      return;
    }
    if (focusNow() !== el) fx.f?.set(el, { mouse: true, silent: true });
    activate(el);
  });

  let lastM = 0;
  on(stage, 'mousemove', (e: MouseEvent) => {
    if (Math.abs(e.movementX) + Math.abs(e.movementY) < 2) return;
    setMode('ptr');
    setVcur((v) => (v.show ? { ...v, show: false } : v));
    if (Date.now() - lastM < 40) return;
    const el = (e.target as HTMLElement).closest<HTMLElement>('.f');
    if (el && el !== focusNow() && rootContains(el)) {
      lastM = Date.now();
      fx.f?.set(el, { mouse: true, silent: true });
    }
  });
  on(stage, 'touchstart', () => setMode('ptr'), { passive: true });
  on(
    stage,
    'wheel',
    (e: WheelEvent) => {
      const r = (e.target as HTMLElement).closest<HTMLElement>('.drow');
      if (r && Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
        r.scrollLeft += e.deltaY;
        e.preventDefault();
      }
    },
    { passive: false },
  );

  // gamepad físico (a Gamepad API só entrega à janela em foco — com um jogo aberto o Core trata o botão Guia)
  const prev: Record<string, boolean> = {};
  const rep: Record<string, number> = {};
  let raf = 0;
  const poll = (t: number) => {
    const p = [...(navigator.getGamepads ? navigator.getGamepads() : [])].find(Boolean);
    if (p && !S.capturing) {
      const b = (i: number) => !!p.buttons[i]?.pressed;
      const ax = p.axes;
      const s: Record<string, boolean> = {
        u: b(12) || ax[1] < -0.55, d: b(13) || ax[1] > 0.55, l: b(14) || ax[0] < -0.55, r: b(15) || ax[0] > 0.55,
        A: b(0), B: b(1), OPT: b(2) || b(9), Y: b(3), LB: b(4), RB: b(5), LT: b(6), RT: b(7), G: b(8) || b(16),
      };
      for (const k in s) {
        if (s[k] && !prev[k]) {
          setMode('pad');
          input(k);
          rep[k] = t + 380;
        } else if (s[k] && k.length === 1 && 'udlr'.includes(k) && t > rep[k]) {
          input(k);
          rep[k] = t + 120;
        }
        prev[k] = s[k];
      }
    }
    raf = requestAnimationFrame(poll);
  };
  raf = requestAnimationFrame(poll);
  off.push(() => cancelAnimationFrame(raf));
  on(window, 'gamepadconnected', () => setMode('pad'));
  on(window, 'gamepaddisconnected', () => setMode('key'));

  return () => off.forEach((f) => f());
}

export { hasOv, ovs };
