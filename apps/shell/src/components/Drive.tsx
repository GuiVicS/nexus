// Armazenamento → Google Drive (rclone, pelo Core em drive.rs). Também usado no assistente de primeiro uso.
// Entrar abre a página do Google numa janela na TV; ao autorizar, o Core procura/cria a pasta "Nexus", monta e
// a janela fecha sozinha. As ROMs em Nexus/ROMs/<console> aparecem nos Emuladores com a marca da nuvem.
import { createSignal, onCleanup, onMount, Show } from 'solid-js';
import type { DriveStatus } from '@nexus/protocol';
import { sfx } from '@nexus/ui';
import { api, client } from '../lib/api';
import { cycHandlers } from '../lib/actions';
import { fmtBytes } from '../lib/format';
import { askConfirm, fail, guarded, toast } from '../lib/state';
import { Icon } from './ui';

const CACHE_GB = [5, 10, 20, 50, 100, 200];

export function DrivePanel(p: { wizard?: boolean; onDone?: () => void }) {
  const [st, setSt] = createSignal<DriveStatus | null>(null);
  const [busy, setBusy] = createSignal(false);
  const load = async () => {
    try {
      setSt(await api.driveStatus(true));
    } catch (e) {
      fail(e);
    }
  };
  const login = async () => {
    if (busy()) return;
    setBusy(true);
    try {
      const r = await guarded((a) => api.driveLogin(a), 'Conta da casa: PIN dos responsáveis');
      if (r) toast('Google Drive', 'Entre com a sua conta Google e toque em “Permitir”', 'cloud');
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
      void load();
    }
  };
  const logout = async () => {
    if (!(await askConfirm({ title: 'Sair do Google Drive?', sub: 'Os arquivos continuam no seu Drive. Some da TV o acesso e o que estava guardado no cache.', yes: 'Sair', no: 'Cancelar', danger: true }))) return;
    const r = await guarded((a) => api.driveDisconnect(a), 'Conta da casa: PIN dos responsáveis');
    if (r) toast('Google Drive', 'Conta desconectada', 'cloud');
    void load();
  };
  const remount = async () => {
    setBusy(true);
    try {
      const r = await guarded((a) => api.driveMount(a), 'Conta da casa: PIN dos responsáveis');
      if (r) toast('Google Drive', 'Conectado de novo', 'cloud');
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
      void load();
    }
  };
  onMount(() => {
    void load();
    const off = client.on('drive.changed', (ev: { connected: boolean; created?: boolean; folder?: string; error?: string }) => {
      if (ev.error) toast('Google Drive', ev.error, 'info');
      else if (ev.connected) {
        sfx('on');
        toast('Google Drive conectado', ev.created ? `Pasta “${ev.folder}” criada no seu Drive` : `Usando a pasta “${ev.folder}” do seu Drive`, 'cloud');
        p.onDone?.();
      }
      void load();
    });
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      if (a === 'login') void login();
      else if (a === 'cancel') void api.driveCancel().then(load);
      else if (a === 'logout') void logout();
      else if (a === 'mount') void remount();
      else if (a === 'skip') p.onDone?.();
    };
    document.addEventListener('nexus:drive', h);
    cycHandlers.set('drive:cache', (d) => {
      const cur = st()?.cacheGb ?? 20;
      const i = CACHE_GB.findIndex((g) => g >= cur);
      const nx = CACHE_GB[Math.max(0, Math.min(CACHE_GB.length - 1, (i < 0 ? CACHE_GB.length - 1 : i) + d))];
      if (nx === cur) return;
      setSt((s) => (s ? { ...s, cacheGb: nx } : s));
      void guarded((a) => api.driveCache(nx, a), 'Conta da casa: PIN dos responsáveis').then(() => void load());
    });
    onCleanup(() => {
      off();
      document.removeEventListener('nexus:drive', h);
      cycHandlers.delete('drive:cache');
    });
  });
  const quota = (s: DriveStatus) => (s.about?.total ? `${fmtBytes(s.about.used ?? 0)} de ${fmtBytes(s.about.total)} usados` : 'Espaço da conta indisponível agora');
  return (
    <Show when={st()} fallback={<div class="srow info"><div><b>Consultando o Google Drive…</b></div></div>}>
      {(s) => (
        <Show
          when={s().available}
          fallback={
            <>
              <div class="srow info" data-k="dr-none"><div><b>Google Drive indisponível</b><small>O componente rclone não está instalado neste aparelho (sudo apt install rclone)</small></div></div>
              <Show when={p.wizard}>
                <div class="srow f" data-k="dr-skip" data-r="22" data-a="drive:skip" data-verb="Pular"><div><b>Continuar sem o Drive</b><small>Dá para entrar depois em Ajustes → Armazenamento</small></div><Icon n="right" /></div>
              </Show>
            </>
          }
        >
          <Show
            when={s().connected}
            fallback={
              <>
                <div class="srow info"><div><b>Use o Google Drive como armazenamento</b><small>Jogos (ROMs) na nuvem: abrem na TV sem ocupar espaço até você jogar. O Nexus usa só a pasta “{s().folder}” e a cria se ainda não existir.</small></div></div>
                <Show
                  when={!s().loggingIn}
                  fallback={
                    <>
                      <div class="srow info" data-k="dr-wait"><div><b>Aguardando o login no Google…</b><small>Termine na janela do Google. Para digitar, use o teclado do celular.</small></div></div>
                      <div class="srow f" data-k="dr-cancel" data-r="22" data-a="drive:cancel" data-verb="Cancelar"><div><b>Cancelar o login</b></div><Icon n="x" /></div>
                    </>
                  }
                >
                  <div class={`srow f ${busy() ? 'off' : ''}`} data-k="dr-login" data-r="22" data-a="drive:login" data-verb="Entrar"><Icon n="cloud" /><div><b>{busy() ? 'Abrindo o Google…' : 'Entrar no Google Drive'}</b><small>Abre a página do Google aqui na TV</small></div></div>
                </Show>
                <Show when={p.wizard}>
                  <div class="srow f" data-k="dr-skip" data-r="22" data-a="drive:skip" data-verb="Pular"><div><b>Agora não</b><small>Dá para entrar depois em Ajustes → Armazenamento</small></div><Icon n="right" /></div>
                </Show>
              </>
            }
          >
            <div class="srow info" data-k="dr-ok"><Icon n="cloud" /><div><b>Google Drive conectado</b><small>{s().mounted ? `Pasta “${s().folder}” · ${quota(s())}` : 'Sem acesso agora (internet?)'}</small></div></div>
            <Show when={!s().mounted}>
              <div class={`srow f ${busy() ? 'off' : ''}`} data-k="dr-mount" data-r="22" data-a="drive:mount" data-verb="Conectar"><div><b>Conectar de novo</b><small>Tenta acessar o Drive outra vez</small></div><Icon n="refresh" /></div>
            </Show>
            <div class="srow f" data-k="dr-cache" data-r="22" data-cyc="drive:cache" data-a="cyc:drive:cache" data-verb="Alterar">
              <div><b>Espaço para jogos baixados</b><small>{s().cacheBytes > 0 ? `${fmtBytes(s().cacheBytes)} em uso` : 'Nada baixado ainda'} · os menos jogados saem quando passa do limite</small></div>
              <div class="cv"><Icon n="left" /><span>{s().cacheGb} GB</span><Icon n="right" /></div>
            </div>
            <div class="srow info" data-k="dr-how"><div><b>Como colocar jogos</b><small>No site ou app do Google Drive, copie as ROMs para {s().folder}/ROMs/&lt;console&gt; (ex.: {s().folder}/ROMs/snes). Elas aparecem nos Emuladores com a marca da nuvem.</small></div></div>
            <Show when={p.wizard}>
              <div class="srow f" data-k="dr-next" data-r="22" data-a="drive:skip" data-verb="Continuar"><div><b>Continuar</b></div><Icon n="right" /></div>
            </Show>
            <Show when={!p.wizard}>
              <div class="srow f" data-k="dr-logout" data-r="22" data-a="drive:logout" data-verb="Sair"><div><b>Sair da conta</b><small>Os arquivos continuam no seu Drive</small></div><Icon n="trash" /></div>
            </Show>
          </Show>
        </Show>
      )}
    </Show>
  );
}
