// Ajustes → Armazenamento (B9): uso do disco por categoria, discos USB (abrir, usar como pasta de ROMs, ejetar com
// segurança, formatar com PIN e confirmação dupla) e limpeza do cache. Tudo pelo Core (storage.rs); o Google Drive fica
// na mesma categoria (Drive.tsx).
import { createSignal, For, onCleanup, onMount, Show } from 'solid-js';
import type { StorageUsage, UsbDisk } from '@nexus/protocol';
import { sfx } from '@nexus/ui';
import { api } from '../lib/api';
import { fmtBytes } from '../lib/format';
import { askChoice, askConfirm, fail, guarded, toast } from '../lib/state';
import { Bar, Icon, Switch } from './ui';

export function StoragePanel() {
  const [usage, setUsage] = createSignal<StorageUsage | null>(null);
  const [disks, setDisks] = createSignal<UsbDisk[] | null>(null);
  const [busy, setBusy] = createSignal(false);
  const loadUsage = () => void api.storageUsage().then(setUsage).catch(fail);
  const loadDisks = () => void api.storageDevices().then((r) => setDisks(r.devices)).catch(() => setDisks((d) => d ?? []));
  const run = async (what: () => Promise<void>) => {
    if (busy()) return;
    setBusy(true);
    try {
      await what();
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
      loadDisks();
      loadUsage();
    }
  };
  const format = async (d: UsbDisk) => {
    if (!(await askConfirm({ title: `Formatar “${d.label}”?`, sub: `Tudo o que está neste disco (${fmtBytes(d.size)}) será apagado e não dá para desfazer. Salve o que precisa antes.`, yes: 'Continuar', no: 'Cancelar', danger: true, safe: true }))) return;
    const fs = await askChoice({ title: 'Tipo do disco', sub: 'exFAT funciona em PC, Mac e TV. ext4 só funciona em Linux (mais rápido).', yes: 'exFAT', no: 'ext4' });
    if (fs === null) return;
    if (!(await askConfirm({ title: `Apagar tudo em “${d.label}”?`, sub: 'Última chance: depois disso não há volta.', yes: 'Apagar tudo', no: 'Cancelar', danger: true, safe: true }))) return;
    await run(async () => {
      const r = await guarded((a) => api.storageFormat({ dev: d.dev, fs: fs ? 'exfat' : 'ext4', label: 'NEXUS', ...a }), 'Formatar apaga o disco: PIN dos responsáveis');
      if (r) {
        sfx('on');
        toast('Disco formatado', `Agora ele se chama NEXUS (${fs ? 'exFAT' : 'ext4'})`, 'check');
      }
    });
  };
  onMount(() => {
    loadUsage();
    loadDisks();
    const t = setInterval(loadDisks, 6000); // espetou ou tirou um disco: a lista acompanha
    const h = (e: Event) => {
      const [a, i] = (e as CustomEvent<string>).detail.split(':');
      const d = disks()?.[Number(i)];
      if (a === 'clean') {
        void run(async () => {
          if (!(await askConfirm({ title: 'Limpar o cache?', sub: 'Apaga capas baixadas, ícones e registros antigos. Eles voltam a ser baixados quando precisar. Não apaga jogos, saves nem o cache do Google Drive.', yes: 'Limpar' }))) return;
          const r = await api.storageClean();
          sfx('on');
          toast('Cache limpo', r.freed ? `${fmtBytes(r.freed)} liberados` : 'Não havia nada para apagar', 'trash');
        });
      } else if (a === 'rescan') {
        loadDisks();
      } else if (d) {
        if (a === 'mount') {
          void run(async () => {
            const r = await api.storageMount(d.dev);
            toast('Disco aberto', r.path, 'check');
          });
        } else if (a === 'roms') {
          void run(async () => {
            const r = await api.storageRoms(d.dev, !d.isRoms);
            sfx(r.on ? 'on' : 'off');
            toast(r.on ? 'Pasta de ROMs ligada' : 'Pasta de ROMs desligada', r.on ? `Coloque os jogos em ${r.path}/<console>` : d.label, 'pad');
          });
        } else if (a === 'eject') {
          void run(async () => {
            await api.storageEject(d.dev);
            sfx('on');
            toast('Pode tirar o disco', `“${d.label}” foi ejetado com segurança`, 'check');
          });
        } else if (a === 'format') {
          void format(d);
        }
      }
    };
    document.addEventListener('nexus:stg', h);
    onCleanup(() => {
      clearInterval(t);
      document.removeEventListener('nexus:stg', h);
    });
  });
  const pct = (b: number) => (usage() && usage()!.total ? (b * 100) / usage()!.total : 0);
  return (
    <>
      <Show when={usage()} fallback={<div class="srow info" data-k="st-calc"><div><b>Calculando o uso do disco…</b><small>Pode levar alguns segundos com muitos jogos</small></div></div>}>
        {(u) => (
          <>
            <div class="srow info" data-k="st-total"><div><b>Disco do console</b><small>{fmtBytes(u().free)} livres de {fmtBytes(u().total)} · {fmtBytes(u().used)} em uso</small><Bar pct={pct(u().used)} /></div></div>
            <For each={u().cats}>
              {(c) => (
                <div class="srow info" data-k={`st-cat-${c.id}`}><div><b>{c.label}</b><small>{fmtBytes(c.bytes)}</small><Bar pct={pct(c.bytes)} /></div></div>
              )}
            </For>
          </>
        )}
      </Show>
      <div class={`srow f ${busy() ? 'off' : ''}`} data-k="st-clean" data-r="22" data-a="stg:clean" data-verb="Limpar"><div><b>Limpar cache e temporários</b><small>Capas, ícones e registros antigos (não mexe em jogos nem saves)</small></div><Icon n="trash" /></div>
      <h3 class="sh2">Discos USB</h3>
      <Show when={disks()} fallback={<div class="srow info"><div><b>Procurando discos…</b></div></div>}>
        <For each={disks()!} fallback={<div class="srow info" data-k="st-nousb"><div><b>Nenhum disco USB conectado</b><small>Conecte um pendrive ou HD externo: ele aparece aqui sozinho</small></div></div>}>
          {(d, i) => (
            <>
              <div class="srow info" data-k={`st-usb-${i()}`}>
                <div><b>{d.label}</b><small>{fmtBytes(d.size)} · {d.fs}{d.mount ? ` · aberto${d.free != null ? `, ${fmtBytes(d.free)} livres` : ''}` : ' · ainda não aberto'}{d.isRoms ? ' · pasta de ROMs' : ''}</small></div>
                <Icon n="drive" />
              </div>
              <Show when={!d.mount}>
                <div class={`srow f ${busy() ? 'off' : ''}`} data-k={`st-usb-${i()}-mount`} data-r="22" data-a={`stg:mount:${i()}`} data-verb="Abrir"><div><b>Abrir o disco</b><small>Deixa o disco pronto para usar</small></div><Icon n="plus" /></div>
              </Show>
              <div class={`srow f ${busy() ? 'off' : ''}`} data-k={`st-usb-${i()}-roms`} data-r="22" data-a={`stg:roms:${i()}`} data-verb="Alternar"><div><b>Usar como pasta de ROMs</b><small>Os jogos da pasta ROMs do disco aparecem nos Emuladores</small></div><Switch on={d.isRoms} /></div>
              <div class={`srow f ${busy() ? 'off' : ''}`} data-k={`st-usb-${i()}-eject`} data-r="22" data-a={`stg:eject:${i()}`} data-verb="Ejetar"><div><b>Ejetar com segurança</b><small>Só tire o disco depois deste aviso</small></div><Icon n="up" /></div>
              <div class={`srow f ${busy() ? 'off' : ''}`} data-k={`st-usb-${i()}-format`} data-r="22" data-a={`stg:format:${i()}`} data-verb="Formatar" style={{ color: 'var(--danger)' }}><div><b>Formatar…</b><small>Apaga tudo no disco. Pede o PIN dos responsáveis</small></div><Icon n="trash" /></div>
            </>
          )}
        </For>
        <div class="srow f" data-k="st-rescan" data-r="22" data-a="stg:rescan" data-verb="Procurar"><div><b>Procurar discos de novo</b></div><Icon n="refresh" /></div>
      </Show>
    </>
  );
}
