import { createEffect, createMemo, createSignal, For, onCleanup, onMount, Show } from 'solid-js';
import type { Item } from '@nexus/protocol';
import { sfx } from '@nexus/ui';
import { ACT, hubTab, setHubTab, verbOf } from '../lib/actions';
import { findItem, isInstalled, isKid, S } from '../lib/state';
import { effectivePrice, fmtAgo, fmtBytes, fmtEta, fmtPlaytime, fmtSpeed, metaOf, priceNum, ratingLabel } from '../lib/format';
import { Avatar, Bar, heroStyle, Icon } from './ui';
import { artUrl } from '../lib/api';

interface Cta { l: string; ic: string; a: string; verb: string; off?: boolean }

/** Títulos longos quebram em 2 linhas a 104 px e empurravam os botões para cima das abas: fonte menor. */
function titleSize(t: string): '' | 't-long' | 't-xlong' {
  return t.length > 26 ? 't-xlong' : t.length > 13 ? 't-long' : '';
}

export function ctaFor(it: Item): Cta {
  if (S.game?.itemId === it.id) return { l: 'Retomar', ic: 'play', a: 'resume', verb: 'Retomar' };
  if (it.state === 'installing') return { l: `Baixando… ${Math.floor(it.install?.percent ?? 0)}%`, ic: 'download', a: 'noop', verb: 'Aguarde', off: true };
  if (it.state === 'updating') return { l: `Atualizando… ${Math.floor(it.install?.percent ?? 0)}%`, ic: 'download', a: 'noop', verb: 'Aguarde', off: true };
  if (it.state === 'not_installed') {
    if (!it.caps.install && it.provider === 'steam') return { l: `Ver na Steam${it.price ? ` · ${effectivePrice(it)}` : ''}`, ic: 'bag', a: `steamstore:${it.id}`, verb: 'Ver na Steam' };
    if (!it.caps.install) return { l: 'Indisponível', ic: 'lock', a: 'noop', verb: 'Indisponível', off: true };
    const price = priceNum(effectivePrice(it));
    return price > 0 ? { l: `Comprar · ${effectivePrice(it)}`, ic: 'bag', a: `buy:${it.id}`, verb: 'Comprar' } : { l: 'Instalar', ic: 'download', a: `buy:${it.id}`, verb: 'Instalar' };
  }
  if (it.state === 'error') return { l: 'Indisponível', ic: 'lock', a: 'noop', verb: 'Ver', off: true };
  if (it.kind === 'app') return { l: 'Abrir', ic: 'play', a: `app:${it.id}`, verb: 'Abrir' };
  return { l: it.progress ? 'Continuar' : 'Jogar', ic: it.tags.includes('cloud') ? 'cloud' : 'play', a: `launch:${it.id}`, verb: verbOf(it) };
}

function chipOf(it: Item) {
  if (S.game?.itemId === it.id) return 'EM EXECUÇÃO';
  if (it.state === 'installing') return 'BAIXANDO';
  if (it.state === 'updating') return 'ATUALIZANDO';
  if (it.state === 'not_installed') return it.offer ? `OFERTA ${it.offer.pct}` : it.tags.includes('owned') ? 'NA SUA CONTA' : it.provider === 'steam' ? 'NA STEAM' : 'NA LOJA';
  if (it.kind === 'app') return 'APP';
  if (it.tags.includes('cloud')) return 'NA NUVEM';
  if (it.tags.includes('update')) return 'ATUALIZAÇÃO';
  if (it.tags.includes('classic')) return 'CLÁSSICO';
  return 'INSTALADO';
}

export function Hub(p: { id: string; oid: number }) {
  const [id, setId] = createSignal(p.id);
  const it = createMemo(() => findItem(id()));
  const tabs = createMemo<[string, string][]>(() => {
    const i = it();
    if (!i || i.kind !== 'game') return [];
    // jogos da Steam fora do aparelho: capturas de tela primeiro, como na página de uma loja de apps
    if (i.state === 'not_installed') return i.provider === 'steam' ? [['img', 'Imagens'], ['sobre', 'Sobre']] : [['sobre', 'Sobre']];
    const t: [string, string][] = [['atv', 'Atividades']];
    if (i.achievements) t.push(['ach', 'Conquistas']);
    if (!isKid() && S.friends.some((f) => f.online) && i.provider === 'demo') t.push(['ami', 'Amigos']);
    t.push(['sobre', 'Sobre']);
    return t;
  });
  const tab = () => (tabs().some((t) => t[0] === hubTab()) ? hubTab() : (tabs()[0]?.[0] ?? ''));
  const step = (e: Event) => {
    const d = (e as CustomEvent<number>).detail;
    const t = tabs();
    if (!t.length) return;
    const i = t.findIndex((x) => x[0] === tab());
    setHubTab(t[(i + d + t.length) % t.length][0]);
    sfx('tab');
  };
  const refresh = () => {
    const top = document.querySelector<HTMLElement>(`#ovs [data-oid="${p.oid}"]`);
    if (top?.dataset.hubid) setId(top.dataset.hubid);
  };
  onMount(() => {
    document.addEventListener('nexus:hub-step', step);
    document.addEventListener('nexus:hub-refresh', refresh);
  });
  onCleanup(() => {
    document.removeEventListener('nexus:hub-step', step);
    document.removeEventListener('nexus:hub-refresh', refresh);
  });
  createEffect(() => {
    // a tela de fundo acompanha o item mostrado
    const i = it();
    if (!i) return;
  });
  return (
    <Show when={it()} fallback={<div class="empty">Item indisponível.</div>}>
      {(i) => {
        const c = () => ctaFor(i());
        return (
          <>
            <div class="hbg" style={heroStyle(i())} />
            <div class="hsc" />
            <div class="hmain">
              <span class="chip">{chipOf(i())}</span>
              <h1 class={`${i().kind === 'game' && i().logo ? `lg-${i().logo}` : ''} ${titleSize(i().title)}`}>{i().title}</h1>
              <p class="meta">{metaOf(i())}{i().rating !== null && i().rating !== undefined ? ` · ${ratingLabel(i().rating)}` : ''}</p>
              <p class={`hdesc ${titleSize(i().title) ? 'short' : ''}`}>{i().description}</p>
              <div class="hacts">
                <div class={`btn pri f ${c().off ? 'off' : ''}`} data-k="cta" data-autofocus data-r="p" data-pad="8" data-a={c().a} data-verb={c().verb}><Icon n={c().ic} /> {c().l}</div>
                <Show when={i().state === 'installing' && i().caps.cancel}>
                  <div class="btn f" data-k="cancel" data-r="p" data-pad="8" data-a={`cancel:${i().id}`} data-verb="Cancelar"><Icon n="x" /> Cancelar</div>
                </Show>
                <Show when={!isKid() && (S.appJobs[i().id] || S.appUpd[i().id]?.canUpdate)}>
                  <Show when={S.appJobs[i().id]} fallback={
                    <div class="btn f" data-k="hupd" data-r="p" data-pad="8" data-a={`appupgrade:${i().id}`} data-verb="Atualizar"><Icon n="download" /> Atualizar{S.appUpd[i().id]?.available ? ` · ${S.appUpd[i().id].available}` : ''}</div>
                  }>
                    <div class="btn f off" data-k="hupd" data-r="p" data-pad="8" data-a="noop" data-verb="Aguarde"><span class="spin" /> {S.appJobs[i().id] === 'restore' ? 'Restaurando…' : 'Atualizando…'}</div>
                  </Show>
                </Show>
                <div class="btn f" data-k="hopt" data-r="p" data-pad="8" data-a={`opts:${i().id}`} data-verb="Opções"><Icon n="menu" /> Opções</div>
              </div>
              <Show when={!isKid() && S.appUpd[i().id]}>
                {(u) => <p class="hupd">{u().canUpdate ? `Nova versão ${u().available || ''} disponível${u().current ? ` (você tem a ${u().current})` : ''}. Se algo der errado, use Opções → Restaurar versão anterior.` : `Nova versão ${u().available} disponível. Este app vem do ${u().source}: o sistema instala a atualização.`}</p>}
              </Show>
            </div>
            <Show when={tabs().length}>
              <div class="htabs">
                <For each={tabs()}>{([t, l]) => <div class={`ht f ${t === tab() ? 'on' : ''}`} data-k={`ht-${t}`} data-r="p" data-pad="6" data-a={`hubtab:${t}`} data-verb="Ver">{l}</div>}</For>
              </div>
              <div class="hbody" id="hbody"><HubBody it={i()} tab={tab()} /></div>
            </Show>
          </>
        );
      }}
    </Show>
  );
}

function Card(p: { ic: string; t: string; s: string; a: string; v?: string; bar?: number }) {
  return (
    <div class="ac f" data-r="22" data-a={p.a} data-verb={p.v ?? 'Abrir'}>
      <div class="ci"><Icon n={p.ic} /></div>
      <div class="tx"><b>{p.t}</b><small>{p.s}</small><Show when={p.bar !== undefined}><Bar pct={p.bar!} /></Show></div>
    </div>
  );
}

function Fact(p: { k: string; v: string }) {
  return <div><small>{p.k}</small>{p.v}</div>;
}

/** Capturas de tela da loja (servidas pelo Core em /art/<id>/shotN); as que não existem somem. */
function Shots(p: { id: string }) {
  const [ok, setOk] = createSignal(0);
  const [bad, setBad] = createSignal(0);
  return (
    <div class="hshots">
      <For each={[0, 1, 2, 3] as const}>
        {(n) => <img src={artUrl(p.id, `shot${n}`)} alt="" loading="lazy" onLoad={() => setOk((x) => x + 1)} onError={(e) => {
          e.currentTarget.style.display = 'none';
          setBad((x) => x + 1);
        }} />}
      </For>
      <Show when={!ok()}><p class="pane-sub">{bad() >= 4 ? 'Este jogo não tem imagens na loja.' : 'Carregando imagens…'}</p></Show>
    </div>
  );
}

function HubBody(p: { it: Item; tab: string }) {
  const it = () => p.it;
  return (
    <>
      <Show when={p.tab === 'atv'}>
        <Show when={it().install && (it().state === 'installing' || it().state === 'updating')}>
          <Card ic="download" t={`${Math.floor(it().install!.percent)}% concluído`} s={`${fmtBytes(it().install!.bytesDone)} de ${fmtBytes(it().install!.bytesTotal)} · ${fmtSpeed(it().install!.bps)} · falta ${fmtEta(it().install!.etaSec)}`} a="noop" bar={it().install!.percent} />
        </Show>
        <Show when={it().state === 'installed' || it().state === 'running'}>
          <Card ic="play" t={S.game?.itemId === it().id ? 'Retomar' : it().progress ? 'Continuar' : 'Começar'} s={it().chapter || (it().lastPlayed ? `Último jogo ${fmtAgo(it().lastPlayed)}` : 'Do início')} a={S.game?.itemId === it().id ? 'resume' : `launch:${it().id}`} v="Jogar" bar={it().progress ?? undefined} />
        </Show>
        <Card ic="clock" t="Tempo de jogo" s={fmtPlaytime(it().playtimeSec)} a="noop" />
        <Show when={it().tags.includes('update')}>
          <Card ic="download" t="Atualização disponível" s="Será aplicada ao abrir o jogo" a={`launch:${it().id}`} v="Atualizar" />
        </Show>
        <Show when={it().tags.includes('cloud')}>
          <Card ic="cloud" t="Streaming em nuvem" s="Jogue sem baixar" a="noop" />
        </Show>
      </Show>
      <Show when={p.tab === 'ach' && it().achievements}>
        <Card ic="trophy" t={`${it().achievements!.got} de ${it().achievements!.total} conquistas`} s={`${Math.round((it().achievements!.got / Math.max(1, it().achievements!.total)) * 100)}% concluído`} a="noop" bar={Math.round((it().achievements!.got / Math.max(1, it().achievements!.total)) * 100)} />
        <div class="trs">
          <For each={Array.from({ length: 6 }, (_, i) => i)}>
            {(i) => {
              const got = Math.round((it().achievements!.got / Math.max(1, it().achievements!.total)) * 6);
              return <span class={i < got ? 'got' : ''}><Icon n={i < got ? 'star' : 'lock'} fill={i < got} /></span>;
            }}
          </For>
        </div>
      </Show>
      <Show when={p.tab === 'ami'}>
        <For each={S.friends.filter((f) => f.online)}>
          {(f) => (
            <div class="fr f" data-r="p" data-pad="8" data-a={`toast:Convite enviado para ${f.name}`} data-verb="Convidar">
              <Avatar p={{ avatar: f.name.toLowerCase(), hue: f.hue, name: f.name }} class="a" size={56} />
              <div>{f.name}<small>{f.status}</small></div>
            </div>
          )}
        </For>
      </Show>
      <Show when={p.tab === 'img'}>
        <Shots id={it().id} />
      </Show>
      <Show when={p.tab === 'sobre'}>
        <div class="facts">
          <Fact k="Estúdio" v={it().studio || '—'} />
          <Fact k="Gênero" v={it().subtitle || '—'} />
          <Fact k="Ano" v={it().year ? String(it().year) : '—'} />
          <Fact k="Classificação" v={it().rating !== null && it().rating !== undefined ? ratingLabel(it().rating) : '—'} />
          <Fact k="Tamanho" v={fmtBytes(it().sizeBytes)} />
          <Fact k="Origem" v={it().provider === 'steam' ? 'Steam' : it().provider === 'native' ? 'Linux' : 'Demonstração'} />
          <Fact k="Tempo de jogo" v={fmtPlaytime(it().playtimeSec)} />
          <Fact k="Último jogo" v={fmtAgo(it().lastPlayed)} />
        </div>
      </Show>
    </>
  );
}

export { isInstalled, ACT };
