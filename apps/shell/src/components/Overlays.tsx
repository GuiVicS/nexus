import { createEffect, createMemo, createSignal, For, onCleanup, onMount, Show } from 'solid-js';
import type { Item } from '@nexus/protocol';
import { api, artUrl } from '../lib/api';
import { ACT, THEME_LABEL, hubTab } from '../lib/actions';
import { textSink } from '../lib/actions';
import { closeOv, fail, findItem, guarded, hasGrant, isInstalled, isKid, me, popOv, prefs, S, setS, toast, replaceTop } from '../lib/state';
import { effectivePrice, money, norm, priceNum } from '../lib/format';
import { Avatar, Icon, Switch, Tile, appBg, Bar } from './ui';
import { sfx } from '@nexus/ui';
import { audio, loadAudio } from '../lib/audio';

export const [clock, setClock] = createSignal('--:--');
const tickClock = () => setClock(new Date().toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit' }));
tickClock();
setInterval(tickClock, 10000);

// ---------- Folhas (opções, compra, liberar para crianças) ----------
export function Sheet(p: { type: 'opts' | 'buy' | 'allow'; id?: string; profileId?: string; oid: number }) {
  return (
    <>
      <div class="veil" />
      <div class="sheet">
        <Show when={p.type === 'opts' && p.id}><OptsSheet id={p.id!} /></Show>
        <Show when={p.type === 'buy' && p.id}><BuySheet id={p.id!} /></Show>
        <Show when={p.type === 'allow' && p.profileId}><AllowSheet profileId={p.profileId!} /></Show>
      </div>
    </>
  );
}

function Row(p: { ic: string; label: string; a: string; danger?: boolean; small?: string }) {
  return (
    <div class={`mb f ${p.danger ? 'danger' : ''}`} data-r="20" data-pad="6" data-a={p.a} data-verb="Selecionar">
      <Icon n={p.ic} />{p.label}<Show when={p.small}><small>{p.small}</small></Show>
    </div>
  );
}

function OptsSheet(p: { id: string }) {
  const it = createMemo(() => findItem(p.id));
  const kids = createMemo(() => S.profiles.filter((x) => x.kind === 'kid'));
  return (
    <Show when={it()} fallback={<h2>Item indisponível</h2>}>
      {(i) => {
        const inHub = () => document.querySelector('#ovs .hub') !== null;
        const inst = () => isInstalled(i());
        return (
          <>
            <h2>{i().title}</h2>
            <p class="sub">{i().subtitle}</p>
            <Show when={i().state === 'not_installed' && i().caps.install}>
              <Row ic={priceNum(effectivePrice(i())) > 0 ? 'bag' : 'download'} label={priceNum(effectivePrice(i())) > 0 ? `Comprar · ${effectivePrice(i())}` : 'Instalar'} a={`buy:${i().id}`} />
            </Show>
            <Show when={inst() && !S.game}>
              <Row ic="play" label={i().kind === 'app' ? 'Abrir' : i().progress ? 'Continuar' : 'Jogar'} a={`launch:${i().id}`} />
            </Show>
            <Show when={!inHub()}><Row ic="info" label="Ver detalhes" a={`hub:${i().id}`} /></Show>
            <Show when={inst() && !isKid()}>
              <Row ic="heart" label={prefs().favs.includes(i().id) ? 'Remover dos favoritos' : 'Favoritar'} a={`fav:${i().id}`} />
              <Row ic="plus" label={prefs().pins.includes(i().id) ? 'Remover do início' : 'Fixar no início'} a={`pin:${i().id}`} />
            </Show>
            <Show when={inst() && !isKid() && kids().length > 0}>
              <Row ic="users" label="Liberar para perfis infantis" a={`allowsheet:${kids()[0].id}`} />
            </Show>
            <Show when={!isKid() && i().provider === 'native' && !S.appJobs[i().id]}>
              <Show when={S.appUpd[i().id]?.canUpdate}><Row ic="download" label={`Atualizar${S.appUpd[i().id].available ? ` para a versão ${S.appUpd[i().id].available}` : ''}`} a={`appupgrade:${i().id}`} /></Show>
              <Show when={S.appRestorable.includes(i().id) && S.game?.itemId !== i().id}><Row ic="refresh" label="Restaurar versão anterior" a={`apprestore:${i().id}`} /></Show>
              <Row ic="refresh" label="Procurar atualizações" a="appcheck" />
            </Show>
            <Show when={i().state === 'installing' && i().caps.cancel}><Row ic="x" label="Cancelar download" a={`cancel:${i().id}`} /></Show>
            <Show when={inst() && i().caps.uninstall && i().state !== 'running'}><Row ic="trash" label="Desinstalar" a={`uninstall:${i().id}`} danger /></Show>
          </>
        );
      }}
    </Show>
  );
}

function BuySheet(p: { id: string }) {
  const it = createMemo(() => findItem(p.id));
  return (
    <Show when={it()}>
      {(i) => (
        <>
          <h2>Confirmar compra</h2>
          <p class="sub">{i().title}</p>
          <div class="buy"><span>Preço</span><span><Show when={i().offer}><s style={{ color: 'var(--muted)' }}>{i().offer!.old}</s>{' '}</Show>{effectivePrice(i())}</span></div>
          <Show when={S.wallet !== null}><div class="buy"><span>Pagamento</span><span>Carteira · {money(S.wallet!)}</span></div></Show>
          <div class="buy"><span>Tamanho</span><span>{i().sizeBytes ? `${(i().sizeBytes! / 1e9).toFixed(1).replace('.', ',')} GB` : '—'}</span></div>
          <div class="buy tot"><span>Total</span><span>{effectivePrice(i())}</span></div>
          <div class="lacts" style={{ 'justify-content': 'flex-start' }}>
            <div class="btn pri f" data-autofocus data-r="p" data-pad="8" data-a={`confirm:${i().id}`} data-verb="Confirmar"><Icon n="bag" /> Confirmar</div>
            <div class="btn f" data-r="p" data-pad="8" data-a="back" data-verb="Cancelar"><Icon n="back" /> Cancelar</div>
          </div>
        </>
      )}
    </Show>
  );
}

function AllowSheet(p: { profileId: string }) {
  const [pid, setPid] = createSignal(p.profileId);
  const [allowed, setAllowed] = createSignal<string[]>([]);
  const kids = () => S.profiles.filter((x) => x.kind === 'kid');
  const kid = () => S.profiles.find((x) => x.id === pid());
  const load = () => void api.allowed(pid()).then(setAllowed).catch(fail);
  onMount(load);
  createEffect(() => {
    pid();
    load();
  });
  const list = createMemo(() => S.lib.filter((i) => isInstalled(i)).sort((a, b) => norm(a.title).localeCompare(norm(b.title))));
  const toggle = async (id: string) => {
    const on = !allowed().includes(id);
    const r = await guarded((a) => api.allow(pid(), id, on, a), 'Confirme para alterar o que a criança pode abrir');
    if (r) {
      setAllowed(r);
      sfx(on ? 'on' : 'off');
    }
  };
  const handler = (e: Event) => void toggle((e as CustomEvent<string>).detail);
  onMount(() => document.addEventListener('nexus:allow-toggle', handler));
  onCleanup(() => document.removeEventListener('nexus:allow-toggle', handler));
  return (
    <>
      <h2>Liberar para {kid()?.name ?? 'criança'}</h2>
      <p class="sub">Jogos com classificação dentro do limite já aparecem. Aqui você libera o restante.</p>
      <Show when={kids().length > 1}>
        <div class="chips" style={{ 'margin-bottom': '16px' }}>
          <For each={kids()}>{(k) => <div class={`cp f ${k.id === pid() ? 'on' : ''}`} data-r="p" data-pad="8" data-a={`allowkid:${k.id}`} data-verb="Ver" onClick={() => setPid(k.id)}>{k.name}</div>}</For>
        </div>
      </Show>
      <For each={list()} fallback={<p class="sub">Nenhum item instalado.</p>}>
        {(i) => (
          <div class="row2 f" data-r="20" data-a={`allowtoggle:${i.id}`} data-verb="Alternar">
            <div class="tx">{i.title}<small>{i.kind === 'app' ? 'App' : 'Jogo'}</small></div>
            <Switch on={allowed().includes(i.id)} />
          </div>
        )}
      </For>
    </>
  );
}

// ---------- Busca ----------
const KEYSL = 'abcdefghijklmnopqrstuvwxyz1234567890'.split('');

export function Search() {
  const [q, setQ] = createSignal('');
  const [res, setRes] = createSignal<Item[]>([]);
  const [loaded, setLoaded] = createSignal(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let seq = 0;
  const run = (text: string) => {
    clearTimeout(timer);
    timer = setTimeout(async () => {
      const my = ++seq;
      try {
        const r = await api.search(text);
        if (my === seq) {
          setRes(r);
          setLoaded(true);
        }
      } catch (e) {
        fail(e);
      }
    }, 140);
  };
  onMount(() => {
    textSink.enter = false;
    textSink.cur = (e) => {
      if (e.kind === 'char' && e.ch && q().length < 24 && /^[\p{L}\p{N} ]$/u.test(e.ch)) setQ(q() + e.ch);
      else if (e.kind === 'del') setQ(q().slice(0, -1));
      else if (e.kind === 'clear') setQ('');
      else return;
      run(q());
    };
    run('');
  });
  onCleanup(() => {
    textSink.cur = null;
    clearTimeout(timer);
  });
  const shown = createMemo(() => (q().trim() ? res() : [...res()].sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0)).slice(0, 10)));
  return (
    <>
      <div class="stitle">Buscar</div>
      <div class="sq"><span id="sqt"><Show when={q()} fallback={<span class="ph2">Jogos, apps…</span>}>{q().replace(/ /g, ' ')}</Show></span><i class="caret" /></div>
      <div class="kbd">
        <For each={KEYSL}>{(k) => <div class="key f" data-r="18" data-pad="6" data-a={`key:${k}`} data-verb="Digitar">{k}</div>}</For>
        <div class="key w3 f" data-r="18" data-pad="6" data-a="key:space" data-verb="Digitar">espaço</div>
        <div class="key f" data-r="18" data-pad="6" data-a="key:del" data-verb="Apagar"><Icon n="left" /></div>
        <div class="key w2 f" data-r="18" data-pad="6" data-a="key:clear" data-verb="Limpar">limpar</div>
      </div>
      <div class="sr" id="sres">
        <h3 class="sh" style={{ margin: '0 0 18px 6px' }}>{q().trim() ? `${shown().length} resultado${shown().length === 1 ? '' : 's'}` : 'Recentes'}</h3>
        <div class="gridt">
          <For each={shown()} fallback={<p class="empty">{loaded() ? 'Nada encontrado.' : 'Buscando…'}</p>}>
            {(it) => <Tile it={it} a={it.kind === 'app' && isInstalled(it) ? `app:${it.id}` : `open:${it.id}`} />}
          </For>
        </div>
      </div>
    </>
  );
}

// ---------- Central de controle ----------
export function Guide() {
  onMount(() => void loadAudio());
  const game = () => (S.game ? findItem(S.game.itemId) : undefined);
  const prof = () => me()!;
  const sw = (k: 'sfx' | 'reduce', l: string) => (
    <div class="row2 f" data-r="20" data-a={`tog:${k}`} data-verb="Alternar"><div class="tx">{l}</div><Switch on={prefs()[k]} /></div>
  );
  const row = (a: string, ic: string, t: string, s?: string, v?: string) => (
    <div class="row2 f" data-r="20" data-a={a} data-verb={v ?? 'Selecionar'}><Icon n={ic} /><div><div class="tx">{t}</div><Show when={s}><small>{s}</small></Show></div></div>
  );
  return (
    <>
      <Show when={S.game && S.frame}>
        <div class="gframe" style={{ 'background-image': `url("${artUrl('frame', 'frame')}?t=${S.frame}")` }} />
      </Show>
      <div class="veil" />
      <aside class="panel l">
        <div class="ph">
          <Avatar p={prof()} />
          <div><b>{prof().name}</b><small>{isKid() ? (S.session?.remainingMin !== null && S.session?.remainingMin !== undefined ? `Perfil infantil · ${S.session.remainingMin} min restantes` : 'Perfil infantil') : S.hello?.demo ? 'Online · modo demonstração' : 'Online'}</small></div>
          <span class="t">{clock()}</span>
        </div>
        <Show when={S.game}>
          <h3>Agora</h3>
          {row('resume', 'play', `Retomar ${S.game!.title}`, S.game!.state === 'closing' ? 'Fechando…' : game()?.kind === 'app' ? 'Aberto' : 'Em execução', 'Retomar')}
          {row('exitgame', 'home', 'Ir ao Início', `${game()?.kind === 'app' ? 'O app' : 'O jogo'} continua aberto em segundo plano`, 'Abrir')}
          {row('closegame', 'power', `Fechar ${game()?.kind === 'app' ? 'app' : 'jogo'}`, game()?.kind === 'app' ? 'Volta ao Nexus' : 'O progresso é salvo pelo próprio jogo', 'Fechar')}
        </Show>
        <h3>Atalhos</h3>
        {/* volume de verdade do sistema (jogos e apps), não só dos sons da interface */}
        <Show when={audio()?.available} fallback={<div class="row2 info"><Icon n="vol" /><div><div class="tx">Som</div><small>{audio() ? 'Indisponível' : 'Carregando…'}</small></div></div>}>
          <div class="row2 f slider" data-k="g-vol" data-r="20" data-slider="sysvol" data-a="audio:mute" data-verb="Mudo"><Icon n={audio()!.muted ? 'x' : 'vol'} /><div class="tr"><i style={{ width: `${audio()!.muted ? 0 : audio()!.volume}%` }} /></div><span class="v">{audio()!.muted ? 'mudo' : audio()!.volume}</span></div>
        </Show>
        {sw('sfx', 'Sons')}
        {sw('reduce', 'Reduzir animações')}
        <div class="row2 f" data-r="20" data-cyc="theme" data-a="cyc:theme" data-verb="Alterar"><Icon n={document.documentElement.dataset.theme === 'light' ? 'sun' : 'moon'} /><div><div class="tx">Tema</div><small>{THEME_LABEL[prefs().theme]}</small></div></div>
        <Show when={!isKid()}>{row('remote', 'pad', 'Controle no celular', 'Escaneie o QR code', 'Abrir')}</Show>
        <h3>Perfil e energia</h3>
        {row('switch', 'user', 'Trocar perfil')}
        <Show when={!isKid()}>
          {row('restart', 'refresh', 'Reiniciar')}
          {row('off', 'power', 'Desligar')}
        </Show>
      </aside>
    </>
  );
}

// ---------- Notificações ----------
export function Notifs() {
  return (
    <>
      <div class="veil" />
      <aside class="panel r">
        <h3 style={{ 'margin-top': '0' }}>Notificações</h3>
        <For each={S.notifs} fallback={<p class="sub" style={{ padding: '10px 6px', color: 'var(--muted)' }}>Nada novo por aqui.</p>}>
          {(n) => (
            <div class="row2 f" data-r="20" data-a={n.action ? `nt:${n.action}` : 'back'} data-verb="Abrir">
              <Icon n={n.kind === 'alert' ? 'info' : n.kind} />
              <div><div class="tx">{n.title}</div><small>{n.body}</small></div>
            </div>
          )}
        </For>
        <Show when={S.notifs.length > 0}>
          <div class="row2 f" data-r="20" data-a="clearnotifs" data-verb="Limpar"><Icon n="trash" /><div><div class="tx">Limpar tudo</div></div></div>
        </Show>
      </aside>
    </>
  );
}

// ---------- Execução ----------
export function Launch(p: { id: string }) {
  const it = createMemo(() => findItem(p.id));
  const running = () => S.game?.state === 'running';
  const closing = () => S.game?.state === 'closing';
  return (
    <Show when={it()}>
      {(i) => (
        <>
          <div class="lbg" style={i().art.hero ? { 'background-image': `url("${artUrl(i().id, 'hero')}")` } : { background: appBg(i()), opacity: '.35' }} />
          <div class="lc">
            <Show when={i().kind === 'app'}><div class="appico"><Tile it={i()} noProg /></div></Show>
            <h2 class={i().kind === 'game' && i().logo ? `lg-${i().logo}` : ''}>{i().title}</h2>
            <Show when={running()} fallback={
              <>
                <div class="bar indet"><i /></div>
                <p><span class="spin" />{closing() ? 'Fechando…' : 'Abrindo…'}</p>
                <Show when={S.launchHint}><p class="lnote">{S.launchHint}</p></Show>
                <div class="lacts">
                  <div class="btn f" data-autofocus data-r="p" data-pad="8" data-a="closegame" data-verb="Cancelar"><Icon n="x" /> Cancelar</div>
                </div>
              </>
            }>
              <p><span class="dot" />{i().kind === 'app' ? 'Aberto' : 'Em execução'}{closing() ? ' · fechando…' : ''}</p>
              <div class="lacts">
                <div class="btn pri f" data-autofocus data-r="p" data-pad="8" data-a="resume" data-verb="Retomar"><Icon n="play" /> Retomar</div>
                <div class="btn f" data-r="p" data-pad="8" data-a="exitgame" data-verb="Voltar"><Icon n="home" /> Início</div>
                <div class="btn f" data-r="p" data-pad="8" data-a="closegame" data-verb="Fechar"><Icon n="power" /> Fechar {i().kind === 'app' ? 'app' : 'jogo'}</div>
              </div>
              <Show when={closing()}>
                <div class="lacts"><div class="btn f" data-r="p" data-pad="8" data-a="forceclose" data-verb="Forçar"><Icon n="x" /> Forçar encerramento</div></div>
              </Show>
              <p class="lnote">O {i().kind === 'app' ? 'app' : 'jogo'} fica em primeiro plano. Menu rápido: botão Guia do controle (ou Select+Start), F1 ou tecla Super (⊞). A mesma tecla volta {i().kind === 'app' ? 'ao app' : 'ao jogo'}.</p>
            </Show>
          </div>
        </>
      )}
    </Show>
  );
}

// ---------- Falha: o Core não responde ----------
export function Fault() {
  const [secs, setSecs] = createSignal(0);
  const t = setInterval(() => setSecs((x) => x + 1), 1000);
  onCleanup(() => clearInterval(t));
  return (
    <div class="fault">
      <div class="fic"><Icon n="info" /></div>
      <h1>O Nexus encontrou um problema</h1>
      <p>A parte principal do Nexus parou de responder. Seus jogos e dados estão seguros.</p>
      <p class="fst" role="status">{S.faultMsg || `Tentando reconectar sozinho… (${secs() + 12} s)`}</p>
      <div class="lacts">
        <div class="btn pri f" data-autofocus data-r="p" data-pad="8" data-a="fault:retry" data-verb="Tentar"><Icon n="refresh" /> Tentar de novo</div>
        <div class="btn f" data-r="p" data-pad="8" data-a="fault:session" data-verb="Reiniciar"><Icon n="home" /> Reiniciar o Nexus</div>
        <div class="btn f" data-r="p" data-pad="8" data-a="fault:reboot" data-verb="Reiniciar"><Icon n="power" /> Reiniciar o aparelho</div>
      </div>
      <small class="fhelp">Se nada resolver: no teclado, Ctrl+Alt+F3 abre o modo texto para suporte.</small>
    </div>
  );
}

// ---------- Fim do tempo (perfil infantil) ----------
export function TimeUp() {
  const [unlocked, setUnlocked] = createSignal(hasGrant());
  const call = async () => {
    const ok = await guarded(async () => true, 'Peça a um responsável para digitar o PIN');
    if (ok) setUnlocked(true);
  };
  const extend = async (min: number) => {
    const p = me();
    if (!p) return;
    const r = await guarded((a) => api.extend(p.id, min, a));
    if (r) {
      toast(`+${min} minutos liberados`, 'Bom jogo!', 'clock');
      closeOv(topOvId());
    }
  };
  const topOvId = () => Number(document.querySelector<HTMLElement>('#ovs [data-kind="timeup"]')?.dataset.oid);
  onMount(() => {
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      if (a === 'call') void call();
      else if (a.startsWith('extend:')) void extend(Number(a.slice(7)));
    };
    document.addEventListener('nexus:timeup', h);
    onCleanup(() => document.removeEventListener('nexus:timeup', h));
  });
  return (
    <div class="timeup">
      <div class="moon"><Icon n="moon" /></div>
      <h1>Hora de descansar!</h1>
      <p>O tempo de jogo de hoje acabou. Amanhã tem mais!</p>
      <div class="lacts">
        <Show when={!unlocked()} fallback={
          <>
            <div class="btn pri f" data-autofocus data-r="p" data-pad="8" data-a="timeup:extend:15" data-verb="Liberar"><Icon n="clock" /> +15 min</div>
            <div class="btn f" data-r="p" data-pad="8" data-a="timeup:extend:30" data-verb="Liberar"><Icon n="clock" /> +30 min</div>
            <div class="btn f" data-r="p" data-pad="8" data-a="switch" data-verb="Trocar"><Icon n="user" /> Trocar de perfil</div>
          </>
        }>
          <div class="btn pri f" data-autofocus data-r="p" data-pad="8" data-a="timeup:call" data-verb="Chamar"><Icon n="lock" /> Chamar um responsável</div>
        </Show>
      </div>
    </div>
  );
}

// ---------- Confirmação ----------
export function Confirm(p: { title: string; sub?: string; yes?: string; no?: string; danger?: boolean; oid: number }) {
  return (
    <>
      <div class="veil" />
      <div class="sheet narrow">
        <h2>{p.title}</h2>
        <Show when={p.sub}><p class="sub">{p.sub}</p></Show>
        <div class="lacts" style={{ 'justify-content': 'flex-start', 'margin-top': '10px' }}>
          <div class="btn f" data-autofocus data-r="p" data-pad="8" data-a={`yes:${p.oid}`} data-verb="Confirmar" style={p.danger ? { color: 'var(--danger)' } : {}}>{p.yes ?? 'Confirmar'}</div>
          <div class="btn f" data-r="p" data-pad="8" data-a={`no:${p.oid}`} data-verb="Cancelar">{p.no ?? 'Cancelar'}</div>
        </div>
      </div>
    </>
  );
}

export { Bar, hubTab, popOv, replaceTop, setS };
