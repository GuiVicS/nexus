import { createEffect, createMemo, createSignal, For, on, onCleanup, onMount, Show } from 'solid-js';
import type { Item } from '@nexus/protocol';
import { api } from '../lib/api';
import { fail, isInstalled, isKid, prefs, S, setS, fx } from '../lib/state';
import { effectivePrice, fmtAgo, fmtBytes, fmtEta, fmtPlaytime, fmtSpeed, metaOf, money, norm, ratingLabel } from '../lib/format';
import { verbOf } from '../lib/actions';
import { Bar, Empty, Glyph, heroStyle, Icon, PriceTag, Tile } from './ui';

export const [stripItem, setStripItem] = createSignal<Item | null>(null);

// ---------- Início (Dock) ----------
export function dockItems(): Item[] {
  const lib = S.lib;
  const pinned = prefs().pins.map((id) => lib.find((i) => i.id === id)).filter((i): i is Item => !!i && (isInstalled(i) || i.state === 'installing'));
  if (pinned.length) return pinned;
  return [...lib]
    .filter((i) => isInstalled(i) || i.state === 'installing')
    .sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0) || norm(a.title).localeCompare(norm(b.title)))
    .slice(0, 12);
}

interface Card { ic: string; t: string; s: string; a: string; v?: string; bar?: number }

export function actsFor(it: Item): Card[] {
  const run = S.game?.itemId === it.id;
  if (it.state === 'installing' || it.state === 'updating') {
    const p = it.install;
    return [{ ic: 'download', t: `${it.state === 'updating' ? 'Atualizando' : 'Baixando'}… ${Math.floor(p?.percent ?? 0)}%`, s: p && p.bytesTotal ? `${fmtBytes(p.bytesDone)} de ${fmtBytes(p.bytesTotal)} · ${fmtSpeed(p.bps)} · falta ${fmtEta(p.etaSec)}` : 'Preparando o download…', a: `hub:${it.id}`, v: 'Ver', bar: p?.percent ?? 0 }];
  }
  if (it.kind === 'game') {
    const cards: Card[] = [
      { ic: 'play', t: run ? 'Retomar' : it.progress ? 'Continuar' : 'Começar', s: it.chapter || (it.lastPlayed ? `Último jogo ${fmtAgo(it.lastPlayed)}` : 'Do início'), a: run ? 'resume' : `launch:${it.id}`, v: 'Jogar', bar: it.progress ?? undefined },
      { ic: 'clock', t: 'Tempo de jogo', s: fmtPlaytime(it.playtimeSec), a: `hub:${it.id}:atv`, v: 'Ver' },
    ];
    if (it.achievements) cards.push({ ic: 'trophy', t: `${it.achievements.got} de ${it.achievements.total} conquistas`, s: 'Ver todas', a: `hub:${it.id}:ach`, v: 'Ver', bar: Math.round((it.achievements.got / Math.max(1, it.achievements.total)) * 100) });
    else cards.push({ ic: 'info', t: 'Detalhes', s: it.studio || it.subtitle, a: `hub:${it.id}`, v: 'Ver' });
    return cards;
  }
  return [
    { ic: 'play', t: run ? 'Retomar' : `Abrir ${it.title}`, s: it.category || 'App', a: run ? 'resume' : `app:${it.id}`, v: 'Abrir' },
    { ic: 'plus', t: prefs().pins.includes(it.id) ? 'Fixado no início' : 'Fixar no início', s: 'Gerenciar atalhos do início', a: `opts:${it.id}`, v: 'Opções' },
    { ic: 'info', t: 'Sobre o app', s: it.description || it.category || '', a: `hub:${it.id}`, v: 'Ver' },
  ];
}

function chipOf(it: Item) {
  if (S.game?.itemId === it.id) return 'EM EXECUÇÃO';
  if (it.state === 'installing') return 'BAIXANDO';
  if (it.state === 'updating') return 'ATUALIZANDO';
  if (it.kind === 'app') return 'APP';
  if (it.tags.includes('cloud')) return 'NA NUVEM';
  if (it.tags.includes('update')) return 'ATUALIZAÇÃO';
  if (it.tags.includes('classic')) return 'CLÁSSICO';
  return it.provider === 'steam' ? 'STEAM' : 'INSTALADO';
}

export function Home() {
  const items = createMemo(dockItems);
  const activeId = createMemo(() => (S.dockId === 'lib' ? 'lib' : items().some((i) => i.id === S.dockId) ? S.dockId! : (items()[0]?.id ?? 'lib')));
  const active = createMemo(() => items().find((i) => i.id === activeId()));
  const games = createMemo(() => S.lib.filter((i) => i.kind === 'game'));
  // Steam instalada mas sem conta: o Início convida a entrar (login pelo modo TV da Steam, com QR code)
  const steamNeedsLogin = () => !!S.steam?.installed && !S.steam?.persona && !isKid();
  onMount(() => {
    const poll = () => void api.steamAccount().then((a) => setS('steam', a)).catch(() => {});
    poll();
    const t = setInterval(() => steamNeedsLogin() && poll(), 8000);
    onCleanup(() => clearInterval(t));
  });
  let info!: HTMLDivElement;
  createEffect(on(activeId, () => {
    if (!info) return;
    info.classList.remove('swap');
    void info.offsetWidth;
    info.classList.add('swap');
  }));
  const dockAct = (it: Item) => (S.game?.itemId === it.id ? 'resume' : it.kind === 'app' ? `app:${it.id}` : it.state === 'installing' ? `hub:${it.id}` : `launch:${it.id}`);
  return (
    <>
      <div id="dockInfo" ref={info}>
        <Show when={active()} fallback={
          <>
            <span class="chip">BIBLIOTECA</span>
            <h1>Todos os jogos</h1>
            <p class="meta">{games().length} {games().length === 1 ? 'jogo' : 'jogos'}{S.lib.some((i) => i.tags.includes('cloud')) ? ` · ${S.lib.filter((i) => i.tags.includes('cloud')).length} na nuvem` : ''}</p>
            <div class="hint"><Glyph k="A" />Abrir biblioteca</div>
          </>
        }>
          {(it) => (
            <>
              <span class="chip">{chipOf(it())}</span>
              <h1 class={it().kind === 'game' && it().logo ? `lg-${it().logo}` : ''}>{it().title}</h1>
              <p class="meta">{metaOf(it())}</p>
              <Show when={it().kind === 'game' && it().progress && it().state === 'installed'}>
                <div class="dprog"><Bar pct={it().progress!} /><span>{it().progress}%{it().chapter ? ` · ${it().chapter}` : ''}</span></div>
              </Show>
              <div class="hint"><Glyph k="A" />{verbOf(it())}</div>
            </>
          )}
        </Show>
      </div>
      <div class="drow" id="dock">
        <For each={items()}>{(it) => <Tile it={it} dock a={dockAct(it)} />}</For>
        <div class="tl f all" data-k="lib" data-dock="lib" data-r="22" data-a="tab:biblioteca" data-verb="Abrir">
          <div class="in"><span><Icon n="grid" /></span><b>Biblioteca</b><small>{games().length} {games().length === 1 ? 'jogo' : 'jogos'}</small></div>
        </div>
      </div>
      <div id="acts">
        <Show when={active()}>
          {(it) => (
            <For each={actsFor(it())}>
              {(c, i) => (
                <div class="ac f" data-k={`ac${i()}`} data-r="22" data-a={c.a} data-verb={c.v ?? 'Abrir'} data-id={it().id}>
                  <div class="ci"><Icon n={c.ic} /></div>
                  <div class="tx"><b>{c.t}</b><small>{c.s}</small><Show when={c.bar !== undefined}><Bar pct={c.bar!} /></Show></div>
                </div>
              )}
            </For>
          )}
        </Show>
      </div>
      <Show when={items().length === 0}>
        <div class="home-empty">
          <Show
            when={steamNeedsLogin()}
            fallback={
              <Empty icon="box" title="Sua biblioteca está vazia" text="Instale um jogo pela Loja ou adicione programas do Linux em Ajustes → Apps e jogos.">
                <div class="btn pri f" data-autofocus data-r="p" data-pad="8" data-a="tabgo:loja" data-verb="Abrir"><Icon n="bag" /> Loja</div>
                <div class="btn f" data-r="p" data-pad="8" data-a="settings:apps" data-verb="Abrir"><Icon n="plus" /> Adicionar app</div>
              </Empty>
            }
          >
            <Empty icon="download" title="Entre na sua conta Steam" text="Seus jogos da Steam aparecem aqui. Escaneie o QR code com o app Steam no celular — não precisa digitar senha.">
              <div class="btn pri f" data-autofocus data-k="home-steamlogin" data-r="p" data-pad="8" data-a="steamopen" data-verb="Entrar"><Icon n="play" /> Entrar na Steam</div>
              <div class="btn f" data-r="p" data-pad="8" data-a="tabgo:loja" data-verb="Abrir"><Icon n="bag" /> Loja</div>
            </Empty>
          </Show>
        </div>
      </Show>
    </>
  );
}

// ---------- Biblioteca ----------
type Filter = [string, string, (i: Item) => boolean];
const filters = (): Filter[] => {
  const fav = prefs().favs;
  const all: Filter[] = [
    ['todos', 'Todos', () => true],
    ['recentes', 'Jogados recentemente', (i) => !!i.lastPlayed || i.playtimeSec > 0],
    ['inst', 'Instalados', (i) => isInstalled(i)],
    ['fora', 'Para instalar', (i) => i.state === 'not_installed' || i.state === 'installing'],
    ['fav', 'Favoritos', (i) => fav.includes(i.id)],
    ['nuvem', 'Na nuvem', (i) => i.tags.includes('cloud')],
    ['classico', 'Clássicos', (i) => i.tags.includes('classic')],
  ];
  const games = S.lib.filter((i) => i.kind === 'game' && !i.tags.includes('emu'));
  return all.filter(([k, , fn]) => k === 'todos' || k === 'fav' || games.some(fn));
};

export function Library() {
  // jogos de console (ROMs) ficam na aba Emuladores, por console
  const games = createMemo(() => S.lib.filter((i) => i.kind === 'game' && !i.tags.includes('emu')));
  const fl = createMemo(filters);
  const list = createMemo(() => {
    const f = fl().find((x) => x[0] === S.libFilter) ?? fl()[0];
    const l = games().filter(f[2]);
    return S.libFilter === 'recentes' ? [...l].sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0)) : [...l].sort((a, b) => norm(a.title).localeCompare(norm(b.title)));
  });
  const label = () => (fl().find((x) => x[0] === S.libFilter) ?? fl()[0])[1];
  createEffect(() => {
    if (!fl().some((x) => x[0] === S.libFilter)) setS('libFilter', 'todos');
  });
  return (
    <>
      <nav class="rail">
        <h4>Jogos</h4>
        <For each={fl()}>
          {([k, l, fn]) => (
            <div class={`ri f ${S.libFilter === k ? 'on' : ''}`} data-k={`rail-${k}`} data-group="lib" data-rail={k} data-r="22" data-a={`rail:${k}`} data-verb="Filtrar">
              {l}<b>{games().filter(fn).length}</b>
            </div>
          )}
        </For>
      </nav>
      <Strip fallbackTitle={label()} fallbackSub={`${list().length} ${list().length === 1 ? 'jogo' : 'jogos'}`} />
      <div class="sc" id="sc">
        <div class="gridt">
          <For each={list()} fallback={<p class="empty">Nada por aqui ainda.{isKid() ? '' : ' Que tal visitar a Loja?'}</p>}>
            {(g) => <Tile it={g} a={`open:${g.id}`} />}
          </For>
        </div>
      </div>
    </>
  );
}

export function Strip(p: { fallbackTitle: string; fallbackSub: string; wide?: boolean }) {
  return (
    <div class={`strip ${p.wide ? 'w' : ''}`} id="strip">
      <Show when={stripItem()} fallback={<><h2>{p.fallbackTitle}</h2><p>{p.fallbackSub}</p></>}>
        {(it) => (
          <>
            <h2>{it().title}</h2>
            <p>{metaOf(it())}{it().rating !== null && it().rating !== undefined ? ` · ${ratingLabel(it().rating)}` : ''}{it().sizeBytes && it().state !== 'installed' ? ` · ${fmtBytes(it().sizeBytes)}` : ''}</p>
          </>
        )}
      </Show>
    </div>
  );
}

// ---------- Mídia (apps) ----------
const CAT_ORDER = ['Vídeo', 'Música', 'Internet', 'Jogos', 'Ferramentas', 'Escritório', 'Gráficos', 'Desenvolvimento', 'Sistema', 'Apps'];
const CAT_TITLE: Record<string, string> = { Vídeo: 'Vídeo e streaming', Música: 'Música e podcasts', Internet: 'Internet', Jogos: 'Jogos Linux', Ferramentas: 'Ferramentas', Escritório: 'Escritório', Gráficos: 'Gráficos', Desenvolvimento: 'Desenvolvimento', Sistema: 'Sistema', Apps: 'Mais apps' };
const catOf = (i: Item) => {
  const c = i.category || 'Apps';
  if (/stream/i.test(c)) return 'Vídeo';
  if (/m[úu]sica|podcast/i.test(c)) return 'Música';
  return CAT_ORDER.includes(c) ? c : 'Apps';
};

export function Media() {
  const apps = createMemo(() => S.lib.filter((i) => i.kind === 'app' && (isInstalled(i) || i.state === 'installing')));
  const groups = createMemo(() => {
    const m = new Map<string, Item[]>();
    for (const a of [...apps()].sort((x, y) => norm(x.title).localeCompare(norm(y.title)))) {
      const c = catOf(a);
      m.set(c, [...(m.get(c) ?? []), a]);
    }
    return CAT_ORDER.filter((c) => m.has(c)).map((c) => [c, m.get(c)!] as const);
  });
  return (
    <>
      <Strip wide fallbackTitle="Mídia" fallbackSub="Streaming, música e apps instalados" />
      <div class="sc w" id="sc">
        <div class="gridt">
          <For each={groups()} fallback={
            <div class="empty-wrap">
              <Empty icon="tv" title="Nenhum app adicionado" text="Programas do Linux (navegador, Kodi, Minecraft…) aparecem aqui depois que você os adiciona em Ajustes → Apps e jogos.">
                <Show when={!isKid()}><div class="btn f" data-autofocus data-r="p" data-pad="8" data-a="settings:apps" data-verb="Abrir"><Icon n="plus" /> Adicionar app</div></Show>
              </Empty>
            </div>
          }>
            {([c, list]) => (
              <>
                <h3 class="sh">{CAT_TITLE[c]}</h3>
                <For each={list}>{(a) => <Tile it={a} a={`app:${a.id}`} />}</For>
              </>
            )}
          </For>
          <Show when={!isKid()}>
            <div class="tl f all" data-k="store-apps" data-r="22" data-a="tabgo:loja" data-verb="Abrir">
              <div class="in"><span><Icon n="bag" /></span><b>Loja</b><small>Apps e jogos</small></div>
            </div>
          </Show>
        </div>
      </div>
    </>
  );
}

// ---------- Loja ----------
const SHOP_CATS: [string, string][] = [
  ['conta', 'Na sua conta'],
  ['destaques', 'Mais vendidos'],
  ['ofertas', 'Ofertas'],
  ['gratis', 'Grátis'],
  ['linux', 'Nativos Linux'],
  ['lancamentos', 'Lançamentos'],
  ['jogos', 'Jogos'],
  ['classicos', 'Clássicos'],
  ['apps', 'Apps'],
];
const STORE_FEEDS = SHOP_CATS.map(([c]) => c).filter((c) => c !== 'conta');
const [shopLoading, setShopLoading] = createSignal(false);

export async function loadShop() {
  if (isKid()) return;
  setShopLoading(true);
  const w = api.wallet().then((x) => setS('wallet', x ? x.balance : null)).catch(() => {});
  await Promise.all(
    STORE_FEEDS.map((c) =>
      api.store(c).then((r) => setS('shop', c, r)).catch(() => {}),
    ),
  );
  await w;
  setShopLoading(false);
}

// Loja no jeito de uma loja de apps (Play Store): abas em pílulas no topo; "Para você" com banners grandes
// e prateleiras horizontais por categoria ("Ver tudo" abre a categoria em grade).
const SHELVES: [string, string][] = [
  ['conta', 'Na sua conta para instalar'],
  ['ofertas', 'Ofertas da semana'],
  ['gratis', 'Grátis para jogar'],
  ['destaques', 'Mais vendidos'],
  ['lancamentos', 'Lançamentos'],
  ['linux', 'Feitos para Linux'],
  ['classicos', 'Clássicos'],
  ['apps', 'Apps'],
];
const SHELF_MAX = 14;

function bannerChip(it: Item, cat: string) {
  if (it.offer?.pct) return `OFERTA ${it.offer.pct}`;
  if (cat === 'gratis' || effectivePrice(it) === 'Grátis') return 'GRÁTIS';
  if (cat === 'lancamentos') return 'NOVIDADE';
  return 'EM DESTAQUE';
}

export function Store() {
  onMount(() => void loadShop());
  const owned = createMemo(() => S.lib.filter((i) => i.state === 'not_installed' || (i.state === 'installing' && i.tags.includes('owned'))));
  const items = (c: string) => (c === 'conta' ? owned() : (S.shop[c] ?? []));
  const cats = createMemo(() => SHOP_CATS.filter(([c]) => items(c).length > 0));
  const home = () => S.shopCat === 'inicio';
  // categoria escolhida sumiu (ex.: "Na sua conta" sem jogos): volta para "Para você"
  createEffect(() => {
    if (shopLoading() || home()) return;
    if (!cats().some(([k]) => k === S.shopCat)) setS('shopCat', 'inicio');
  });
  // banners: os mais vendidos e as ofertas com imagem larga, sem repetir
  const banners = createMemo(() => {
    const seen = new Set<string>();
    const out: [Item, string][] = [];
    for (const c of ['destaques', 'ofertas', 'lancamentos']) {
      for (const it of items(c).slice(0, 4)) {
        if (!it.art.hero || seen.has(it.id)) continue;
        seen.add(it.id);
        out.push([it, c]);
      }
    }
    return out.slice(0, 8);
  });
  const shelves = createMemo(() => SHELVES.filter(([c]) => items(c).length > 0));
  const list = createMemo(() => items(S.shopCat));
  const title = () => SHOP_CATS.find(([k]) => k === S.shopCat)?.[1] ?? 'Loja';
  const loadingAll = () => shopLoading() && !banners().length && !shelves().length;
  return (
    <>
      <div class="stbar">
        <div class={`cp f ${home() ? 'on' : ''}`} data-k="shop-inicio" data-group="shop" data-rail="shop:inicio" data-r="p" data-pad="6" data-a="rail:shop:inicio" data-verb="Ver">Para você</div>
        <For each={cats()}>
          {([k, l]) => <div class={`cp f ${S.shopCat === k ? 'on' : ''}`} data-k={`shop-${k}`} data-group="shop" data-rail={`shop:${k}`} data-r="p" data-pad="6" data-a={`rail:shop:${k}`} data-verb="Ver">{l}</div>}
        </For>
        <Show when={S.wallet !== null}><span class="wallet">Carteira {money(S.wallet!)}</span></Show>
      </div>
      <Show when={home()} fallback={
        <>
          <Strip wide fallbackTitle={title()} fallbackSub={`${list().length} ${list().length === 1 ? 'item' : 'itens'}`} />
          <div class="sc w st" id="sc">
            <div class="gridt">
              <For each={list()}>{(it) => <Tile it={it} a={`open:${it.id}`} badge={it.offer?.pct} price={<PriceTag it={it} />} />}</For>
            </div>
          </div>
        </>
      }>
        <div class="sc w st home" id="sc">
          <Show when={!loadingAll()} fallback={
            <div class="empty-wrap">
              <Empty icon="bag" title={shopLoading() ? 'Carregando a loja…' : 'A loja está vazia'} text={shopLoading() ? 'Buscando os jogos da Steam. Pode levar alguns segundos.' : 'Não consegui carregar a loja da Steam. Confira a conexão com a internet.'}>
                <div class="btn f" data-autofocus data-r="p" data-pad="8" data-a="settings" data-verb="Abrir" onClick={() => setS('setCat', 'steam')}><Icon n="gear" /> Ajustes</div>
              </Empty>
            </div>
          }>
            <Show when={banners().length}>
              <div class="sbanners">
                <For each={banners()}>
                  {([it, c]) => (
                    <div class="sbn f" data-k={`bn-${it.id}`} data-id={it.id} data-r="28" data-a={`open:${it.id}`} data-verb="Ver" aria-label={it.title}>
                      <div class="in" style={heroStyle(it)}>
                        <div class="shade" />
                        <div class="tx">
                          <span class="chip">{bannerChip(it, c)}</span>
                          <b>{it.title}</b>
                          <small>{it.subtitle || metaOf(it)}</small>
                        </div>
                        <PriceTag it={it} />
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </Show>
            <For each={shelves()}>
              {([c, l]) => (
                <section class="shelf">
                  <div class="shh">
                    <h3>{l}</h3>
                    <Show when={items(c).length > SHELF_MAX || c !== 'conta'}>
                      <div class="cp f sm" data-k={`all-${c}`} data-r="p" data-pad="6" data-a={`rail:shop:${c}`} data-verb="Ver tudo">Ver tudo <Icon n="right" /></div>
                    </Show>
                  </div>
                  <div class="shr">
                    <For each={items(c).slice(0, SHELF_MAX)}>{(it) => <Tile it={it} a={`open:${it.id}`} badge={it.offer?.pct} price={<PriceTag it={it} />} />}</For>
                  </div>
                </section>
              )}
            </For>
            <Show when={!isKid() && !!S.emuReady}>
              <div class="spromo f" data-k="shop-roms" data-r="26" data-a="romsadd" data-verb="Abrir">
                <Icon n="plus" />
                <div><b>Tem jogos de console antigos?</b><small>Adicione ROMs pelo pendrive ou envie do celular. Eles aparecem na aba Emuladores.</small></div>
              </div>
            </Show>
          </Show>
        </div>
      </Show>
    </>
  );
}

export { fx };
