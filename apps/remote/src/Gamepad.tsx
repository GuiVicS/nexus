import { createSignal, For, onCleanup, onMount } from 'solid-js';
import type { Link } from './link';

type Axes = { x: number; y: number; rx: number; ry: number };

function haptic(ms = 12) {
  try { navigator.vibrate?.(ms); } catch { /* sem vibração */ }
}

export function Gamepad(p: { link: Link; player: number; status: string; onLeave: () => void }) {
  const link = p.link;
  const held = new Set<string>();
  const axes: Axes = { x: 0, y: 0, rx: 0, ry: 0 };
  let hat: [number, number] = [0, 0];
  let dirty = true;
  let last = '';
  const [pressed, setPressed] = createSignal<Set<string>>(new Set());
  const [portrait, setPortrait] = createSignal(innerHeight > innerWidth);

  const flush = () => {
    const msg = { t: 'state', buttons: [...held], hat, axes: { x: +axes.x.toFixed(3), y: +axes.y.toFixed(3), rx: +axes.rx.toFixed(3), ry: +axes.ry.toFixed(3) } };
    const s = JSON.stringify(msg);
    if (s !== last || dirty) {
      link.send(msg);
      last = s;
      dirty = false;
    }
  };
  let timer: ReturnType<typeof setInterval>;
  onMount(() => {
    timer = setInterval(flush, 16);
    const rs = () => setPortrait(innerHeight > innerWidth);
    addEventListener('resize', rs);
    const vis = () => { if (document.hidden) releaseAll(); };
    document.addEventListener('visibilitychange', vis);
    onCleanup(() => { removeEventListener('resize', rs); document.removeEventListener('visibilitychange', vis); });
  });
  onCleanup(() => { clearInterval(timer); releaseAll(); flush(); });

  const releaseAll = () => {
    held.clear();
    hat = [0, 0];
    axes.x = axes.y = axes.rx = axes.ry = 0;
    setPressed(new Set<string>());
    dirty = true;
  };

  const setBtn = (b: string, on: boolean) => {
    if (on === held.has(b)) return;
    on ? held.add(b) : held.delete(b);
    setPressed(new Set(held));
    dirty = true;
    flush();
    if (on) haptic();
  };

  // cada botão acompanha o seu dedo (pointer capture), então vários dedos funcionam ao mesmo tempo
  const btn = (name: string) => ({
    onPointerDown: (e: PointerEvent) => { (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId); e.preventDefault(); setBtn(name, true); },
    onPointerUp: () => setBtn(name, false),
    onPointerCancel: () => setBtn(name, false),
    onLostPointerCapture: () => setBtn(name, false),
    onContextMenu: (e: Event) => e.preventDefault(),
  });

  const setHat = (x: number, y: number) => {
    if (hat[0] === x && hat[1] === y) return;
    hat = [x, y];
    dirty = true;
    flush();
    haptic(8);
    setPressed(new Set(held));
  };
  const dpad = (e: PointerEvent, down: boolean) => {
    const el = e.currentTarget as HTMLElement;
    if (!down) return setHat(0, 0);
    const r = el.getBoundingClientRect();
    const dx = (e.clientX - (r.left + r.width / 2)) / (r.width / 2);
    const dy = (e.clientY - (r.top + r.height / 2)) / (r.height / 2);
    const dead = 0.28;
    setHat(Math.abs(dx) < dead ? 0 : dx > 0 ? 1 : -1, Math.abs(dy) < dead ? 0 : dy > 0 ? 1 : -1);
  };

  const stick = (xk: 'x' | 'rx', yk: 'y' | 'ry', btnName: string) => {
    let id = -1;
    let el: HTMLElement;
    const [knob, setKnob] = createSignal<[number, number]>([0, 0]);
    let taps = 0;
    const move = (e: PointerEvent) => {
      const r = el.getBoundingClientRect();
      const rad = r.width / 2;
      let dx = (e.clientX - (r.left + rad)) / rad;
      let dy = (e.clientY - (r.top + rad)) / rad;
      const m = Math.hypot(dx, dy);
      if (m > 1) { dx /= m; dy /= m; }
      axes[xk] = dx;
      axes[yk] = dy;
      setKnob([dx, dy]);
      dirty = true;
      flush();
    };
    const props = {
      ref: (e: HTMLElement) => (el = e),
      onPointerDown: (e: PointerEvent) => {
        if (id !== -1) return;
        id = e.pointerId;
        el.setPointerCapture(id);
        e.preventDefault();
        move(e);
        taps = Date.now() - taps < 280 ? 2 : 1;
        if (taps === 2) setBtn(btnName, true);
      },
      onPointerMove: (e: PointerEvent) => { if (e.pointerId === id) move(e); },
      onPointerUp: (e: PointerEvent) => end(e),
      onPointerCancel: (e: PointerEvent) => end(e),
      onContextMenu: (e: Event) => e.preventDefault(),
    };
    const end = (e: PointerEvent) => {
      if (e.pointerId !== id) return;
      id = -1;
      axes[xk] = axes[yk] = 0;
      setKnob([0, 0]);
      setBtn(btnName, false);
      taps = Date.now();
      dirty = true;
    };
    return { props, knob };
  };
  const L = stick('x', 'y', 'l3');
  const R = stick('rx', 'ry', 'r3');
  const on = (b: string) => pressed().has(b);
  const hatOn = (d: string) => (d === 'u' && hat[1] === -1) || (d === 'd' && hat[1] === 1) || (d === 'l' && hat[0] === -1) || (d === 'r' && hat[0] === 1);

  return (
    <div class="pad-screen">
      {portrait() && <div class="rotate">Gire o celular para o modo paisagem</div>}
      <div class="topbar">
        <button class="ghost" onClick={p.onLeave} aria-label="Sair">‹ Sair</button>
        <span class={`pill ${p.status}`}>Jogador {p.player} · {p.status === 'open' ? 'conectado' : 'reconectando…'}</span>
      </div>
      <div class="pad">
        <div class="side left">
          <div class="shoulders"><button class={`sh ${on('l2') ? 'on' : ''}`} {...btn('l2')}>LT</button><button class={`sh ${on('l') ? 'on' : ''}`} {...btn('l')}>LB</button></div>
          <div class="stick" {...L.props}><i style={{ transform: `translate(${L.knob()[0] * 38}%,${L.knob()[1] * 38}%)` }} /></div>
          <div class="dpad" onPointerDown={(e) => { (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId); e.preventDefault(); dpad(e, true); }} onPointerMove={(e) => (e.buttons || e.pointerType === 'touch') && dpad(e, true)} onPointerUp={(e) => dpad(e, false)} onPointerCancel={(e) => dpad(e, false)} onContextMenu={(e) => e.preventDefault()}>
            <For each={['u', 'l', 'r', 'd']}>{(d) => <span class={`dp ${d} ${hatOn(d) ? 'on' : ''}`} />}</For>
          </div>
        </div>
        <div class="mid">
          <button class={`sm ${on('select') ? 'on' : ''}`} {...btn('select')}>Select</button>
          <button class={`sm home ${on('mode') ? 'on' : ''}`} {...btn('mode')} aria-label="Guia">⌂</button>
          <button class={`sm ${on('start') ? 'on' : ''}`} {...btn('start')}>Start</button>
        </div>
        <div class="side right">
          <div class="shoulders"><button class={`sh ${on('r') ? 'on' : ''}`} {...btn('r')}>RB</button><button class={`sh ${on('r2') ? 'on' : ''}`} {...btn('r2')}>RT</button></div>
          <div class="face">
            <button class={`fb y ${on('y') ? 'on' : ''}`} {...btn('y')}>Y</button>
            <button class={`fb x ${on('x') ? 'on' : ''}`} {...btn('x')}>X</button>
            <button class={`fb b ${on('b') ? 'on' : ''}`} {...btn('b')}>B</button>
            <button class={`fb a ${on('a') ? 'on' : ''}`} {...btn('a')}>A</button>
          </div>
          <div class="stick" {...R.props}><i style={{ transform: `translate(${R.knob()[0] * 38}%,${R.knob()[1] * 38}%)` }} /></div>
        </div>
      </div>
    </div>
  );
}
