// Telas de sistema da Fase 3: Atualizações (B11), Sobre e diagnóstico (B16), Acesso por SSH (passo a passo + liga/desliga)
// e Recuperação (B15, atrás do PIN). Tudo passa pelo Core (updates.rs, about.rs, sshd.rs, recovery.rs).
import { createSignal, For, onCleanup, onMount, Show } from 'solid-js';
import type { AboutStatus, SshStatus, SupportReport, UpdateStatus } from '@nexus/protocol';
import { sfx } from '@nexus/ui';
import { api, client } from '../lib/api';
import { askConfirm, fail, fx, guarded, requireParental, S, setS, toast } from '../lib/state';
import { fmtBytes } from '../lib/format';
import { Icon, Switch } from './ui';

/** Linha só de leitura, mas focável: assim dá para ler tudo andando com ↑/↓ (a página rola com o foco). */
function Line(p: { k: string; label: string; desc?: string; step?: boolean; warn?: boolean; code?: string }) {
  return (
    <div class={`srow f ${p.step ? 'step' : ''}`} data-k={p.k} data-r="22" data-a="noop" data-verb="Ok" style={p.warn ? { color: 'var(--danger)' } : {}}>
      <div>
        <b>{p.label}</b>
        <Show when={p.desc}><small>{p.desc}</small></Show>
        <Show when={p.code}><code class="cmd">{p.code}</code></Show>
      </div>
    </div>
  );
}

function Act(p: { k: string; a: string; label: string; desc?: string; icon: string; verb?: string; off?: boolean; danger?: boolean }) {
  return (
    <div class={`srow f ${p.off ? 'off' : ''}`} data-k={p.k} data-r="22" data-a={p.off ? 'noop' : p.a} data-verb={p.verb ?? 'Abrir'} style={p.danger ? { color: 'var(--danger)' } : {}}>
      <div><b>{p.label}</b><Show when={p.desc}><small>{p.desc}</small></Show></div>
      <Icon n={p.icon} />
    </div>
  );
}

const sys = (a: string) => document.dispatchEvent(new CustomEvent('nexus:sys', { detail: a }));
export const sysAction = sys;

/** Ouve os cliques das linhas (`data-a="sys:<prefixo>:…"`) de um painel. */
function useSys(prefix: string, handler: (rest: string) => void) {
  onMount(() => {
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      if (a.startsWith(`${prefix}:`)) handler(a.slice(prefix.length + 1));
    };
    document.addEventListener('nexus:sys', h);
    onCleanup(() => document.removeEventListener('nexus:sys', h));
  });
}

const when = (ts: number) => new Date(ts * 1000).toLocaleString('pt-BR', { day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit' });
const dur = (s: number) => {
  const d = Math.floor(s / 86400), h = Math.floor((s % 86400) / 3600), m = Math.floor((s % 3600) / 60);
  return d ? `${d} d ${h} h` : h ? `${h} h ${m} min` : `${m} min`;
};
const gb = (mb: number) => `${(mb / 1024).toFixed(1).replace('.', ',')} GB`;

// ---------------------------------------------------------------- Atualizações (B11)
export function UpdatesPanel() {
  const [st, setSt] = createSignal<UpdateStatus | null>(null);
  const [err, setErr] = createSignal('');
  const [checking, setChecking] = createSignal(false);
  let timer: ReturnType<typeof setInterval> | undefined;
  const load = () =>
    api.updateStatus().then((s) => {
      setSt(s);
      setErr('');
    }).catch(() => setErr('Não foi possível consultar as atualizações'));

  const install = async (kind: 'nexus' | 'system') => {
    const s = st();
    if (!s) return;
    const ok = await askConfirm(
      kind === 'nexus'
        ? { title: `Atualizar o Nexus para ${s.nexus.latest}?`, sub: 'A interface continua funcionando durante a instalação. No fim, reinicie a sessão para usar a versão nova.', yes: 'Atualizar' }
        : { title: `Atualizar o sistema (${s.system.upgradable} pacotes)?`, sub: 'Pode demorar alguns minutos e pedir para reiniciar o aparelho no fim. Não desligue durante a atualização.', yes: 'Atualizar' },
    );
    if (!ok) return;
    const r = await guarded((a) => api.updateInstall({ kind, ...a }), 'Confirme para atualizar');
    if (r) {
      toast(kind === 'nexus' ? 'Atualizando o Nexus…' : 'Atualizando o sistema…', 'Você pode continuar usando o aparelho', 'download');
      void load();
    }
  };
  const revert = async () => {
    const to = st()?.nexus.revertTo;
    if (!to) return;
    const ok = await askConfirm({ title: `Voltar para o Nexus ${to}?`, sub: 'A versão atual será trocada pela anterior. Seus perfis, jogos e ajustes continuam.', yes: 'Voltar', danger: true });
    if (!ok) return;
    const r = await guarded((a) => api.updateRevert(a), 'Confirme para voltar a versão');
    if (r) {
      toast('Voltando a versão do Nexus…', to, 'refresh');
      void load();
    }
  };

  useSys('up', (a) => {
    if (a === 'check') {
      setChecking(true);
      void guarded((x) => api.updateCheck(x), 'Confirme para procurar atualizações').then((s) => {
        if (s) setSt(s);
      }).finally(() => setChecking(false));
    } else if (a === 'nexus') void install('nexus');
    else if (a === 'system') void install('system');
    else if (a === 'revert') void revert();
    else if (a === 'restart')
      void askConfirm({ title: 'Reiniciar a sessão do Nexus?', sub: 'A tela some por alguns segundos e volta com a versão nova.', yes: 'Reiniciar' }).then((ok) => {
        if (ok) void guarded((x) => api.recoveryRun({ action: 'restart_session', ...x })).catch(fail);
      });
    else if (a === 'reload') void load();
  });
  onMount(() => {
    void load();
    timer = setInterval(() => {
      if (st()?.job.running) void load();
    }, 1500);
    const off = client.on('update.done', (p: { ok: boolean; message: string }) => {
      toast(p.ok ? 'Atualização concluída' : 'A atualização não deu certo', p.message, p.ok ? 'check' : 'info');
      void load();
    });
    onCleanup(() => {
      clearInterval(timer);
      off();
    });
  });

  return (
    <Show when={st()} fallback={err() ? <Act k="up-reload" a="sys:up:reload" icon="refresh" label="Tentar de novo" desc={err()} verb="Tentar" /> : <Line k="up-loading" label="Carregando…" />}>
      {(s) => {
        const busy = () => s().job.running || s().gameRunning;
        return (
          <>
            <Line k="up-nexus" label={`Nexus ${s().nexus.version}`} desc={!s().nexus.packaged ? 'Versão de desenvolvimento (não instalada por pacote): atualizações pela tela ficam desligadas' : s().nexus.hasUpdate ? `Nova versão disponível: ${s().nexus.latest}` : s().nexus.latest ? 'Você está com a versão mais nova' : 'Ainda não procurei versões novas'} />
            <Line k="up-sys" label={s().system.os} desc={`Kernel ${s().system.kernel} · ${s().system.upgradable ? `${s().system.upgradable} pacote(s) com atualização${s().system.packages.length ? `: ${s().system.packages.join(', ')}${s().system.upgradable > s().system.packages.length ? '…' : ''}` : ''}` : 'sistema em dia (pela última verificação)'}`} />
            <Line k="up-checked" label="Última verificação" desc={s().checkedAt ? when(s().checkedAt!) : 'Nunca'} />
            <Show when={s().checkError}><Line k="up-err" label="Não consegui verificar" desc={s().checkError!} warn /></Show>
            <Show when={s().gameRunning}><Line k="up-game" label="Há um jogo aberto" desc="Feche o jogo para atualizar: o Nexus nunca atualiza durante uma partida." warn /></Show>
            <Show when={s().nexus.needsRestart}><Act k="up-restart" a="sys:up:restart" icon="refresh" label={`Reiniciar a sessão para usar o Nexus ${s().nexus.version}`} desc="A versão nova já está instalada" verb="Reiniciar" /></Show>
            <Show when={s().system.needsReboot}><Line k="up-reboot" label="O sistema pede reinício" desc="Use Ajustes → Sistema → Reiniciar quando puder." warn /></Show>
            <Show when={s().job.running}><Line k="up-job" label={s().job.kind === 'nexus' ? 'Atualizando o Nexus…' : 'Atualizando o sistema…'} desc={s().job.progress || 'Aguarde, não desligue o aparelho'} /></Show>
            <Show when={s().job.done && !s().job.running}><Line k="up-done" label={s().job.done!.ok ? 'Última atualização: concluída' : 'Última atualização: não deu certo'} desc={s().job.done!.message} warn={!s().job.done!.ok} /></Show>
            <h3 class="sh2">Ações</h3>
            <Act k="up-check" a="sys:up:check" icon="refresh" label={checking() ? 'Procurando…' : 'Procurar atualizações'} desc="Consulta a versão mais nova do Nexus e os pacotes do sistema" verb="Procurar" off={checking() || s().job.running} />
            <Show when={s().nexus.hasUpdate && s().nexus.packaged}>
              <Act k="up-install" a="sys:up:nexus" icon="download" label={`Instalar o Nexus ${s().nexus.latest}`} desc={busy() ? 'Indisponível agora' : 'Baixa a versão nova e instala; pede PIN dos responsáveis'} verb="Instalar" off={busy()} />
            </Show>
            <Show when={s().system.upgradable > 0}>
              <Act k="up-install-sys" a="sys:up:system" icon="download" label={`Atualizar o sistema (${s().system.upgradable})`} desc={busy() ? 'Indisponível agora' : 'Atualiza os programas do Linux; pode pedir reinício'} verb="Atualizar" off={busy()} />
            </Show>
            <Show when={s().nexus.revertTo}>
              <Act k="up-revert" a="sys:up:revert" icon="back" label={`Voltar para o Nexus ${s().nexus.revertTo}`} desc="Se a versão nova deu problema, instala a anterior" verb="Voltar" off={busy()} danger />
            </Show>
            <h3 class="sh2">Histórico</h3>
            <For each={s().history} fallback={<Line k="up-hist-none" label="Nenhuma atualização feita por aqui ainda" />}>
              {(h, i) => <Line k={`up-hist-${i()}`} label={`${when(h.ts)} · ${h.kind === 'nexus' ? 'Nexus' : 'Sistema'}${h.kind === 'nexus' && h.from ? ` ${h.from} → ${h.to}` : ''}`} desc={h.ok ? h.message || 'Concluída' : `Não deu certo: ${h.message}`} warn={!h.ok} />}
            </For>
          </>
        );
      }}
    </Show>
  );
}

// ---------------------------------------------------------------- Sobre e diagnóstico (B16)
export function AboutPanel() {
  const [st, setSt] = createSignal<AboutStatus | null>(null);
  const [err, setErr] = createSignal('');
  const [rep, setRep] = createSignal<SupportReport | null>(null);
  const [making, setMaking] = createSignal(false);
  const load = () => api.aboutStatus().then((s) => { setSt(s); setErr(''); }).catch(() => setErr('Não foi possível ler as informações do aparelho'));
  useSys('about', (a) => {
    if (a === 'reload') void load();
    else if (a === 'report' && !making()) {
      setMaking(true);
      void guarded((x) => api.aboutReport(x), 'Confirme para gerar o relatório')
        .then((r) => {
          if (r) {
            setRep(r);
            sfx('on');
            toast('Relatório de suporte pronto', r.name, 'check');
          }
        })
        .finally(() => setMaking(false));
    }
  });
  onMount(() => {
    void load();
    const t = setInterval(() => void load(), 5000);
    onCleanup(() => clearInterval(t));
  });
  return (
    <Show when={st()} fallback={err() ? <Act k="ab-reload" a="sys:about:reload" icon="refresh" label="Tentar de novo" desc={err()} verb="Tentar" /> : <Line k="ab-loading" label="Lendo o aparelho…" />}>
      {(s) => (
        <>
          <h3 class="sh2">Aparelho</h3>
          <Line k="ab-model" label={s().model || 'Aparelho sem nome de modelo'} desc={`${s().arch} · desempenho ${{ high: 'alto', mid: 'médio', low: 'básico' }[s().tier] ?? s().tier}`} />
          <Line k="ab-cpu" label="Processador" desc={`${s().cpu} · ${s().cores} núcleo${s().cores === 1 ? '' : 's'}`} />
          <Line k="ab-mem" label="Memória" desc={`${gb(s().memoryUsedMb)} em uso de ${gb(s().memoryMb)}`} />
          <Line k="ab-gpu" label="Placa de vídeo" desc={{ amd: 'AMD', intel: 'Intel', nvidia: 'NVIDIA', broadcom: 'Broadcom (Raspberry Pi)', mali: 'Mali', adreno: 'Adreno', virtual: 'Virtual (máquina virtual)', unknown: 'Não identificada' }[s().gpu] ?? s().gpu} />
          <Line k="ab-os" label="Sistema" desc={`${s().os} · kernel ${s().kernel}`} />
          <Line k="ab-nexus" label="Nexus" desc={`Versão ${s().nexus}`} />
          <Line k="ab-disk" label="Armazenamento" desc={`${fmtBytes(s().diskFreeBytes)} livres de ${fmtBytes(s().diskTotalBytes)}`} />
          <h3 class="sh2">Agora</h3>
          <Line k="ab-cpupct" label="Uso do processador" desc={`${s().cpuPct}% · carga ${s().load.join(' ')}`} />
          <Line k="ab-temp" label="Temperatura" desc={s().tempC === null ? 'Este aparelho não informa' : `${Math.round(s().tempC!)} °C${s().tempC! >= 80 ? ' — quente: veja a ventilação' : ''}`} warn={(s().tempC ?? 0) >= 80} />
          <Line k="ab-throttle" label="Energia e calor (throttling)" desc={s().throttle === null ? 'Só o Raspberry Pi informa isso; neste aparelho não se aplica' : s().throttle!.now.length ? `Agora: ${s().throttle!.now.join(', ')}` : s().throttle!.past.length ? `Tudo certo agora; desde que ligou: ${s().throttle!.past.join(', ')}` : 'Tudo certo: nenhum limite foi acionado'} warn={!!s().throttle?.now.length} />
          <Line k="ab-up" label="Ligado há" desc={dur(s().uptimeSecs)} />
          <h3 class="sh2">O que mais consome</h3>
          <For each={s().top} fallback={<Line k="ab-top-none" label="Não consegui listar os programas" />}>
            {(p, i) => <Line k={`ab-top-${i()}`} label={p.name} desc={`${p.cpu}% do processador · ${p.memMb} MB de memória`} />}
          </For>
          <h3 class="sh2">Suporte</h3>
          <Act k="ab-report" a="sys:about:report" icon="list" label={making() ? 'Gerando…' : 'Gerar relatório de suporte'} desc="Junta informações e registros em um arquivo (sem senhas nem PIN) para você mandar a quem for ajudar" verb="Gerar" off={making()} />
          <Show when={rep()}>
            {(r) => (
              <div class="qrcard" data-k="ab-report-card">
                <div class="qr" innerHTML={r().qrSvg} role="img" aria-label="QR code para baixar o relatório no celular" />
                <div class="qrinfo">
                  <h3>Pegue o relatório pelo celular</h3>
                  <ol>
                    <li>Conecte o celular na mesma rede Wi-Fi do aparelho</li>
                    <li>Escaneie o QR code e salve ou compartilhe o arquivo</li>
                  </ol>
                  <code>{r().name} · {fmtBytes(r().size)}</code>
                  <small>Também fica salvo em {r().dir}</small>
                </div>
              </div>
            )}
          </Show>
        </>
      )}
    </Show>
  );
}

// ---------------------------------------------------------------- Acesso por SSH (passo a passo + liga/desliga)
export function SshPanel() {
  const [st, setSt] = createSignal<SshStatus | null>(null);
  const [err, setErr] = createSignal('');
  const [busy, setBusy] = createSignal(false);
  const load = () => api.sshStatus().then((s) => { setSt(s); setErr(''); }).catch(() => setErr('Não foi possível consultar o servidor SSH'));
  useSys('ssh', (a) => {
    const s = st();
    if (a === 'reload') void load();
    else if (a === 'toggle' && s && s.installed && !busy()) {
      const on = !s.active;
      void (async () => {
        if (on && !(await askConfirm({ title: 'Ligar o acesso por SSH?', sub: 'Qualquer pessoa na sua rede que souber a senha do usuário poderá entrar no aparelho. Use só em rede de confiança e com senha forte.', yes: 'Ligar' }))) return;
        setBusy(true);
        try {
          const r = await guarded((x) => api.sshSet(on, x), on ? 'Confirme para ligar o SSH' : 'Confirme para desligar o SSH');
          if (r) {
            setSt(r);
            sfx('on');
            toast(r.active ? 'SSH ligado' : 'SSH desligado', r.active ? `ssh ${r.user}@${r.ips[0] ?? 'IP'}` : 'Ninguém entra mais por SSH', r.active ? 'check' : 'lock');
          }
        } finally {
          setBusy(false);
        }
      })();
    }
  });
  onMount(() => {
    void load();
    const t = setInterval(() => void load(), 5000);
    onCleanup(() => clearInterval(t));
  });
  return (
    <Show when={st()} fallback={err() ? <Act k="ssh-reload" a="sys:ssh:reload" icon="refresh" label="Tentar de novo" desc={err()} verb="Tentar" /> : <Line k="ssh-loading" label="Consultando…" />}>
      {(s) => {
        const ip = () => s().ips[0] ?? 'IP-DO-APARELHO';
        const port = () => (s().port === 22 ? '' : ` -p ${s().port}`);
        const cmd = () => `ssh${port()} ${s().user}@${ip()}`;
        return (
          <>
            <Line k="ssh-intro" label="Entrar no aparelho por outro computador" desc="O SSH abre um terminal deste aparelho dentro do seu computador (Linux, macOS ou Windows 10/11), pela rede. Siga os passos na ordem." />
            <div class={`srow f ${!s().installed || busy() ? 'off' : ''}`} data-k="ssh-toggle" data-r="22" data-a={s().installed && !busy() ? 'sys:ssh:toggle' : 'noop'} data-verb={s().active ? 'Desligar' : 'Ligar'}>
              <div>
                <b>Servidor SSH</b>
                <small>{!s().installed ? 'Não instalado neste aparelho (passo 1)' : s().active ? `Ligado · aceita conexões na porta ${s().port}` : 'Desligado · ninguém entra por SSH'}</small>
              </div>
              <Switch on={s().active} />
            </div>
            <Line step k="ssh-1" label="Passo 1 — Instalar o servidor SSH" desc={s().installed ? 'Já está instalado neste aparelho. Nada a fazer.' : 'Ainda não está instalado. No terminal do aparelho (Ctrl+Alt+F3 no teclado, ou outro acesso), digite:'} code={s().installed ? undefined : s().installCmd} />
            <Line step k="ssh-2" label="Passo 2 — Ligar o servidor" desc={s().active ? 'Já está ligado (use a chave acima para desligar).' : s().installed ? 'Use a chave “Servidor SSH” acima (pede o PIN dos responsáveis) ou, no terminal do aparelho:' : 'Depois de instalar, ligue pela chave acima ou, no terminal do aparelho:'} code={s().active ? undefined : s().enableCmd} />
            <Line step k="ssh-3" label="Passo 3 — Descobrir o usuário e o endereço (IP)" desc={`Usuário: ${s().user} · Endereço: ${s().ips.length ? s().ips.join(' ou ') : 'sem rede! Conecte o Wi-Fi ou o cabo em Ajustes → Rede'} · Porta: ${s().port}${s().host ? ` · Nome do aparelho: ${s().host}` : ''}`} />
            <Line step k="ssh-4" label="Passo 4 — No outro computador, abra o terminal e digite" desc={s().ips.length ? 'Este é o comando pronto para o seu aparelho (troque nada):' : 'Troque IP-DO-APARELHO pelo endereço quando a rede estiver conectada:'} code={cmd()} />
            <Line step k="ssh-5" label="Passo 5 — Primeiro acesso" desc={`O terminal pergunta “Are you sure you want to continue connecting (yes/no)?”.${s().fingerprint ? ` Confira que a impressão digital é ${s().fingerprint} e digite yes.` : ' Digite yes.'} Depois digite a senha do usuário ${s().user} (a mesma do login do aparelho; ela não aparece enquanto você digita).`} />
            <Line step k="ssh-6" label="Dica — entrar sem digitar a senha" desc="No outro computador, crie uma chave (uma vez) e envie para o aparelho:" code={`ssh-keygen -t ed25519\nssh-copy-id${port()} ${s().user}@${ip()}`} />
            <Line step k="ssh-7" label="Para sair do terminal e voltar à interface" desc="Digite exit (ou Ctrl+D). O Nexus continua aberto na TV: o SSH é só um acesso a mais. Para ver o que o Nexus está fazendo: journalctl --user -u nexus-core -f" />
            <Line step k="ssh-8" label="Quando terminar, desligue o SSH" desc="Use a chave “Servidor SSH” acima, ou Ajustes → Recuperação. No terminal do aparelho: sudo systemctl disable --now ssh" />
            <Line step k="ssh-safe" label="Segurança" desc="Use uma senha forte, só em rede de casa, e nunca libere a porta do SSH no roteador para a internet." warn />
          </>
        );
      }}
    </Show>
  );
}

// ---------------------------------------------------------------- Recuperação (B15)
export function RecoveryPanel() {
  const [ok, setOk] = createSignal(false);
  const [desk, setDesk] = createSignal(false);
  const run = async (action: 'restart_session' | 'repair_shell' | 'desktop_on' | 'desktop_off' | 'restore_defaults' | 'erase_profile' | 'factory_reset', id?: string) => {
    const r = await guarded((a) => api.recoveryRun({ action, id, ...a }), 'Confirme para continuar');
    if (r) setDesk(r.desktopMode);
    return !!r;
  };
  const loadDesk = () => void guarded((a) => api.recoveryStatus(a)).then((s) => s && setDesk(s.desktopMode));
  useSys('rec', (a) => {
    const row = () => document.querySelector<HTMLElement>(`#setpane [data-a="sys:rec:${a}"]`);
    void (async () => {
      if (a === 'session') {
        if (await askConfirm({ title: 'Reiniciar a sessão do Nexus?', sub: 'A tela some por alguns segundos e volta ao início. Jogos abertos serão fechados.', yes: 'Reiniciar' })) await run('restart_session');
      } else if (a === 'shell') {
        if (await askConfirm({ title: 'Reparar a interface?', sub: 'Reinicia só a tela do Nexus. Seus dados e jogos continuam.', yes: 'Reparar' })) await run('repair_shell');
      } else if (a === 'desktop') {
        const on = !desk();
        if (await askConfirm({ title: on ? 'Voltar ao desktop normal no próximo login?' : 'Cancelar a volta ao desktop?', sub: on ? 'Na próxima vez que entrar, abre o desktop do Linux em vez do Nexus. Para voltar ao Nexus, desligue esta opção por aqui ou escolha a sessão “Nexus” na tela de login.' : 'O próximo login abre o Nexus.', yes: on ? 'Ligar' : 'Desligar' })) {
          if (await run(on ? 'desktop_on' : 'desktop_off')) toast(on ? 'Próximo login: desktop normal' : 'Próximo login: Nexus', '', 'check');
        }
      } else if (a === 'defaults') {
        if (await askConfirm({ title: 'Restaurar os padrões?', sub: 'Tema, sons, tela (margem), teclado e preferências de todos os perfis voltam ao original. Perfis, jogos e PIN ficam.', yes: 'Restaurar', danger: true })) {
          if (await run('restore_defaults')) toast('Padrões restaurados', 'Tema, sons e ajustes voltaram ao original', 'check');
        }
      } else if (a.startsWith('erase:')) {
        const id = a.slice(6);
        const p = S.profiles.find((x) => x.id === id);
        if (await askConfirm({ title: `Apagar os dados de ${p?.name ?? 'este perfil'}?`, sub: 'Tempo de jogo, histórico, favoritos e preferências somem. O perfil, o nome e o PIN ficam.', yes: 'Apagar', danger: true })) {
          if (await run('erase_profile', id)) toast('Dados apagados', p?.name ?? '', 'trash');
        }
      } else if (a === 'factory') {
        if (!(await askConfirm({ title: 'Reset de fábrica?', sub: 'Apaga TODOS os perfis, o PIN dos responsáveis, os apps adicionados e os históricos. Os jogos e as ROMs não são apagados.', yes: 'Continuar', danger: true }))) return;
        if (!(await askConfirm({ title: 'Última chance: apagar tudo do Nexus?', sub: 'Depois disto o aparelho volta ao assistente de primeiro uso. Isso não pode ser desfeito.', yes: 'Apagar tudo', no: 'Cancelar', danger: true }))) return;
        if (await run('factory_reset')) location.reload();
      } else if (a === 'go-upd') setS('setCat', 'upd');
      else if (a === 'go-about') setS('setCat', 'sobre');
      else if (a === 'go-ssh') setS('setCat', 'ssh');
    })().catch(fail).finally(() => {
      // cancelar um diálogo em cadeia não pode deixar o foco na lista lateral (o motor devolve o foco depois do fechamento)
      setTimeout(() => {
        const r = row();
        if (r && !a.startsWith('go-') && !document.querySelector('#ovs [data-kind=confirm], #ovs [data-kind=pin]') && !document.querySelector('#setpane .f.focus')) fx.f?.set(r, { silent: true });
      }, 120);
    });
  });
  onMount(() => {
    // a recuperação fica atrás do PIN dos responsáveis (se houver um)
    void requireParental('Recuperação: confirme com o PIN dos responsáveis').then((v) => {
      if (v) {
        setOk(true);
        loadDesk();
      } else setS('setCat', 'sis');
    });
  });
  return (
    <Show when={ok()} fallback={<Line k="rec-locked" label="Recuperação protegida" desc="Digite o PIN dos responsáveis para continuar" />}>
      <Line k="rec-intro" label="Use só se algo deu errado" desc="Estas ações consertam ou apagam coisas. Cada uma pede confirmação." />
      <h3 class="sh2">Consertar</h3>
      <Act k="rec-session" a="sys:rec:session" icon="refresh" label="Reiniciar a sessão" desc="Fecha e abre o Nexus de novo (compositor, serviço e interface)" verb="Reiniciar" />
      <Act k="rec-shell" a="sys:rec:shell" icon="tv" label="Reparar a interface" desc="Tela travada, preta ou estranha: reinicia só a tela do Nexus" verb="Reparar" />
      <div class="srow f" data-k="rec-desktop" data-r="22" data-a="sys:rec:desktop" data-verb="Alternar">
        <div><b>Voltar ao desktop normal no próximo login</b><small>Saída de emergência se o Nexus não abre direito. {desk() ? 'LIGADO: o próximo login abre o desktop.' : 'Desligado.'}</small></div>
        <Switch on={desk()} />
      </div>
      <Act k="rec-upd" a="sys:rec:go-upd" icon="download" label="Reverter uma atualização" desc="Atualizações → Voltar para a versão anterior" />
      <Act k="rec-about" a="sys:rec:go-about" icon="info" label="Diagnóstico" desc="Sobre e diagnóstico: temperatura, uso e relatório de suporte" />
      <Act k="rec-ssh" a="sys:rec:go-ssh" icon="gear" label="Acesso por SSH" desc="Ligar ou desligar o SSH e ver o passo a passo com o endereço do aparelho" />
      <Line k="rec-term" label="Terminal" desc="Para um terminal completo, use o acesso por SSH de outro computador: é mais seguro e confortável que um terminal na TV." />
      <h3 class="sh2">Apagar</h3>
      <Act k="rec-defaults" a="sys:rec:defaults" icon="refresh" label="Restaurar padrões" desc="Tema, sons, margem da tela e teclado voltam ao original" verb="Restaurar" danger />
      <For each={S.profiles}>
        {(p) => <Act k={`rec-erase-${p.id}`} a={`sys:rec:erase:${p.id}`} icon="trash" label={`Apagar os dados de ${p.name}`} desc="Histórico, tempo de jogo, favoritos e preferências (o perfil fica)" verb="Apagar" danger />}
      </For>
      <Act k="rec-factory" a="sys:rec:factory" icon="trash" label="Reset de fábrica" desc="Apaga perfis, PIN e históricos e volta ao primeiro uso (jogos e ROMs ficam)" verb="Apagar tudo" danger />
    </Show>
  );
}
