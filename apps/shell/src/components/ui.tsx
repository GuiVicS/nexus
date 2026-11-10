import { createMemo, createSignal, For, Show, type JSX } from 'solid-js';
import { ICONS, type IconName } from '@nexus/ui';
import type { Item, Profile } from '@nexus/protocol';
import { artUrl } from '../lib/api';
import { mono, effectivePrice, priceNum } from '../lib/format';
import { mode, S } from '../lib/state';
import { verbOf } from '../lib/actions';

export function Icon(p: { n: string; fill?: boolean; class?: string }) {
  const body = () => ICONS[p.n as IconName] ?? ICONS.info;
  return <svg class={`i ${p.fill ? 'fill' : ''} ${p.class ?? ''}`} viewBox="0 0 24 24" innerHTML={body()} />;
}

/** Glifo de botão (A/B/Y/...) que muda entre controle e teclado. */
const GLY = {
  pad: { A: 'A', B: 'B', Y: 'Y', X: 'X', OPT: '≡', LB: 'LB', RB: 'RB', LT: 'LT', RT: 'RT', G: '⌂' },
  key: { A: 'Enter', B: 'Esc', Y: 'S', X: 'M', OPT: 'M', LB: 'Q', RB: 'E', LT: 'PgUp', RT: 'PgDn', G: 'Tab' },
} as const;

export function Glyph(p: { k: keyof typeof GLY.pad }) {
  const m = () => (mode() === 'pad' ? 'pad' : 'key');
  return (
    <i class={`gl ${p.k}`}>
      <Show when={m() === 'pad' && (p.k === 'OPT' || p.k === 'G')} fallback={GLY[m()][p.k]}>
        <Icon n={p.k === 'OPT' ? 'menu' : 'home'} />
      </Show>
    </i>
  );
}

export const avatarStyle = (p: Pick<Profile, 'avatar' | 'hue' | 'name'>): JSX.CSSProperties =>
  p.avatar === 'initial'
    ? { background: `linear-gradient(135deg,hsl(${p.hue} 60% 42%),hsl(${(p.hue + 50) % 360} 60% 24%))` }
    : { 'background-image': `url(avatars/${p.avatar}.jpg)`, 'background-color': `hsl(${p.hue} 40% 30%)` };

export function Avatar(p: { p: Pick<Profile, 'avatar' | 'hue' | 'name'>; class?: string; size?: number; children?: JSX.Element }) {
  return (
    <div class={`av ${p.class ?? ''}`} style={{ ...avatarStyle(p.p), ...(p.size ? { width: p.size + 'px', height: p.size + 'px' } : {}) }}>
      <Show when={p.p.avatar === 'initial'}>
        <span class="ini">{(p.p.name[0] ?? '?').toUpperCase()}</span>
      </Show>
      {p.children}
    </div>
  );
}

export function Bar(p: { pct: number; class?: string }) {
  return (
    <div class={`bar ${p.class ?? ''}`}>
      <i style={{ width: `${Math.max(0, Math.min(100, p.pct))}%` }} />
    </div>
  );
}

export function Switch(p: { on: boolean }) {
  return <div class="sw" role="switch" aria-checked={p.on} />;
}

const hueGrad = (it: Item) => `linear-gradient(135deg,hsl(${it.hue[0]} 55% 34%),hsl(${it.hue[1]} 55% 18%))`;

export function AppFace(p: { it: Item }) {
  const [broken, setBroken] = createSignal(false);
  const it = () => p.it;
  const hasIcon = () => it().art.icon && !broken();
  const b = () => it().brand;
  return (
    <Show
      when={hasIcon() && b() && b()!.mode !== 'i'}
      fallback={
        <div class="br br-i" style={{ background: b()?.bg ?? hueGrad(it()) }}>
          <Show when={hasIcon()} fallback={<span class="mg">{mono(it().title)}</span>}>
            <img src={artUrl(it().id, 'icon')} alt="" onError={() => setBroken(true)} />
          </Show>
          <b>{it().title}</b>
        </div>
      }
    >
      <div class="br" style={{ background: b()!.bg }}>
        <img class={b()!.mode === 'lw' ? 'w' : ''} src={artUrl(it().id, 'icon')} alt={it().title} onError={() => setBroken(true)} />
      </div>
    </Show>
  );
}

export const appBg = (it: Item) => (it.brand ? it.brand.bg : hueGrad(it));
/** Fundo grande do item: imagem de fundo (hero) quando existe; senão o degradê/cor do app. */
export const heroStyle = (it: Item, extra: Record<string, string> = {}): JSX.CSSProperties => (it.art.hero ? { 'background-image': `url("${artUrl(it.id, 'hero')}")`, 'background-color': `hsl(${it.hue[0]} 40% 20%)`, ...extra } : { background: appBg(it), ...extra });

export function Tile(p: { it: Item; a?: string; dock?: boolean; badge?: string; price?: JSX.Element; noProg?: boolean; defAct?: string }) {
  const it = () => p.it;
  const running = () => S.game?.itemId === it().id;
  const act = () => p.a ?? p.defAct ?? (it().kind === 'app' ? `app:${it().id}` : `open:${it().id}`);
  return (
    <div class="tl f" data-k={it().id} data-id={it().id} data-r="22" data-a={act()} data-verb={verbOf(it())} data-opt="1" data-dock={p.dock ? '1' : undefined} aria-label={it().title}>
      <div class="in" style={{ background: it().kind === 'app' && !it().art.cover ? undefined : hueGrad(it()) }}>
        <Show when={it().kind === 'app' && !it().art.cover} fallback={
          <>
            <Show when={it().art.cover} fallback={<div class="br br-i" style={{ background: 'transparent' }}><span class="mg">{mono(it().title)}</span></div>}>
              <div class="art" style={{ 'background-image': `url("${artUrl(it().id, 'cover')}")` }} />
            </Show>
            <div class="shade" />
          </>
        }>
          <AppFace it={it()} />
        </Show>
        <Show when={p.badge} fallback={<Show when={running()}><span class="bd on">EM EXECUÇÃO</span></Show>}>
          <span class="bd">{p.badge}</span>
        </Show>
        <Show when={it().tags.includes('cloud') && it().state === 'installed'}><span class="ic"><Icon n="cloud" /></span></Show>
        <Show when={it().tags.includes('update') && it().state === 'installed'}><span class="ic"><Icon n="download" /></span></Show>
        {p.price}
        <Show when={!p.noProg && it().kind === 'game' && it().state === 'installed' && it().progress}>
          <div class="pb"><i style={{ width: `${it().progress}%` }} /></div>
        </Show>
        <Show when={it().state === 'installing' || it().state === 'updating'}>
          <div class="pb dl"><i style={{ width: `${it().install?.percent ?? 0}%` }} /></div>
        </Show>
      </div>
    </div>
  );
}

export function PriceTag(p: { it: Item }) {
  const it = () => p.it;
  return (
    <Show when={it().state === 'installed'} fallback={
      <Show when={it().state === 'installing'} fallback={<span class="pz">{effectivePrice(it()) ?? (it().caps.install ? 'Grátis' : '')}</span>}>
        <span class="pz">{Math.floor(it().install?.percent ?? 0)}%</span>
      </Show>
    }>
      <span class="pz ok"><Icon n="check" /> Instalado</span>
    </Show>
  );
}

export const isFree = (it: Item) => priceNum(effectivePrice(it)) === 0;

export function Empty(p: { icon?: string; title: string; text?: string; children?: JSX.Element }) {
  return (
    <div class="empty-state">
      <div class="ei"><Icon n={p.icon ?? 'box'} /></div>
      <h3>{p.title}</h3>
      <Show when={p.text}><p>{p.text}</p></Show>
      <div class="ea">{p.children}</div>
    </div>
  );
}

export { createMemo, For, Show };
