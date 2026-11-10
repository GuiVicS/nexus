import { createMemo, createSignal, For, onCleanup, onMount, Show } from 'solid-js';
import type { CustomApp, FsListing, SystemApp } from '@nexus/protocol';
import { sfx } from '@nexus/ui';
import { api } from '../lib/api';
import { askConfirm, askText, closeOv, fail, findItem, focusLayer, guarded, pushOv, S, toast } from '../lib/state';
import { cycHandlers } from '../lib/actions';
import { norm } from '../lib/format';
import { Icon, Tile } from './ui';

const changed = () => document.dispatchEvent(new CustomEvent('nexus:apps-changed'));

// ---------- escolher um app instalado no sistema ----------
export function AppPicker(p: { oid: number }) {
  const [apps, setApps] = createSignal<SystemApp[] | null>(null);
  const [q, setQ] = createSignal('');
  const list = createMemo(() => {
    const f = norm(q().trim());
    return (apps() ?? []).filter((a) => !f || norm(`${a.name} ${a.category}`).includes(f));
  });
  const pick = async (fileId: string) => {
    const a = (apps() ?? []).find((x) => x.fileId === fileId);
    if (!a) return;
    if (a.added) {
      closeOv(p.oid);
      void pushOv('appedit', { id: `native:${fileId}` });
      return;
    }
    const r = await guarded((auth) => api.addApp(fileId, undefined, auth), 'Confirme para adicionar o app');
    if (!r) return;
    sfx('on');
    toast('Adicionado ao Nexus', r.name, 'check');
    changed();
    closeOv(p.oid);
    void pushOv('appedit', { id: r.id, fresh: true });
  };
  const askFilter = async () => {
    const v = await askText({ title: 'Buscar app', value: q(), max: 24 });
    if (v !== null) setQ(v);
    queueMicrotask(() => requestAnimationFrame(() => focusLayer(p.oid)));
  };
  onMount(() => {
    void api.systemApps().then(setApps).catch(fail);
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      if (a === '__filter') void askFilter();
      else void pick(a);
    };
    document.addEventListener('nexus:pickapp', h);
    onCleanup(() => document.removeEventListener('nexus:pickapp', h));
  });
  return (
    <>
      <div class="stitle">Adicionar app ou jogo</div>
      <div class="setpane wide" id="pickpane">
        <p class="pane-sub">Escolha entre os programas instalados no sistema. A Steam e os jogos dela já entram sozinhos.</p>
        <div class="srow f" data-k="pk-filter" data-autofocus data-r="22" data-a="pickapp:__filter" data-verb="Buscar">
          <div><b>Buscar</b><small>{q() ? `Filtrando por “${q()}”` : 'Toque para digitar o nome'}</small></div>
          <Icon n="search" />
        </div>
        <Show when={apps()} fallback={<p class="pane-sub">Procurando aplicativos…</p>}>
          <For each={list()} fallback={<p class="pane-sub">Nenhum app encontrado.</p>}>
            {(a) => (
              <div class="srow f" data-k={`pk-${a.fileId}`} data-r="22" data-a={`pickapp:${a.fileId}`} data-verb={a.added ? 'Editar' : 'Adicionar'}>
                <div><b>{a.name}</b><small>{a.category}{a.added ? ' · já adicionado' : ''}</small></div>
                <Icon n={a.added ? 'check' : 'plus'} />
              </div>
            )}
          </For>
        </Show>
      </div>
    </>
  );
}

// ---------- editar nome, tipo e imagens ----------
export function AppEditor(p: { id: string; fresh?: boolean; oid: number }) {
  const [info, setInfo] = createSignal<CustomApp | null>(null);
  const it = createMemo(() => findItem(p.id));
  const load = () => void api.customApps().then((l) => setInfo(l.find((a) => a.id === p.id) ?? null)).catch(fail);
  const update = async (patch: Record<string, unknown>, ok?: string) => {
    const r = await guarded((a) => api.updateApp({ id: p.id, ...patch, ...a } as never), 'Confirme para alterar o app');
    if (r) {
      setInfo(r);
      changed();
      sfx('on');
      if (ok) toast(ok, r.name, 'check');
    }
  };
  const chooseImage = async (field: 'icon' | 'cover' | 'hero', title: string) => {
    const path = (await pushOv<string>('filepick', { title })) as string | undefined;
    if (path) await update({ [field]: path }, 'Imagem atualizada');
  };
  const remove = async () => {
    const i = info();
    if (!(await askConfirm({ title: `Remover ${i?.name ?? 'app'} do Nexus?`, sub: 'O programa continua instalado no sistema; só sai da sua biblioteca.', yes: 'Remover', danger: true }))) return;
    const r = await guarded((a) => api.removeApp(p.id, a), 'Confirme para remover o app');
    if (r) {
      toast('Removido do Nexus', i?.name ?? '', 'trash');
      changed();
      closeOv(p.oid);
    }
  };
  onMount(() => {
    load();
    cycHandlers.set('ae:kind', () => void update({ kind: info()?.kind === 'game' ? 'app' : 'game' }));
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      if (a === 'name') {
        void askText({ title: 'Nome', value: info()?.name ?? '', max: 40 }).then((v) => {
          if (v) void update({ name: v }, 'Nome alterado');
        });
      } else if (a === 'noname') void update({ name: null });
      else if (a === 'kind') cycHandlers.get('ae:kind')?.(1);
      else if (a === 'icon') void chooseImage('icon', 'Escolher ícone');
      else if (a === 'cover') void chooseImage('cover', 'Escolher capa do card');
      else if (a === 'hero') void chooseImage('hero', 'Escolher capa de fundo');
      else if (a.startsWith('rm:')) void update({ [a.slice(3)]: null });
      else if (a === 'remove') void remove();
      else if (a === 'done') closeOv(p.oid);
    };
    document.addEventListener('nexus:ae', h);
    onCleanup(() => {
      document.removeEventListener('nexus:ae', h);
      cycHandlers.delete('ae:kind');
    });
  });
  const imgRow = (field: 'icon' | 'cover' | 'hero', label: string, desc: string, has: () => boolean) => (
    <div class="srow col">
      <div><b>{label}</b><small>{desc}</small></div>
      <div class="chips">
        <div class="btn f" data-k={`ae-${field}`} data-r="p" data-pad="8" data-a={`ae:${field}`} data-verb="Escolher"><Icon n="grid" /> {has() ? 'Trocar imagem' : 'Escolher imagem'}</div>
        <Show when={has()}><div class="btn f" data-k={`ae-rm-${field}`} data-r="p" data-pad="8" data-a={`ae:rm:${field}`} data-verb="Remover"><Icon n="trash" /> Usar a padrão</div></Show>
      </div>
    </div>
  );
  return (
    <>
      <div class="stitle">{p.fresh ? 'Personalizar' : 'Editar app'}</div>
      <Show when={info()} fallback={<p class="pane-sub" style={{ position: 'absolute', left: '96px', top: '200px' }}>Carregando…</p>}>
        {(i) => (
          <>
            <div class="ae-prev">
              <Show when={it()}>{(item) => <div class="ae-card"><Tile it={item()} a="noop" noProg /></div>}</Show>
              <h2>{i().name}</h2>
              <small>{i().kind === 'game' ? 'Jogo' : 'App'}{i().available ? '' : ' · programa não encontrado'}</small>
            </div>
            <div class="setpane ae">
              <div class="srow f" data-k="ae-name" data-autofocus data-r="22" data-a="ae:name" data-verb="Editar">
                <div><b>Nome</b><small>{i().name}{i().name !== i().defaultName ? ` (original: ${i().defaultName})` : ''}</small></div>
                <Icon n="gear" />
              </div>
              <div class="srow f" data-k="ae-kind" data-r="22" data-cyc="ae:kind" data-a="ae:kind" data-verb="Alterar">
                <div><b>Tipo</b><small>Jogos aparecem na Biblioteca; apps na aba Mídia</small></div>
                <div class="cv"><Icon n="left" /><span>{i().kind === 'game' ? 'Jogo' : 'App'}</span><Icon n="right" /></div>
              </div>
              {imgRow('icon', 'Ícone', 'Mostrado nos apps e na barra de execução', () => i().hasIcon)}
              {imgRow('cover', 'Capa do card', 'Imagem do quadrado na tela inicial e nas listas', () => i().hasCover)}
              {imgRow('hero', 'Capa de fundo', 'Imagem grande ao fundo da tela inicial e do item', () => i().hasHero)}
              <div class="lacts pe-act">
                <div class="btn pri f" data-k="ae-done" data-r="p" data-pad="8" data-a="ae:done" data-verb="Concluir"><Icon n="check" /> Concluir</div>
                <div class="btn f" data-k="ae-remove" data-r="p" data-pad="8" data-a="ae:remove" data-verb="Remover" style={{ color: 'var(--danger)' }}><Icon n="trash" /> Remover do Nexus</div>
              </div>
            </div>
          </>
        )}
      </Show>
    </>
  );
}

// ---------- navegador de imagens ----------
export function FilePicker(p: { title: string; oid: number }) {
  const [l, setL] = createSignal<FsListing | null>(null);
  const [err, setErr] = createSignal('');
  const open = async (path?: string) => {
    try {
      const r = await api.fsList(path);
      setL(r);
      setErr('');
      queueMicrotask(() => requestAnimationFrame(() => focusLayer(p.oid)));
    } catch (e) {
      setErr((e as Error).message);
      sfx('deny');
    }
  };
  onMount(() => {
    void open();
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      const i = a.indexOf(':');
      const kind = a.slice(0, i);
      const path = a.slice(i + 1);
      if (kind === 'dir') void open(path);
      else if (kind === 'pick') closeOv(p.oid, path);
    };
    document.addEventListener('nexus:fp', h);
    onCleanup(() => document.removeEventListener('nexus:fp', h));
  });
  return (
    <>
      <div class="stitle">{p.title}</div>
      <div class="setpane wide" id="fpane">
        <Show when={l()} fallback={<p class="pane-sub">{err() || 'Abrindo…'}</p>}>
          {(x) => (
            <>
              <p class="pane-sub mono">{x().path}</p>
              <Show when={err()}><p class="pane-sub">{err()}</p></Show>
              <Show when={x().parent}>
                <div class="srow f" data-k="fp-up" data-autofocus data-r="22" data-a={`fp:dir:${x().parent}`} data-verb="Subir"><div><b>⬑ Pasta anterior</b></div><Icon n="up" /></div>
              </Show>
              <div class="chips" style={{ 'margin-bottom': '14px' }}>
                <For each={x().roots}>{(r) => <div class="cp f" data-k={`fp-root-${r.path}`} data-r="p" data-pad="8" data-a={`fp:dir:${r.path}`} data-verb="Abrir"><Icon n="drive" /> {r.name}</div>}</For>
              </div>
              <For each={x().entries} fallback={<p class="pane-sub">Esta pasta não tem imagens (JPG, PNG, WebP ou SVG) nem subpastas.</p>}>
                {(e) => (
                  <div class="srow f" data-k={`fp-${e.path}`} data-r="22" data-a={e.dir ? `fp:dir:${e.path}` : `fp:pick:${e.path}`} data-verb={e.dir ? 'Abrir' : 'Escolher'}>
                    <div><b>{e.name}</b><small>{e.dir ? 'Pasta' : 'Imagem'}</small></div>
                    <Icon n={e.dir ? 'right' : 'check'} />
                  </div>
                )}
              </For>
            </>
          )}
        </Show>
      </div>
    </>
  );
}

export { S };
