import { createSignal, For, onCleanup, onMount, Show } from 'solid-js';
import { sfx } from '@nexus/ui';
import { textSink } from '../lib/actions';
import { closeOv, toast } from '../lib/state';
import { Icon } from './ui';

// ---------- PIN ----------
export function PinPad(p: { title: string; sub?: string; confirm?: boolean; oid: number }) {
  const [v, setV] = createSignal('');
  const [first, setFirst] = createSignal<string | null>(null);
  const [shake, setShake] = createSignal(false);
  const title = () => (p.confirm && first() !== null ? 'Repita o PIN' : p.title);
  const submit = () => {
    const pin = v();
    if (pin.length < 4) {
      sfx('deny');
      setShake(true);
      setTimeout(() => setShake(false), 400);
      return;
    }
    if (p.confirm) {
      if (first() === null) {
        setFirst(pin);
        setV('');
        sfx('select');
        return;
      }
      if (first() !== pin) {
        sfx('deny');
        toast('Os PINs não conferem', 'Tente de novo', 'lock');
        setFirst(null);
        setV('');
        return;
      }
    }
    closeOv(p.oid, pin);
  };
  onMount(() => {
    textSink.enter = true;
    textSink.cur = (e) => {
      if (e.kind === 'char' && e.ch && /^\d$/.test(e.ch) && v().length < 6) setV(v() + e.ch);
      else if (e.kind === 'del') setV(v().slice(0, -1));
      else if (e.kind === 'clear') setV('');
      else if (e.kind === 'enter') submit();
    };
  });
  onCleanup(() => {
    textSink.cur = null;
    textSink.enter = false;
  });
  return (
    <>
      <div class="veil" />
      <div class={`pinpad ${shake() ? 'shake' : ''}`}>
        <div class="pk"><Icon n="lock" /></div>
        <h2>{title()}</h2>
        <Show when={p.sub}><p class="sub">{p.sub}</p></Show>
        <div class="dots">
          <For each={[0, 1, 2, 3, 4, 5]}>{(i) => <i class={i < v().length ? 'on' : i >= 4 ? 'opt' : ''} />}</For>
        </div>
        <div class="pad">
          <For each={['1', '2', '3', '4', '5', '6', '7', '8', '9']}>{(d) => <div class="key f" data-autofocus={d === '5' ? '' : undefined} data-r="18" data-pad="6" data-a={`key:${d}`} data-verb="Digitar">{d}</div>}</For>
          <div class="key f" data-r="18" data-pad="6" data-a="key:del" data-verb="Apagar"><Icon n="left" /></div>
          <div class="key f" data-r="18" data-pad="6" data-a="key:0" data-verb="Digitar">0</div>
          <div class="key pri f" data-r="18" data-pad="6" data-a="key:ok" data-verb="Confirmar"><Icon n="check" /></div>
        </div>
      </div>
    </>
  );
}

// ---------- Texto (teclado na tela) ----------
const ROWS = ['1234567890', 'qwertyuiop', 'asdfghjkl', 'zxcvbnm'];

export function TextEntry(p: { title: string; sub?: string; value?: string; max?: number; kind?: 'name' | 'key' | 'secret' | 'ip'; oid: number }) {
  const [v, setV] = createSignal(p.value ?? '');
  // senha/IP/chave: sem maiúscula automática
  const [up, setUp] = createSignal(!p.value && (p.kind ?? 'name') === 'name');
  const [reveal, setReveal] = createSignal(false);
  const secret = p.kind === 'secret';
  // teclado físico ou do celular: o que cada tipo aceita
  const okChar = (c: string) => (secret ? !/\p{C}/u.test(c) : p.kind === 'ip' ? /^[0-9./,]$/.test(c) : /^[\p{L}\p{N} ._@-]$/u.test(c));
  const max = p.max ?? 16;
  const add = (ch: string) => {
    if (v().length >= max) return sfx('deny');
    const c = up() && /\p{L}/u.test(ch) ? ch.toUpperCase() : ch;
    setV(v() + c);
    if (up() && (p.kind ?? 'name') === 'name' && /\p{L}/u.test(ch)) setUp(false);
  };
  onMount(() => {
    textSink.enter = true;
    textSink.cur = (e) => {
      if (e.kind === 'char' && e.ch) {
        if (e.screen) add(e.ch);
        else if (okChar(e.ch) && v().length < max) {
          setV(v() + e.ch);
          setUp(false);
        }
      } else if (e.kind === 'del') setV(v().slice(0, -1));
      else if (e.kind === 'clear') setV('');
      else if (e.kind === 'shift') setUp(!up());
      else if (e.kind === 'reveal') setReveal(!reveal());
      else if (e.kind === 'enter') closeOv(p.oid, secret ? v() : v().trim());
    };
  });
  onCleanup(() => {
    textSink.cur = null;
    textSink.enter = false;
  });
  return (
    <>
      <div class="veil" />
      <div class="textentry">
        <h2>{p.title}</h2>
        <Show when={p.sub}><p class="tesub">{p.sub}</p></Show>
        <div class="sq wide"><span id="tev"><Show when={v()} fallback={<span class="ph2">Digite…</span>}>{secret && !reveal() ? '•'.repeat(v().length) : v().replace(/ /g, ' ')}</Show></span><i class="caret" /><small>{v().length}/{max}</small></div>
        <div class="kbd10">
          <For each={ROWS}>
            {(row) => (
              <div class="krow">
                <For each={row.split('')}>{(k) => <div class="key f" data-autofocus={k === 'a' ? '' : undefined} data-r="18" data-pad="6" data-a={`key:${k}`} data-verb="Digitar">{up() ? k.toUpperCase() : k}</div>}</For>
              </div>
            )}
          </For>
          <div class="krow">
            <div class={`key w2 f ${up() ? 'sel' : ''}`} data-r="18" data-pad="6" data-a="key:shift" data-verb="Maiúsculas">Aa</div>
            <For each={['ç', 'ã', 'é', '.', '-']}>{(k) => <div class="key f" data-r="18" data-pad="6" data-a={`key:${k}`} data-verb="Digitar">{up() ? k.toUpperCase() : k}</div>}</For>
            <div class="key w2 f" data-r="18" data-pad="6" data-a="key:space" data-verb="Digitar">espaço</div>
          </div>
          <Show when={secret || p.kind === 'ip'}>
            <div class="krow">
              <For each={secret ? ['@', '#', '!', '?', '$', '%', '&', '*', '_', '+', '/'] : ['/', ',']}>{(k) => <div class="key f" data-r="18" data-pad="6" data-a={`key:${k}`} data-verb="Digitar">{k}</div>}</For>
              <Show when={secret}><div class="key w2 f" data-k="reveal" data-r="18" data-pad="6" data-a="key:reveal" data-verb={reveal() ? 'Ocultar' : 'Mostrar'}>{reveal() ? 'ocultar' : 'mostrar'}</div></Show>
            </div>
          </Show>
          <div class="krow">
            <div class="key w2 f" data-r="18" data-pad="6" data-a="key:del" data-verb="Apagar"><Icon n="left" /></div>
            <div class="key w2 f" data-r="18" data-pad="6" data-a="key:clear" data-verb="Limpar">limpar</div>
            <div class="key w4 pri f" data-r="18" data-pad="6" data-a="key:ok" data-verb="Confirmar"><Icon n="check" /> Pronto</div>
          </div>
        </div>
      </div>
    </>
  );
}
