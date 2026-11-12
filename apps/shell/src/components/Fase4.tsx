// Telas de sistema da Fase 4: energia (A5), CEC (B5), perfis/PIN (B12), aparência e acessibilidade (B13/B14),
// apps instalados (B17), pastas e emuladores de ROMs (B18) e privacidade (B19). Tudo passa pelo Core.
import { createSignal, For, onCleanup, onMount, Show } from 'solid-js';
import type { AppDetail, CecStatus, EmuStatus, PrivacyStatus, UsageReport } from '@nexus/protocol';
import { sfx } from '@nexus/ui';
import { api } from '../lib/api';
import { ACT, cycHandlers, powerAction } from '../lib/actions';
import { askConfirm, askPin, askText, closeOv, fail, guarded, prefs, pushOv, S, setPref, setS, toast } from '../lib/state';
import { fmtBytes } from '../lib/format';
import { Icon, Switch } from './ui';

function Line(p: { k: string; label: string; desc?: string; warn?: boolean; code?: string }) {
  return (
    <div class="srow f" data-k={p.k} data-r="22" data-a="noop" data-verb="Ok" style={p.warn ? { color: 'var(--danger)' } : {}}>
      <div><b>{p.label}</b><Show when={p.desc}><small>{p.desc}</small></Show><Show when={p.code}><code class="cmd">{p.code}</code></Show></div>
    </div>
  );
}
function Act(p: { k: string; a: string; label: string; desc?: string; icon: string; verb?: string; danger?: boolean; off?: boolean }) {
  return (
    <div class={`srow f ${p.off ? 'off' : ''}`} data-k={p.k} data-r="22" data-a={p.off ? 'noop' : p.a} data-verb={p.verb ?? 'Abrir'} style={p.danger ? { color: 'var(--danger)' } : {}}>
      <div><b>{p.label}</b><Show when={p.desc}><small>{p.desc}</small></Show></div>
      <Icon n={p.icon} />
    </div>
  );
}

/** Ouve `data-a="f4:<prefixo>:…"`. */
function useF4(prefix: string, handler: (rest: string) => void) {
  onMount(() => {
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      if (a.startsWith(`${prefix}:`)) handler(a.slice(prefix.length + 1));
    };
    document.addEventListener('nexus:f4', h);
    onCleanup(() => document.removeEventListener('nexus:f4', h));
  });
}

const hm = (s: number) => (s >= 3600 ? `${Math.floor(s / 3600)} h ${Math.round((s % 3600) / 60)} min` : `${Math.round(s / 60)} min`);

// ---------------------------------------------------------------- A5: tela de energia
export function PowerScreen(p: { oid: number }) {
  const hasProfile = () => !!S.session;
  useF4('pw', (a) => {
    void (async () => {
      if (a === 'suspend') {
        closeOv(p.oid);
        try {
          const r = await api.power('suspend');
          toast(r.simulated ? 'Suspendendo (simulado)' : 'Suspendendo…', 'Aperte o botão de energia para acordar', 'power');
        } catch (e) {
          fail(e);
        }
      } else if (a === 'restart' || a === 'off') {
        closeOv(p.oid);
        await powerAction(a);
      } else if (a === 'switch') {
        closeOv(p.oid);
        ACT.switch('');
      } else if (a === 'back') closeOv(p.oid);
    })();
  });
  return (
    <>
      <div class="veil" />
      <div class="sheet narrow energia">
        <h2>Energia</h2>
        <p class="pane-sub">{S.game ? `${S.game.title} será fechado se você reiniciar ou desligar.` : 'O que você quer fazer com o aparelho?'}</p>
        <Act k="pw-suspend" a="f4:pw:suspend" icon="power" label="Suspender" desc="Economiza energia e volta rápido" verb="Suspender" />
        <Act k="pw-restart" a="f4:pw:restart" icon="refresh" label="Reiniciar" desc="Desliga e liga de novo" verb="Reiniciar" />
        <Act k="pw-off" a="f4:pw:off" icon="power" label="Desligar" desc="Desliga o aparelho (e a TV, se o CEC estiver ligado)" verb="Desligar" danger />
        <Show when={hasProfile()}><Act k="pw-switch" a="f4:pw:switch" icon="user" label="Trocar de perfil" desc="Volta à escolha de perfis" verb="Trocar" /></Show>
        <div data-autofocus class="srow f" data-k="pw-back" data-r="22" data-a="f4:pw:back" data-verb="Voltar"><div><b>Voltar</b></div><Icon n="left" /></div>
      </div>
    </>
  );
}

// ---------------------------------------------------------------- B5: CEC
export function CecPanel() {
  const [st, setSt] = createSignal<CecStatus | null>(null);
  const [err, setErr] = createSignal('');
  const [busy, setBusy] = createSignal(false);
  const load = () => api.cecStatus().then((s) => { setSt(s); setErr(''); }).catch(() => setErr('Não foi possível consultar o CEC'));
  onMount(() => void load());
  useF4('cec', (a) => {
    void (async () => {
      if (a === 'reload') return void load();
      if (a === 'wake' || a === 'sleep') {
        const s = st();
        if (!s) return;
        try {
          setSt(await api.cecSet({ [a]: !s[a] }));
          sfx('on');
        } catch (e) { fail(e); }
      } else if (a === 'on' || a === 'off') {
        setBusy(true);
        try {
          await api.cecTv(a === 'on');
          toast(a === 'on' ? 'Pedi à TV para ligar' : 'Pedi à TV para desligar', '', 'check');
        } catch (e) { fail(e); } finally { setBusy(false); }
      }
    })();
  });
  return (
    <Show when={st()} fallback={err() ? <Act k="cec-reload" a="f4:cec:reload" icon="refresh" label="Tentar de novo" desc={err()} verb="Tentar" /> : <Line k="cec-loading" label="Consultando…" />}>
      {(s) => {
        const ok = () => s().installed && s().adapters.length > 0;
        return (
          <>
            <Line k="cec-intro" label="Ligar e desligar a TV junto" desc="O HDMI-CEC deixa o Nexus ligar a TV, trocar a entrada para ele e desligá-la ao desligar o aparelho. A TV também pode controlar o Nexus com o controle remoto dela." />
            <Line k="cec-adapter" label="Adaptador CEC" desc={!s().installed ? 'O programa cec-client não está instalado.' : s().adapters.length ? s().adapters.map((a) => `${a.kind || 'adaptador'} (${a.port})`).join(' · ') : 'Nenhum adaptador encontrado. O Raspberry Pi tem CEC na porta HDMI; em PC é preciso um adaptador USB-CEC (Pulse-Eight).'} warn={!ok()} />
            <Show when={!s().installed}><Line k="cec-install" label="Como instalar" desc="No terminal do aparelho:" code={s().installCmd} /></Show>
            <div class={`srow f ${ok() ? '' : 'off'}`} data-k="cec-wake" data-r="22" data-a={ok() ? 'f4:cec:wake' : 'noop'} data-verb="Alternar">
              <div><b>Ligar a TV ao abrir o Nexus</b><small>Manda “ligar” e “usar esta entrada” quando o Nexus inicia</small></div><Switch on={s().wake} />
            </div>
            <div class={`srow f ${ok() ? '' : 'off'}`} data-k="cec-sleep" data-r="22" data-a={ok() ? 'f4:cec:sleep' : 'noop'} data-verb="Alternar">
              <div><b>Desligar a TV ao desligar ou suspender</b><small>A TV vai para espera junto com o aparelho</small></div><Switch on={s().sleep} />
            </div>
            <Act k="cec-on" a="f4:cec:on" icon="tv" label="Ligar a TV agora" desc="Teste" verb="Ligar" off={!ok() || busy()} />
            <Act k="cec-off" a="f4:cec:off" icon="power" label="Desligar a TV agora" desc="Teste" verb="Desligar" off={!ok() || busy()} />
            <Line k="cec-note" label="Sem a TV responder?" desc="Procure nos ajustes da TV o nome do fabricante para o CEC (Anynet+, Bravia Sync, Simplink, EasyLink…) e deixe ligado." />
          </>
        );
      }}
    </Show>
  );
}

// ---------------------------------------------------------------- B19: privacidade
export function PrivacyPanel() {
  const [st, setSt] = createSignal<PrivacyStatus | null>(null);
  const [err, setErr] = createSignal('');
  const load = () => api.privacyStatus().then((s) => { setSt(s); setErr(''); }).catch(() => setErr('Não foi possível consultar'));
  onMount(() => void load());
  useF4('priv', (a) => {
    void (async () => {
      if (a === 'reload') return void load();
      const t = { recent: ['Limpar os jogos recentes?', 'Eles somem de “Jogados recentemente”. O tempo jogado continua.'], playtime: ['Apagar o tempo de jogo?', 'Zera o tempo jogado e os recentes deste perfil.'], cache: ['Apagar as imagens baixadas?', 'Capas e ícones voltam a ser baixados quando precisar.'] }[a];
      if (!t) return;
      if (!(await askConfirm({ title: t[0], sub: t[1], yes: 'Limpar', danger: true }))) return;
      const r = await guarded((auth) => api.privacyClear(a as 'recent', auth), 'Confirme para limpar');
      if (r) {
        toast('Limpo', `${r.cleared} item(ns)`, 'trash');
        void load();
      }
    })();
  });
  return (
    <Show when={st()} fallback={err() ? <Act k="priv-reload" a="f4:priv:reload" icon="refresh" label="Tentar de novo" desc={err()} verb="Tentar" /> : <Line k="priv-loading" label="Consultando…" />}>
      {(s) => (
        <>
          <Line k="priv-tele" label="Telemetria: nenhuma" desc="O Nexus não envia dados sobre o que você joga, assiste ou busca, nem sobre o aparelho. Não existe conta do Nexus." />
          <h3 class="sh2">O que vai pela rede</h3>
          <For each={s().connections}>
            {(c, i) => <Line k={`priv-net-${i()}`} label={`${c.name} · ${c.on ? 'em uso' : 'desligado'}`} desc={`${c.what} (${c.when})`} />}
          </For>
          <h3 class="sh2">O que fica guardado aqui</h3>
          <For each={s().stores}>{(c, i) => <Line k={`priv-store-${i()}`} label={`${c.name}${c.count ? ` · ${c.count}` : ''}`} desc={c.detail} />}</For>
          <h3 class="sh2">Limpar</h3>
          <Act k="priv-recent" a="f4:priv:recent" icon="trash" label="Limpar jogos recentes" desc="Tira os jogos da lista de recentes deste perfil" verb="Limpar" />
          <Act k="priv-playtime" a="f4:priv:playtime" icon="trash" label="Apagar o tempo de jogo" desc="Zera o tempo jogado deste perfil" verb="Apagar" danger />
          <Act k="priv-cache" a="f4:priv:cache" icon="trash" label="Apagar imagens baixadas" desc="Capas e ícones guardados neste aparelho" verb="Apagar" />
          <Line k="priv-search" label="Histórico de busca" desc="O Nexus não guarda o que você busca. Nada para limpar." />
        </>
      )}
    </Show>
  );
}

// ---------------------------------------------------------------- B13/B14: aparência e acessibilidade
const TS_LABEL = ['Normal', 'Grande', 'Muito grande'];
const BG_LABEL: Record<string, string> = { art: 'Imagem do jogo', solid: 'Cor lisa', blue: 'Azul', green: 'Verde', purple: 'Roxo', orange: 'Laranja' };
const BG_ORDER = ['art', 'solid', 'blue', 'green', 'purple', 'orange'];

function TextSizeRow() {
  onMount(() => {
    cycHandlers.set('ts', (d) => setPref({ textSize: (((prefs().textSize ?? 0) + d + 3) % 3) }));
    onCleanup(() => cycHandlers.delete('ts'));
  });
  return (
    <div class="srow f" data-k="s-ts" data-r="22" data-cyc="ts" data-a="cyc:ts" data-verb="Alterar">
      <div><b>Tamanho do texto</b><small>Aumenta as letras das listas, ajustes e avisos para ler de longe</small></div>
      <div class="cv"><Icon n="left" /><span>{TS_LABEL[prefs().textSize ?? 0]}</span><Icon n="right" /></div>
    </div>
  );
}

export function AppearanceExtras() {
  onMount(() => {
    cycHandlers.set('bg', (d) => {
      const i = BG_ORDER.indexOf(prefs().bg ?? 'art');
      setPref({ bg: BG_ORDER[(i + d + BG_ORDER.length) % BG_ORDER.length] });
    });
    onCleanup(() => cycHandlers.delete('bg'));
  });
  return (
    <>
      <TextSizeRow />
      <div class="srow f" data-k="s-bg" data-r="22" data-cyc="bg" data-a="cyc:bg" data-verb="Alterar">
        <div><b>Fundo</b><small>Imagem do jogo em foco, cor lisa ou um degradê</small></div>
        <div class="cv"><Icon n="left" /><span>{BG_LABEL[prefs().bg ?? 'art']}</span><Icon n="right" /></div>
      </div>
    </>
  );
}

export function AccessExtras() {
  return (
    <>
      <TextSizeRow />
      <div class="srow f" data-k="s-sr" data-r="22" data-a="f4:sr:toggle" data-verb="Alternar">
        <div><b>Leitor de tela</b><small>Fala em voz alta o item em foco. Precisa de uma voz no sistema (pacote espeak-ng ou speech-dispatcher).</small></div>
        <Switch on={!!prefs().screenReader} />
      </div>
      <SrToggle />
    </>
  );
}
function SrToggle() {
  useF4('sr', () => {
    const on = !prefs().screenReader;
    setPref({ screenReader: on });
    sfx(on ? 'on' : 'off');
    if (on) speak('Leitor de tela ligado');
  });
  return null;
}

/** Fala um texto (leitor de tela). Sem voz instalada, o navegador simplesmente não fala. */
export function speak(t: string) {
  try {
    const ss = window.speechSynthesis;
    if (!ss) return;
    ss.cancel();
    const u = new SpeechSynthesisUtterance(t.replace(/\s+/g, ' ').trim().slice(0, 200));
    u.lang = 'pt-BR';
    ss.speak(u);
  } catch { /* sem voz */ }
}

// ---------------------------------------------------------------- B12: PIN esquecido e relatório de uso
export function ParentalExtras() {
  const [rec, setRec] = createSignal<boolean | null>(null);
  const [rep, setRep] = createSignal<UsageReport | null>(null);
  const load = () => void api.recoveryStatus2().then((r) => setRec(r.has)).catch(() => {});
  onMount(load);
  const showCode = (code: string, title = 'Código de recuperação') =>
    askConfirm({ title, sub: `Anote: ${code}. Se esquecer o PIN dos responsáveis, use este código em “Esqueci o PIN”. Ele aparece só agora; guarde em lugar seguro.`, yes: 'Anotei', no: 'Fechar' });
  useF4('par', (a) => {
    void (async () => {
      if (a === 'newcode') {
        const r = await guarded((auth) => api.recoveryNew(auth), 'Confirme para gerar um código novo');
        if (r) {
          setRec(true);
          await showCode(r.code, 'Novo código de recuperação');
        }
      } else if (a === 'forgot') {
        const code = await askText({ title: 'Código de recuperação', sub: 'Digite o código que você anotou (XXXX-XXXX-XXXX)', max: 14, kind: 'key' });
        if (!code) return;
        const pin = await askPin({ title: 'Novo PIN dos responsáveis', sub: 'De 4 a 6 números', confirm: true });
        if (pin === null) return;
        try {
          const r = await api.recoverPin(code, pin);
          setS('grant', null);
          toast('PIN novo salvo', '', 'lock');
          await showCode(r.code, 'Novo código de recuperação');
        } catch (e) {
          sfx('deny');
          const m = (e as { data?: { remaining?: number }; message?: string });
          toast('Código incorreto', m.data?.remaining ? `${m.data.remaining} tentativa(s) restante(s)` : (m.message ?? ''), 'lock');
        }
      } else if (a.startsWith('rep:')) {
        const r = await guarded((auth) => api.usageReport(a.slice(4), auth), 'Confirme para ver o relatório');
        if (r) setRep(r);
      } else if (a === 'repclose') setRep(null);
    })().catch(fail);
  });
  return (
    <>
      <h3 class="sh2">PIN esquecido</h3>
      <Show when={S.parental.hasPin} fallback={<Line k="par-nopin" label="Sem PIN definido" desc="Crie o PIN acima. Junto dele vem um código para recuperar o PIN se você esquecer." />}>
        <Act k="par-forgot" a="f4:par:forgot" icon="lock" label="Esqueci o PIN" desc="Define um PIN novo com o código de recuperação" verb="Recuperar" />
        <Act k="par-newcode" a="f4:par:newcode" icon="refresh" label={rec() ? 'Gerar um código novo' : 'Criar código de recuperação'} desc="O código antigo deixa de valer" verb="Gerar" />
      </Show>
      <h3 class="sh2">Relatório de uso</h3>
      <For each={S.profiles}>{(p) => <Act k={`par-rep-${p.id}`} a={`f4:par:rep:${p.id}`} icon="clock" label={`Uso de ${p.name}`} desc="Tempo por dia e o que mais jogou" verb="Ver" />}</For>
      <Show when={rep()}>
        {(r) => (
          <>
            <Line k="rep-head" label={`${r().name} · hoje ${hm(r().todaySeconds)}${r().limitMin ? ` de ${r().limitMin} min` : ''}`} desc={r().days.length ? 'Últimos dias com uso' : 'Sem uso registrado ainda'} />
            <For each={r().days}>{(d, i) => <Line k={`rep-day-${i()}`} label={`${d.day.split('-').reverse().join('/')} · ${hm(d.seconds)}`} />}</For>
            <For each={r().top} fallback={<Line k="rep-notop" label="Nenhum jogo com tempo registrado" />}>
              {(t, i) => <Line k={`rep-top-${i()}`} label={`${t.title} · ${hm(t.seconds)}`} desc={t.lastPlayed ? `Último uso: ${new Date(t.lastPlayed * 1000).toLocaleDateString('pt-BR')}` : undefined} />}
            </For>
            <Act k="rep-close" a="f4:par:repclose" icon="x" label="Fechar relatório" verb="Fechar" />
          </>
        )}
      </Show>
    </>
  );
}

// ---------------------------------------------------------------- B17: apps instalados (lista com tamanho)
export function InstalledApps() {
  const [list, setList] = createSignal<AppDetail[] | null>(null);
  const load = () => void api.appDetails().then(setList).catch(() => setList([]));
  onMount(() => {
    load();
    document.addEventListener('nexus:apps-changed', load);
    onCleanup(() => document.removeEventListener('nexus:apps-changed', load));
  });
  return (
    <>
      <h3 class="sh2">Espaço e origem</h3>
      <For each={list() ?? []} fallback={<Line k="inst-empty" label={list() === null ? 'Calculando o tamanho…' : 'Nenhum app para mostrar'} />}>
        {(a) => (
          <div class="srow f" data-k={`in-${a.id}`} data-r="22" data-a={`appedit:${a.id}`} data-verb="Gerenciar">
            <div><b>{a.name}{a.hidden ? ' · oculto' : ''}</b><small>{a.sizeBytes != null ? fmtBytes(a.sizeBytes) : 'tamanho desconhecido'} · {a.origin}{a.category ? ` · ${a.category}` : ''}</small></div>
            <Icon n="gear" />
          </div>
        )}
      </For>
    </>
  );
}

/** Adicionar um programa por arquivo (.jar, AppImage, .sh, executável), escolhido no navegador de arquivos. */
export async function addProgramFlow() {
  const path = (await pushOv<string>('filepick', { title: 'Escolher o programa (.jar, AppImage, .sh)', mode: 'program' })) as string | undefined;
  if (!path) return;
  let info;
  try {
    info = await api.programInfo(path);
  } catch (e) {
    return void fail(e);
  }
  if (info.needsJava && !info.javaOk) {
    await askConfirm({ title: 'Falta o Java', sub: `${info.hint ?? ''} Depois volte e escolha o arquivo de novo.`, yes: 'Entendi', no: 'Fechar' });
    return;
  }
  const game = await askConfirm({ title: `Adicionar ${info.name}?`, sub: `${info.kind}. Ele entra na Biblioteca como jogo e abre em tela cheia.`, yes: 'Adicionar como jogo', no: 'Adicionar como app' });
  const r = await guarded((a) => api.addProgram({ path, kind: game ? 'game' : 'app', ...a }), 'Confirme para adicionar o programa');
  if (!r) return;
  sfx('on');
  toast('Adicionado ao Nexus', r.name, 'check');
  document.dispatchEvent(new CustomEvent('nexus:apps-changed'));
  void pushOv('appedit', { id: r.id, fresh: true });
}

// ---------------------------------------------------------------- B18: pastas extras e emulador por console
export function RomsExtras(p: { status: EmuStatus; onStatus: (s: EmuStatus) => void }) {
  const [busy, setBusy] = createSignal(false);
  const apply = (r: { status: EmuStatus }) => p.onStatus(r.status);
  onMount(() => {
    const opts = (p.status.systems ?? []).filter((s) => (s.options?.length ?? 0) > 1);
    for (const s of opts) {
      cycHandlers.set(`core:${s.dir}`, (d) => {
        const cur = p.status.systems.find((x) => x.dir === s.dir);
        if (!cur?.options) return;
        const i = cur.options.indexOf(cur.core);
        const next = cur.options[(i + d + cur.options.length) % cur.options.length];
        void api.emuSetCore(s.dir, next).then(apply).catch(fail);
      });
    }
    onCleanup(() => opts.forEach((s) => cycHandlers.delete(`core:${s.dir}`)));
  });
  useF4('rom', (a) => {
    void (async () => {
      if (a === 'add') {
        const path = (await pushOv<string>('filepick', { title: 'Escolher a pasta de ROMs', mode: 'folder' })) as string | undefined;
        if (!path) return;
        setBusy(true);
        try {
          apply(await api.romAddDir(path));
          toast('Pasta adicionada', path, 'check');
        } catch (e) { fail(e); } finally { setBusy(false); }
      } else if (a === 'rescan') {
        setBusy(true);
        try {
          apply(await api.romRescan());
          toast('Jogos reescaneados', '', 'refresh');
        } catch (e) { fail(e); } finally { setBusy(false); }
      } else if (a.startsWith('rm:')) {
        const path = a.slice(3);
        if (!(await askConfirm({ title: 'Tirar esta pasta?', sub: `${path}\nOs arquivos não são apagados; só deixam de aparecer nos Emuladores.`, yes: 'Tirar', danger: true }))) return;
        try { apply(await api.romRemoveDir(path)); } catch (e) { fail(e); }
      }
    })();
  });
  const multi = () => (p.status.systems ?? []).filter((s) => (s.options?.length ?? 0) > 1);
  return (
    <>
      <h3 class="sh2">Pastas de ROMs</h3>
      <Line k="rom-main" label="Pasta da TV" desc={`${p.status.romsDir} (sempre usada, vence cópias repetidas)`} />
      <For each={p.status.extraDirs ?? []}>
        {(d, i) => <Act k={`rom-extra-${i()}`} a={`f4:rom:rm:${d}`} icon="trash" label={d} desc="Pasta extra · toque para tirar da lista" verb="Tirar" />}
      </For>
      <Act k="rom-add" a="f4:rom:add" icon="plus" label="Adicionar pasta (pendrive, outro disco)" desc="Use a mesma organização: uma pasta por console. Entra depois da pasta da TV e antes do Google Drive." verb="Escolher" off={busy()} />
      <Act k="rom-rescan" a="f4:rom:rescan" icon="refresh" label={busy() ? 'Procurando…' : 'Procurar jogos de novo'} desc="Reescaneia todas as pastas" verb="Procurar" off={busy()} />
      <Show when={multi().length}>
        <h3 class="sh2">Emulador (núcleo) de cada console</h3>
        <For each={multi()}>
          {(s) => (
            <div class="srow f" data-k={`core-${s.dir}`} data-r="22" data-cyc={`core:${s.dir}`} data-a={`cyc:core:${s.dir}`} data-verb="Trocar">
              <div><b>{s.name}</b><small>{s.core === s.defaultCore ? 'Padrão do Nexus' : `Padrão seria ${s.defaultCore}`}{s.coreReady ? ' · pronto' : ' · baixa na 1ª vez'}</small></div>
              <div class="cv"><Icon n="left" /><span>{s.core}</span><Icon n="right" /></div>
            </div>
          )}
        </For>
      </Show>
    </>
  );
}
