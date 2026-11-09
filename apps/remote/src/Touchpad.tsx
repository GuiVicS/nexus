import { createSignal, For, onCleanup, onMount, Show } from 'solid-js';
import type { Link } from './link';

const EXTRA: [string, string][] = [['esc', 'Esc'], ['tab', 'Tab'], ['up', '↑'], ['down', '↓'], ['left', '←'], ['right', '→'], ['enter', 'Enter'], ['backspace', '⌫']];

export function Touchpad(p: { link: Link; status: string; onLeave: () => void }) {
  const link = p.link;
  let pad!: HTMLDivElement;
  let inp!: HTMLInputElement;
  const [kb, setKb] = createSignal(false);
  const [mods, setMods] = createSignal<string[]>([]);
  const [sens, setSens] = createSignal(Number(localStorage.getItem('nx-sens') ?? 1.6));
  const [leftDown, setLeftDown] = createSignal(false);
  let prev = '';

  // ---- ponteiro: 1 dedo move, toque curto clica, 2 dedos rolam / toque com 2 dedos = botão direito, toque-e-arraste segura o clique
  const pts = new Map<number, { x: number; y: number; sx: number; sy: number; t: number }>();
  let moved = false;
  let maxFingers = 0;
  let lastTap = 0;
  let dragging = false;
  let scrollAcc = 0;
  let accX = 0, accY = 0;

  const down = (e: PointerEvent) => {
    pad.setPointerCapture(e.pointerId);
    e.preventDefault();
    pts.set(e.pointerId, { x: e.clientX, y: e.clientY, sx: e.clientX, sy: e.clientY, t: Date.now() });
    maxFingers = Math.max(maxFingers, pts.size);
    if (pts.size === 1) {
      moved = false;
      if (Date.now() - lastTap < 260) {
        dragging = true;
        link.send({ t: 'click', b: 'left', down: true });
        setLeftDown(true);
      }
    }
  };
  const move = (e: PointerEvent) => {
    const pt = pts.get(e.pointerId);
    if (!pt) return;
    const dx = e.clientX - pt.x, dy = e.clientY - pt.y;
    pt.x = e.clientX;
    pt.y = e.clientY;
    if (Math.hypot(e.clientX - pt.sx, e.clientY - pt.sy) > 6) moved = true;
    if (pts.size >= 2) {
      scrollAcc += dy;
      const steps = Math.trunc(scrollAcc / 14);
      if (steps) {
        scrollAcc -= steps * 14;
        link.send({ t: 'scroll', v: -steps, h: 0 });
      }
      return;
    }
    // ganho com aceleração leve
    const g = sens() * (1 + Math.min(1.2, Math.hypot(dx, dy) / 40));
    accX += dx * g;
    accY += dy * g;
    const ix = Math.trunc(accX), iy = Math.trunc(accY);
    if (ix || iy) {
      accX -= ix;
      accY -= iy;
      link.send({ t: 'move', dx: ix, dy: iy });
    }
  };
  const up = (e: PointerEvent) => {
    const pt = pts.get(e.pointerId);
    pts.delete(e.pointerId);
    if (!pt) return;
    if (pts.size === 0) {
      const quick = Date.now() - pt.t < 230;
      if (dragging) {
        link.send({ t: 'click', b: 'left', down: false });
        setLeftDown(false);
        dragging = false;
      } else if (!moved && quick) {
        const b = maxFingers >= 2 ? 'right' : 'left';
        link.send({ t: 'click', b, down: true });
        link.send({ t: 'click', b, down: false });
        if (b === 'left') lastTap = Date.now();
        try { navigator.vibrate?.(8); } catch { /* sem vibração */ }
      }
      maxFingers = 0;
      scrollAcc = 0;
    }
  };
  const cancel = (e: PointerEvent) => {
    pts.delete(e.pointerId);
    if (dragging) { link.send({ t: 'click', b: 'left', down: false }); setLeftDown(false); dragging = false; }
    if (!pts.size) maxFingers = 0;
  };

  const btnHold = (b: 'left' | 'right') => ({
    onPointerDown: (e: PointerEvent) => { (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId); e.preventDefault(); link.send({ t: 'click', b, down: true }); },
    onPointerUp: () => link.send({ t: 'click', b, down: false }),
    onPointerCancel: () => link.send({ t: 'click', b, down: false }),
    onContextMenu: (e: Event) => e.preventDefault(),
  });

  // ---- teclado: o campo oculto envia só o que mudou (texto + backspaces), inclusive com autocorreção
  const onInput = () => {
    const cur = inp.value;
    let i = 0;
    while (i < prev.length && i < cur.length && prev[i] === cur[i]) i++;
    const back = prev.length - i;
    const text = cur.slice(i);
    if (back || text) link.send({ t: 'text', back, text, mods: mods() });
    prev = cur;
    if (cur.length > 200) { inp.value = ''; prev = ''; }
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'Enter') { e.preventDefault(); link.send({ t: 'key', key: 'enter', mods: mods() }); }
    else if (e.key === 'Backspace' && inp.value === '') link.send({ t: 'key', key: 'backspace', mods: mods() });
  };
  const toggleKb = () => {
    const on = !kb();
    setKb(on);
    if (on) { inp.value = ''; prev = ''; inp.focus(); } else inp.blur();
  };
  const toggleMod = (m: string) => setMods((l) => (l.includes(m) ? l.filter((x) => x !== m) : [...l, m]));
  const sendKey = (k: string) => { link.send({ t: 'key', key: k, mods: mods() }); try { navigator.vibrate?.(6); } catch { /* ok */ } setMods([]); };

  onMount(() => {
    const prevent = (e: Event) => e.preventDefault();
    pad.addEventListener('touchmove', prevent, { passive: false });
    onCleanup(() => pad.removeEventListener('touchmove', prevent));
  });

  return (
    <div class="tp-screen">
      <div class="topbar">
        <button class="ghost" onClick={p.onLeave}>‹ Sair</button>
        <span class={`pill ${p.status}`}>Mouse e teclado · {p.status === 'open' ? 'conectado' : 'reconectando…'}</span>
      </div>
      <div class="tp-main">
        <div class={`trackpad ${leftDown() ? 'drag' : ''}`} ref={pad} onPointerDown={down} onPointerMove={move} onPointerUp={up} onPointerCancel={cancel} onContextMenu={(e) => e.preventDefault()}>
          <span class="hint">Toque para clicar · dois dedos rolam · dois dedos tocam = botão direito · toque e arraste</span>
        </div>
        <div class="tp-side">
          <button class="mbtn" {...btnHold('left')}>Esquerdo</button>
          <button class="mbtn" {...btnHold('right')}>Direito</button>
          <button class={`mbtn ${kb() ? 'on' : ''}`} onClick={toggleKb}>⌨ Teclado</button>
        </div>
      </div>
      <div class="sens">
        <label>Velocidade</label>
        <input type="range" min="0.6" max="3.2" step="0.1" value={sens()} onInput={(e) => { setSens(+e.currentTarget.value); localStorage.setItem('nx-sens', String(sens())); }} />
      </div>
      <Show when={kb() || true}>
        <div class="keybar">
          <For each={[['ctrl', 'Ctrl'], ['alt', 'Alt'], ['shift', 'Shift'], ['meta', 'Super']]}>{([m, l]) => <button class={`kk mod ${mods().includes(m) ? 'on' : ''}`} onClick={() => toggleMod(m)}>{l}</button>}</For>
          <For each={EXTRA}>{([k, l]) => <button class="kk" onClick={() => sendKey(k)}>{l}</button>}</For>
          <For each={['f1', 'f2', 'f3', 'f4', 'f5', 'f10', 'f11']}>{(k) => <button class="kk" onClick={() => sendKey(k)}>{k.toUpperCase()}</button>}</For>
        </div>
      </Show>
      <input ref={inp} class="ghost-input" autocomplete="off" autocapitalize="off" autocorrect="off" spellcheck={false} onInput={onInput} onKeyDown={onKey} onBlur={() => setKb(false)} aria-label="Campo de digitação" />
    </div>
  );
}
