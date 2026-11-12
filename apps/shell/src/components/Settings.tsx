import { createEffect, createMemo, createSignal, For, on, onCleanup, onMount, Show, type JSX } from 'solid-js';
import type { CustomApp, EmuStatus } from '@nexus/protocol';
import { api } from '../lib/api';
import { ACT, THEME_LABEL, cycHandlers, editProfile } from '../lib/actions';
import { askConfirm, askPin, effectsLevel, fail, guarded, isKid, me, prefs, S, setS, softwareGL, toast } from '../lib/state';
import { fmtBytes, ratingLabel } from '../lib/format';
import { Avatar, Icon, Switch } from './ui';
import { PadsSection } from './Pads';
import { AudioPanel, BtPanel, DisplayPanel, LocalePanel, NetPanel } from './System';
import { sfx } from '@nexus/ui';

interface Cat { id: string; label: string; icon: string; adult?: boolean }
const CATS: Cat[] = [
  { id: 'rede', label: 'Rede', icon: 'wifi', adult: true },
  { id: 'bt', label: 'Bluetooth', icon: 'bt', adult: true },
  { id: 'audio', label: 'Áudio', icon: 'vol' },
  { id: 'tela', label: 'Tela e vídeo', icon: 'tv', adult: true },
  { id: 'som', label: 'Aparência', icon: 'sun' },
  { id: 'acess', label: 'Acessibilidade', icon: 'access' },
  { id: 'ctrl', label: 'Controles', icon: 'pad' },
  { id: 'remote', label: 'Controle no celular', icon: 'tv', adult: true },
  { id: 'apps', label: 'Apps e jogos', icon: 'grid', adult: true },
  { id: 'emu', label: 'Emuladores', icon: 'pad', adult: true },
  { id: 'conta', label: 'Conta e perfis', icon: 'user', adult: true },
  { id: 'parental', label: 'Controle parental', icon: 'lock', adult: true },
  { id: 'steam', label: 'Steam', icon: 'download', adult: true },
  { id: 'hora', label: 'Idioma e hora', icon: 'clock', adult: true },
  { id: 'sis', label: 'Sistema', icon: 'gear', adult: true },
];

const cats = () => CATS.filter((c) => !c.adult || !isKid());

function SwRow(p: { k: 'sfx' | 'reduce' | 'hc' | 'subs' | 'vib' | 'invy'; label: string; desc: string }) {
  return (
    <div class="srow f" data-k={`s-${p.k}`} data-r="22" data-a={`tog:${p.k}`} data-verb="Alternar">
      <div><b>{p.label}</b><small>{p.desc}</small></div>
      <Switch on={prefs()[p.k]} />
    </div>
  );
}

function ActRow(p: { a: string; label: string; desc?: string; icon: string; danger?: boolean; verb?: string; k?: string }) {
  return (
    <div class="srow f" data-k={p.k ?? `s-${p.a}`} data-r="22" data-a={p.a} data-verb={p.verb ?? 'Abrir'} style={p.danger ? { color: 'var(--danger)' } : {}}>
      <div><b>{p.label}</b><Show when={p.desc}><small>{p.desc}</small></Show></div>
      <Icon n={p.icon} />
    </div>
  );
}

function Info(p: { label: string; desc: string }) {
  return <div class="srow info"><div><b>{p.label}</b><small>{p.desc}</small></div></div>;
}

function Pane(p: { title: string; children: JSX.Element }) {
  return (
    <>
      <h2>{p.title}</h2>
      {p.children}
    </>
  );
}

function PaneSom() {
  return (
    <Pane title="Aparência">
      <div class="srow f" data-k="s-theme" data-r="22" data-cyc="theme" data-a="cyc:theme" data-verb="Alterar">
        <div><b>Tema</b><small>Escuro, claro ou automático (segue o sistema)</small></div>
        <div class="cv"><Icon n="left" /><span>{THEME_LABEL[prefs().theme]}</span><Icon n="right" /></div>
      </div>
      <SwRow k="sfx" label="Sons de navegação" desc="Retorno sonoro ao mover o foco e selecionar" />
      <div class="srow f slider" data-k="s-vol" data-r="22" data-slider="vol" data-a="noop" data-verb="Ajustar">
        <b>Volume dos sons da interface</b><div class="tr"><i style={{ width: `${prefs().vol}%` }} /></div><span class="v">{prefs().vol}</span>
      </div>
      <Info label="Efeitos visuais" desc={softwareGL() ? 'Mínimos (sem aceleração de vídeo: a placa de vídeo não está em uso)' : ({ full: 'Completos', reduced: 'Reduzidos (aparelho de desempenho médio)', minimal: 'Mínimos (economiza recursos)' }[effectsLevel()])} />
    </Pane>
  );
}

function PaneAcess() {
  return (
    <Pane title="Acessibilidade">
      <SwRow k="reduce" label="Reduzir animações" desc="Remove transições e movimentos" />
      <SwRow k="hc" label="Foco em alto contraste" desc="Contorno amarelo grosso" />
      <SwRow k="subs" label="Legendas sempre ativas" desc="Quando o jogo ou app oferecer" />
    </Pane>
  );
}

function PaneCtrl() {
  return (
    <Pane title="Controles">
      <SwRow k="vib" label="Vibração" desc="Retorno tátil nos controles" />
      <SwRow k="invy" label="Inverter eixo Y" desc="Para câmeras em terceira pessoa" />
      <Info label="Mapa de botões" desc="A selecionar · B voltar · Y busca · ≡ opções · LB/RB seções · LT/RT página · Guia central" />
      <Show when={!isKid()}><ActRow a="remote" icon="tv" label="Usar o celular como controle" desc="Gamepad, mouse e teclado no celular" k="s-remote" /></Show>
      <Show when={!isKid()}><PadsSection /></Show>
    </Pane>
  );
}

function PaneRemote() {
  const load = () => void api.pairing().then((p) => setS('pairing', p)).catch(fail);
  onMount(load);
  onCleanup(() => setS('pairing', null));
  return (
    <Pane title="Controle no celular">
      <Show when={S.pairing} fallback={<Info label="Gerando código…" desc="Aguarde um instante" />}>
        {(p) => (
          <>
            <div class="qrcard">
              <div class="qr" innerHTML={p().qrSvg} role="img" aria-label="QR code para abrir o controle no celular" />
              <div class="qrinfo">
                <h3>Escaneie com a câmera do celular</h3>
                <ol>
                  <li>Conecte o celular na mesma rede Wi-Fi do console</li>
                  <li>Aponte a câmera para o QR code e abra o link</li>
                  <li>Escolha <b>Controle</b>, <b>Mouse</b> ou <b>Teclado</b></li>
                </ol>
                <code>{p().url.replace(/\?t=.*/, '')}</code>
                <small>{p().uinput ? 'Aparece como um controle Xbox 360 para os jogos e funciona como mouse e teclado do sistema.' : 'Modo compatível: o celular controla o Nexus. Para usar nos jogos, o sistema precisa liberar /dev/uinput.'}</small>
              </div>
            </div>
            <h3 class="sh2">Aparelhos conectados</h3>
            <For each={p().devices} fallback={<Info label="Nenhum aparelho conectado" desc="Ele aparece aqui assim que você abrir o link" />}>
              {(d) => (
                <div class="srow f" data-k={`rd-${d.id}`} data-r="22" data-a={`rdisc:${d.id}`} data-verb="Desconectar">
                  <div><b>{d.name}</b><small>{d.mode === 'gamepad' ? `Controle · jogador ${d.player}` : 'Mouse e teclado'}</small></div>
                  <Icon n="x" />
                </div>
              )}
            </For>
            <ActRow a="rrotate" icon="refresh" label="Gerar novo código" desc="Desconecta todos os aparelhos e invalida o QR atual" verb="Gerar" k="s-rrotate" />
          </>
        )}
      </Show>
    </Pane>
  );
}

function PaneApps() {
  const [list, setList] = createSignal<CustomApp[] | null>(null);
  const load = () => void api.customApps().then(setList).catch(fail);
  onMount(() => {
    load();
    document.addEventListener('nexus:apps-changed', load);
    onCleanup(() => document.removeEventListener('nexus:apps-changed', load));
  });
  return (
    <Pane title="Apps e jogos">
      <Info label="Só aparece o que você adicionar" desc="Jogos da Steam entram sozinhos. Programas do Linux (Minecraft, Firefox, Kodi…) entram por aqui, com nome, ícone e capas à sua escolha." />
      <ActRow a="appadd" icon="plus" label="Adicionar app ou jogo" desc="Escolha entre os programas instalados no sistema" verb="Adicionar" k="s-appadd" />
      <h3 class="sh2">Atualizações</h3>
      <For each={Object.values(S.appUpd)} fallback={<Info label="Tudo atualizado" desc="O Nexus procura novas versões dos apps a cada 6 horas e avisa nas notificações." />}>
        {(u) => (
          <div class="srow f" data-k={`up-${u.id}`} data-r="22" data-a={S.appJobs[u.id] ? 'noop' : u.canUpdate ? `appupgrade:${u.id}` : `hub:${u.id}`} data-verb={u.canUpdate ? 'Atualizar' : 'Ver'}>
            <div><b>{u.title}</b><small>{S.appJobs[u.id] ? (S.appJobs[u.id] === 'restore' ? 'Restaurando…' : 'Atualizando…') : `${u.current ? `${u.current} → ` : ''}${u.available || 'nova versão'} · ${u.canUpdate ? u.source : `${u.source}, atualizado pelo sistema`}`}</small></div>
            <Icon n={S.appJobs[u.id] ? 'refresh' : 'download'} />
          </div>
        )}
      </For>
      <ActRow a="appcheck" icon="refresh" label="Procurar atualizações agora" desc="Para voltar uma versão, abra o app e use Opções → Restaurar versão anterior" verb="Procurar" k="s-appcheck" />
      <h3 class="sh2">Adicionados ao Nexus</h3>
      <For each={list() ?? []} fallback={<Info label={list() === null ? 'Carregando…' : 'Nada adicionado ainda'} desc={list() === null ? '' : 'Use “Adicionar app ou jogo” acima'} />}>
        {(a) => (
          <div class="srow f" data-k={`ap-${a.id}`} data-r="22" data-a={`appedit:${a.id}`} data-verb="Editar">
            <div><b>{a.name}</b><small>{a.kind === 'game' ? 'Jogo' : 'App'}{a.hasCover || a.hasHero || a.hasIcon ? ' · personalizado' : ''}{a.available ? '' : ' · programa não encontrado'}</small></div>
            <Icon n="gear" />
          </div>
        )}
      </For>
    </Pane>
  );
}

function PaneConta() {
  const p = () => me()!;
  return (
    <Pane title="Conta e perfis">
      <div class="srow f acct" data-k="s-me" data-r="22" data-a={`profedit:${p().id}`} data-verb="Editar">
        <Avatar p={p()} size={84} />
        <div><b>{p().name}</b><small>{p().kind === 'kid' ? 'Perfil infantil' : 'Perfil'}{p().hasPin ? ' · protegido por PIN' : ''}</small></div>
        <Icon n="right" />
      </div>
      <ActRow a="switch" icon="user" label="Trocar de perfil" desc="Volta à escolha de perfis" />
      <h3 class="sh2">Todos os perfis</h3>
      <For each={S.profiles}>
        {(x) => (
          <div class="srow f" data-k={`pr-${x.id}`} data-r="22" data-a={`profedit:${x.id}`} data-verb="Editar">
            <Avatar p={x} size={64} />
            <div><b>{x.name}</b><small>{x.kind === 'kid' ? `Infantil · até ${ratingLabel(x.ageLimit)}${x.dailyLimitMin ? ` · ${x.dailyLimitMin} min/dia` : ''}` : 'Adulto'}{x.hasPin ? ' · PIN' : ''}</small></div>
            <Icon n="gear" />
          </div>
        )}
      </For>
      <ActRow a="profadd" icon="plus" label="Adicionar perfil" desc="Para outra pessoa ou para uma criança" />
    </Pane>
  );
}

function PaneParental() {
  const kids = () => S.profiles.filter((p) => p.kind === 'kid');
  return (
    <Pane title="Controle parental">
      <ActRow a="parentalpin" icon="lock" label="PIN dos responsáveis" desc={S.parental.hasPin ? 'Definido · toque para alterar' : 'Não definido · toque para criar'} verb="Alterar" k="s-ppin" />
      <Show when={S.parental.hasPin}><ActRow a="parentalrm" icon="trash" label="Remover PIN" desc="Desativa a proteção por PIN" danger verb="Remover" k="s-prm" /></Show>
      <h3 class="sh2">Perfis infantis</h3>
      <For each={kids()} fallback={<Info label="Nenhum perfil infantil" desc="Crie um em Conta e perfis → Adicionar perfil" />}>
        {(k) => (
          <>
            <div class="srow f" data-k={`kd-${k.id}`} data-r="22" data-a={`profedit:${k.id}`} data-verb="Editar">
              <Avatar p={k} size={64} />
              <div><b>{k.name}</b><small>Classificação até {ratingLabel(k.ageLimit)} · {k.dailyLimitMin ? `${k.dailyLimitMin} min por dia` : 'sem limite de tempo'}</small></div>
              <Icon n="gear" />
            </div>
            <ActRow a={`allowsheet:${k.id}`} icon="check" label={`Apps e jogos liberados para ${k.name}`} desc="Escolha o que a criança pode abrir" verb="Escolher" k={`al-${k.id}`} />
          </>
        )}
      </For>
    </Pane>
  );
}

function PaneSteam() {
  onMount(() => void api.steamAccount().then((a) => setS('steam', a)).catch(() => {}));
  return (
    <Pane title="Steam">
      <Show when={S.steam} fallback={<Info label="Verificando…" desc="" />}>
        {(s) => (
          <>
            <Info label="Cliente Steam" desc={s().installed ? 'Instalado' : S.hw && S.hw.architecture !== 'x86_64' ? 'Não disponível neste aparelho (a Steam para Linux só existe para PC x86_64)' : 'Não encontrado — instale a Steam para listar e abrir seus jogos'} />
            <Show when={s().installed}>
              <Info label="Conta" desc={s().persona ? `${s().persona}${s().account ? ` (${s().account})` : ''}` : 'Nenhuma conta conectada. Abra a Steam uma vez para entrar.'} />
              <Info label="Jogos na biblioteca" desc={String(s().libraryGames)} />
              <ActRow a="steamopen" icon="play" label={s().persona ? 'Abrir a Steam (modo TV)' : 'Entrar na Steam'} desc={s().persona ? 'Compras, amigos e ajustes da Steam no modo TV. Volte com o botão Guia.' : 'Abre o login da Steam no modo TV: escaneie o QR code com o app Steam no celular.'} verb={s().persona ? 'Abrir' : 'Entrar'} k="s-open" />
              <ActRow a="steamsync" icon="refresh" label="Sincronizar biblioteca" desc="Busca os jogos da conta que ainda não estão instalados" verb="Sincronizar" k="s-sync" />
              <ActRow a="steamkey" icon="lock" label="Chave da Web API" desc="Necessária para listar jogos não instalados (perfil Steam público)" verb="Informar" k="s-key" />
            </Show>
          </>
        )}
      </Show>
    </Pane>
  );
}

function PaneEmu() {
  const [st, setSt] = createSignal<EmuStatus | null>(null);
  const [busy, setBusy] = createSignal(false);
  const load = () => void api.emuStatus().then(setSt).catch(fail);
  onMount(() => {
    load();
    const h = (e: Event) => {
      const all = (e as CustomEvent<string>).detail === 'all';
      if (busy()) return;
      setBusy(true);
      toast('Baixando emuladores…', all ? 'Todos os consoles · pode levar alguns minutos' : 'Dos consoles que têm jogos', 'download');
      void api.emuFetchCores(all).then((r) => {
        toast(r.ok ? 'Emuladores prontos' : 'Alguns emuladores falharam', r.ok ? `${r.done.length} baixado(s)` : r.failed.map((f) => f.core).join(', '), r.ok ? 'check' : 'info');
        load();
      }).catch(fail).finally(() => setBusy(false));
    };
    document.addEventListener('nexus:emufetch', h);
    onCleanup(() => document.removeEventListener('nexus:emufetch', h));
  });
  const withGames = () => (st()?.systems ?? []).filter((s) => s.games > 0);
  return (
    <Pane title="Emuladores">
      <Show when={st()} fallback={<Info label="Verificando…" desc="" />}>
        {(s) => (
          <>
            <Info label="RetroArch" desc={s().retroarch ? 'Instalado · os jogos abrem em tela cheia com seus controles mapeados' : 'Não instalado — sudo nexus-installer install --emulation --yes'} />
            <Info label="Pasta dos jogos" desc={`${s().romsDir}/<console> — copie cada ROM para a pasta do console (veja LEIA-ME.txt)`} />
            <ActRow a="emufetch:mine" icon="download" label={busy() ? 'Baixando…' : 'Baixar emuladores dos meus jogos'} desc="Baixa os núcleos só dos consoles que já têm jogos (também acontece sozinho ao abrir o jogo)" verb="Baixar" k="s-emu-mine" />
            <ActRow a="emufetch:all" icon="download" label="Baixar todos os emuladores" desc={`${s().systems.length} consoles · usa algumas centenas de MB`} verb="Baixar" k="s-emu-all" />
            <h3 class="sh2">{withGames().length ? 'Consoles com jogos' : 'Consoles suportados'}</h3>
            <For each={withGames().length ? withGames() : s().systems}>
              {(x) => <Info label={`${x.name} · ${x.games} jogo${x.games === 1 ? '' : 's'}`} desc={`Pasta ${x.dir} · .${x.exts.join(' .')} · núcleo ${x.core} ${x.coreReady ? '✓ pronto' : '(baixa na 1ª vez)'}`} />}
            </For>
          </>
        )}
      </Show>
    </Pane>
  );
}

function PaneSis() {
  const hw = () => S.hw;
  return (
    <Pane title="Sistema">
      <ActRow a="fs" icon="tv" label="Tela cheia" desc="Alterna entre janela e tela cheia (tecla F)" />
      <Show when={hw()}>
        {(h) => (
          <>
            <Info label="Armazenamento" desc={`${fmtBytes(h().diskFreeBytes)} livres de ${fmtBytes(h().diskTotalBytes)}`} />
            <Info label="Sistema" desc={`${h().os} · ${h().architecture}`} />
            <Info label="Hardware" desc={`${Math.round(h().memoryMb / 1024)} GB de memória · GPU ${h().gpuVendor.toUpperCase()} · desempenho ${{ high: 'alto', mid: 'médio', low: 'básico' }[h().performanceTier] ?? h().performanceTier}`} />
          </>
        )}
      </Show>
      <ActRow a="switch" icon="user" label="Trocar perfil" desc="Volta à escolha de perfis" />
      <ActRow a="restart" icon="refresh" label="Reiniciar" desc="Reinicia o console" />
      <ActRow a="off" icon="power" label="Desligar" desc="Desliga o console" />
      <Info label="Nexus" desc={`Versão ${S.hello?.version ?? '—'}${S.hello?.demo ? ' · modo demonstração' : ''}`} />
    </Pane>
  );
}

export function Settings() {
  const list = createMemo(cats);
  createEffect(() => {
    if (!list().some((c) => c.id === S.setCat)) setS('setCat', 'som');
  });
  createEffect(on(() => S.setCat, () => {
    document.querySelector('#ovs .setpane')?.scrollTo({ top: 0 });
  }, { defer: true }));
  return (
    <>
      <div class="stitle">Ajustes</div>
      <nav class="setrail" data-zone="rail">
        <For each={list()}>
          {(c) => (
            <div class={`ri f ${S.setCat === c.id ? 'on' : ''}`} data-k={`sc-${c.id}`} data-group="setcat" data-rail={`cat:${c.id}`} data-r="22" data-a={`rail:cat:${c.id}`} data-verb="Abrir">
              <Icon n={c.icon} /> {c.label}
            </div>
          )}
        </For>
        <div class="ri f home" data-k="sc-home" data-r="22" data-a="home" data-verb="Voltar ao Início">
          <Icon n="home" /> Voltar ao Início
        </div>
      </nav>
      <div class="setpane" id="setpane" data-zone="pane">
        <Show when={S.setCat === 'rede' && !isKid()}><Pane title="Rede"><NetPanel /></Pane></Show>
        <Show when={S.setCat === 'bt' && !isKid()}><Pane title="Bluetooth"><BtPanel /></Pane></Show>
        <Show when={S.setCat === 'audio'}><Pane title="Áudio"><AudioPanel /></Pane></Show>
        <Show when={S.setCat === 'tela' && !isKid()}><Pane title="Tela e vídeo"><DisplayPanel /></Pane></Show>
        <Show when={S.setCat === 'hora' && !isKid()}><Pane title="Idioma e hora"><LocalePanel /></Pane></Show>
        <Show when={S.setCat === 'som'}><PaneSom /></Show>
        <Show when={S.setCat === 'acess'}><PaneAcess /></Show>
        <Show when={S.setCat === 'ctrl'}><PaneCtrl /></Show>
        <Show when={S.setCat === 'remote' && !isKid()}><PaneRemote /></Show>
        <Show when={S.setCat === 'apps' && !isKid()}><PaneApps /></Show>
        <Show when={S.setCat === 'emu' && !isKid()}><PaneEmu /></Show>
        <Show when={S.setCat === 'conta' && !isKid()}><PaneConta /></Show>
        <Show when={S.setCat === 'parental' && !isKid()}><PaneParental /></Show>
        <Show when={S.setCat === 'steam' && !isKid()}><PaneSteam /></Show>
        <Show when={S.setCat === 'sis' && !isKid()}><PaneSis /></Show>
      </div>
    </>
  );
}

export { ACT, cycHandlers, editProfile, guarded, sfx };
