import { createMemo, createSignal, For, onCleanup, onMount, Show } from 'solid-js';
import type { RomListing, RomMedia } from '@nexus/protocol';
import { sfx } from '@nexus/ui';
import { api, client } from '../lib/api';
import { fail, focusLayer, toast } from '../lib/state';
import { fmtBytes } from '../lib/format';
import { Icon } from './ui';

/**
 * Adicionar jogos de console (ROMs) pela TV: do pendrive/pasta (escolhe e copia) ou de outro aparelho
 * (QR code + link para a página de envio do Nexus, na mesma rede Wi-Fi).
 */
export function Roms(p: { oid: number }) {
  const [media, setMedia] = createSignal<RomMedia | null>(null);
  const [l, setL] = createSignal<RomListing | null>(null);
  const [sel, setSel] = createSignal<Record<string, string>>({}); // caminho → console
  const [names, setNames] = createSignal<Record<string, string>>({});
  const [link, setLink] = createSignal<{ url: string; qrSvg: string } | null>(null);
  const [busy, setBusy] = createSignal<{ done: number; total: number; name: string } | null>(null);
  const count = createMemo(() => Object.keys(sel()).length);
  const sysName = (d: string) => names()[d] ?? d.toUpperCase();
  const refocus = () => queueMicrotask(() => requestAnimationFrame(() => focusLayer(p.oid)));

  const loadMedia = async () => {
    try {
      setMedia(await api.romsMedia());
      refocus();
    } catch (e) {
      fail(e);
    }
  };
  const open = async (path: string) => {
    try {
      setL(await api.romsBrowse(path));
      refocus();
    } catch (e) {
      fail(e);
    }
  };
  const toggle = (path: string) => {
    const f = l()?.files.find((x) => x.path === path);
    if (!f) return;
    const cur = { ...sel() };
    if (cur[path]) delete cur[path];
    else if (f.systems.length) cur[path] = f.systems[0];
    else return void toast('Console desconhecido', 'Coloque o arquivo numa pasta com o nome do console (ex.: snes)', 'info');
    sfx('select');
    setSel(cur);
  };
  // arquivo que serve para vários consoles (.iso, .bin, .chd…): troca o console a cada toque
  const cycle = (path: string) => {
    const f = l()?.files.find((x) => x.path === path);
    if (!f || f.systems.length < 2) return;
    const cur = { ...sel() };
    const i = f.systems.indexOf(cur[path] ?? '');
    cur[path] = f.systems[(i + 1) % f.systems.length];
    setSel(cur);
  };
  const copy = async () => {
    const items = Object.entries(sel()).map(([path, system]) => ({ path, system }));
    if (!items.length || busy()) return;
    setBusy({ done: 0, total: items.length, name: '' });
    try {
      const r = await api.romsImport(items);
      setSel({});
      sfx('launch');
      if (r.failed.length) toast(`${r.copied.length} copiados · ${r.failed.length} com problema`, `${r.failed[0].name}: ${r.failed[0].error}`, 'info');
      else toast(`${r.copied.length} ${r.copied.length === 1 ? 'jogo adicionado' : 'jogos adicionados'}`, 'Já estão na aba Emuladores', 'check');
    } catch (e) {
      fail(e);
    } finally {
      setBusy(null);
    }
  };

  onMount(() => {
    void loadMedia();
    void api.romsUploadLink().then(setLink).catch(() => {});
    void api.emuStatus().then((st) => setNames(Object.fromEntries(st.systems.map((s) => [s.dir, s.name])))).catch(() => {});
    const offP = client.on('roms.progress', (x: { done: number; total: number; name: string }) => setBusy(x.done < x.total ? x : null));
    const offR = client.on('roms.received', (x: { name: string }) => toast('Jogo recebido', x.name, 'download'));
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      const i = a.indexOf(':');
      const kind = a.slice(0, i);
      const arg = a.slice(i + 1);
      if (kind === 'dir') void open(arg);
      else if (kind === 'home') {
        setL(null);
        setSel({});
        void loadMedia();
      } else if (kind === 'dev') {
        void api.romsMount(arg).then((r) => open(r.path)).catch(fail);
      } else if (kind === 'tog') toggle(arg);
      else if (kind === 'sys') cycle(arg);
      else if (kind === 'copy') void copy();
    };
    document.addEventListener('nexus:rs', h);
    onCleanup(() => {
      offP();
      offR();
      document.removeEventListener('nexus:rs', h);
    });
  });

  return (
    <>
      <div class="stitle">Adicionar jogos</div>
      <div class="setpane wide rpane">
        <Show when={l()} fallback={
          <Show when={media()} fallback={<p class="pane-sub">Procurando pendrives…</p>}>
            {(m) => (
              <>
                <h3 class="rh">De onde vêm os jogos?</h3>
                <For each={m().media}>
                  {(d) => (
                    <div class="srow f" data-k={`rs-dev-${d.dev}`} data-r="22" data-a={d.mount ? `rs:dir:${d.mount}` : `rs:dev:${d.dev}`} data-verb="Abrir">
                      <div><b>{d.label}</b><small>Pendrive · {fmtBytes(d.size)}</small></div>
                      <Icon n="drive" />
                    </div>
                  )}
                </For>
                <Show when={!m().media.length}>
                  <p class="pane-sub">Nenhum pendrive conectado. Conecte um pendrive com os jogos na TV e escolha "Procurar de novo".</p>
                </Show>
                <div class="srow f" data-k="rs-again" data-r="22" data-a="rs:home:" data-verb="Atualizar"><div><b>Procurar de novo</b><small>Pendrive, HD externo ou cartão de memória</small></div><Icon n="refresh" /></div>
                <div class="srow f" data-k="rs-dl" data-r="22" data-a={`rs:dir:${m().home}/Downloads`} data-verb="Abrir"><div><b>Downloads</b><small>Arquivos baixados neste aparelho</small></div><Icon n="download" /></div>
                <div class="srow f" data-k="rs-home" data-r="22" data-a={`rs:dir:${m().home}`} data-verb="Abrir"><div><b>Pasta pessoal</b><small>{m().home}</small></div><Icon n="home" /></div>
              </>
            )}
          </Show>
        }>
          {(x) => (
            <>
              <p class="pane-sub mono">{x().path}{x().system ? ` · ${sysName(x().system!)}` : ''}</p>
              <div class="srow f" data-k="rs-up" data-autofocus data-r="22" data-a={x().parent ? `rs:dir:${x().parent}` : 'rs:home:'} data-verb="Voltar"><div><b>{x().parent ? 'Pasta anterior' : 'Outros lugares'}</b></div><Icon n="up" /></div>
              <For each={x().dirs}>
                {(d) => (
                  <div class="srow f" data-k={`rs-${d.path}`} data-r="22" data-a={`rs:dir:${d.path}`} data-verb="Abrir">
                    <div><b>{d.name}</b><small>{d.system ? `Pasta · ${sysName(d.system)}` : 'Pasta'}</small></div>
                    <Icon n="right" />
                  </div>
                )}
              </For>
              <For each={x().files}>
                {(f) => (
                  <div class="rsfile">
                    <div class={`srow f ${sel()[f.path] ? 'on' : ''}`} data-k={`rs-${f.path}`} data-r="22" data-a={`rs:tog:${f.path}`} data-verb={sel()[f.path] ? 'Desmarcar' : 'Marcar'}>
                      <div><b>{f.name}</b><small>{fmtBytes(f.size)} · {f.systems.length === 1 ? sysName(f.systems[0]) : f.systems.length ? 'vários consoles' : 'console desconhecido'}</small></div>
                      <Icon n={sel()[f.path] ? 'check' : 'plus'} />
                    </div>
                    <Show when={sel()[f.path] && f.systems.length > 1}>
                      <div class="cp f" data-k={`rs-sys-${f.path}`} data-r="p" data-pad="8" data-a={`rs:sys:${f.path}`} data-verb="Trocar console">{sysName(sel()[f.path])}</div>
                    </Show>
                  </div>
                )}
              </For>
              <Show when={!x().files.length && !x().dirs.length}><p class="pane-sub">Esta pasta não tem jogos de console nem subpastas.</p></Show>
            </>
          )}
        </Show>
      </div>
      <div class="rsbar">
        <Show when={busy()} fallback={
          <div class={`btn pri f ${count() ? '' : 'off'}`} data-k="rs-copy" data-r="p" data-pad="8" data-a="rs:copy:" data-verb="Copiar"><Icon n="download" /> {count() ? `Copiar ${count()} ${count() === 1 ? 'jogo' : 'jogos'}` : 'Marque os jogos para copiar'}</div>
        }>
          {(b) => <p class="pane-sub"><span class="spin" />Copiando {b().done + 1} de {b().total}{b().name ? ` · ${b().name}` : ''}</p>}
        </Show>
      </div>
      <aside class="rqr">
        <Show when={link()} fallback={<p class="pane-sub">Preparando o envio pelo celular…</p>}>
          {(k) => (
            <>
              <div class="qr" innerHTML={k().qrSvg} />
              <h3>Faça o upload de outro dispositivo</h3>
              <p>Aponte a câmera do celular para o QR code ou abra o link abaixo no navegador de um celular ou computador <b>na mesma rede Wi-Fi</b>. Escolha o console e os arquivos: eles chegam direto aqui.</p>
              <p class="mono lnk">{k().url}</p>
            </>
          )}
        </Show>
      </aside>
    </>
  );
}
