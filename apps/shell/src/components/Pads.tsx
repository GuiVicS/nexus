import { createSignal, For, onCleanup, onMount, Show } from 'solid-js';
import type { Pad, PadBinding } from '@nexus/protocol';
import { sfx } from '@nexus/ui';
import { api } from '../lib/api';
import { askConfirm, askText, closeOv, fail, guarded, pushOv, S, setS, toast } from '../lib/state';
import { cycHandlers } from '../lib/actions';
import { Icon } from './ui';
import { BtPanel } from './System';

const BUS: Record<Pad['bus'], string> = { usb: 'USB', bluetooth: 'Bluetooth', virtual: 'Virtual', outro: 'Conectado' };

// ---------- Ajustes → Controles: conectados, Bluetooth e USB ----------
export function PadsSection() {
  const [pads, setPads] = createSignal<Pad[] | null>(null);
  const loadPads = () => void api.pads().then(setPads).catch(() => setPads([]));

  onMount(() => {
    loadPads();
    // controles USB/Bluetooth entram e saem sozinhos: atualiza a lista
    const t = setInterval(loadPads, 3000);
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      if (a.startsWith('edit:')) void pushOv('padedit', { pad: (pads() ?? []).find((p) => p.id === a.slice(5)) }).then(loadPads);
    };
    document.addEventListener('nexus:pads', h);
    onCleanup(() => {
      clearInterval(t);
      document.removeEventListener('nexus:pads', h);
    });
  });

  return (
    <>
      <h3 class="sh2">Controles</h3>
      <Show when={pads()} fallback={<div class="srow info"><div><b>Procurando…</b></div></div>}>
        <For each={pads()!} fallback={<div class="srow info"><div><b>Nenhum controle</b><small>Conecte por USB ou pareie por Bluetooth abaixo</small></div></div>}>
          {(p) => (
            <div class="srow f" data-k={`pad-${p.id}`} data-r="22" data-a={`pads:edit:${p.id}`} data-verb="Editar">
              <div>
                <b>{p.nickname || p.name}</b>
                <small>{p.connected ? BUS[p.bus] : 'Desconectado'} · {p.registered ? 'Cadastrado' : 'Novo'} · {p.mapped ? 'botões mapeados' : 'sem mapeamento'}</small>
              </div>
              <span class={`pbadge ${p.player ? 'on' : ''}`}>{p.player ? `Jogador ${p.player}` : 'Livre'}</span>
            </div>
          )}
        </For>
      </Show>

      <h3 class="sh2">Bluetooth</h3>
      <BtPanel onlyPads />

      <h3 class="sh2">USB</h3>
      <div class="srow info"><div><b>Controles com fio</b><small>Conecte em qualquer porta USB: o controle aparece em “Controles conectados” na hora, sem configurar nada.</small></div></div>
    </>
  );
}

// ---------- Assistente de mapeamento (25 ações, para os emuladores) ----------
const fmt = (b: PadBinding | undefined) => {
  if (!b) return '—';
  if (b.kind === 'button') return `Botão ${b.code}`;
  if (b.kind === 'hat') return `Direcional ${b.code % 2 ? (b.dir < 0 ? '↑' : '↓') : b.dir < 0 ? '←' : '→'}`;
  return `Eixo ${b.code}${b.dir < 0 ? '−' : '+'}`;
};

export function PadMapper(p: { pad: Pad; oid: number }) {
  const [actions, setActions] = createSignal<{ id: string; label: string }[]>([]);
  const [map, setMap] = createSignal<Record<string, PadBinding>>({});
  const [step, setStep] = createSignal(0);
  const [done, setDone] = createSignal(false);
  let alive = true;
  let skip = false;

  const run = async () => {
    const acts = actions();
    while (alive && step() < acts.length) {
      skip = false;
      const r = await api.padCapture(p.pad.id, 15000).catch((e) => (fail(e), null));
      if (!alive) return;
      if (r === null) return void closeOv(p.oid);
      if (skip) continue;
      if (r.binding) {
        setMap({ ...map(), [acts[step()].id]: r.binding });
        sfx('tab');
        setStep(step() + 1);
        await new Promise((res) => setTimeout(res, 350)); // solta o botão antes do próximo
      }
    }
    if (alive) setDone(true);
  };

  const save = async () => {
    try {
      const r = await api.padSetMap(p.pad.id, map());
      sfx('on');
      toast('Mapeamento salvo', r.retroarch.length ? `${p.pad.name} · perfil criado no RetroArch` : p.pad.name, 'check');
      closeOv(p.oid);
    } catch (e) {
      fail(e);
    }
  };

  onMount(() => {
    setS('capturing', true); // o próprio controle não navega nos menus enquanto mapeia
    void Promise.all([api.padActions(), api.padMap(p.pad.id)]).then(([a, m]) => {
      setActions(a);
      setMap(m);
      void run();
    }).catch(fail);
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      if (a === 'skip') {
        skip = true;
        setStep(step() + 1);
        if (step() >= actions().length) setDone(true);
      } else if (a === 'save') void save();
      else if (a === 'cancel') closeOv(p.oid);
    };
    document.addEventListener('nexus:padmap', h);
    onCleanup(() => {
      alive = false;
      setS('capturing', false);
      document.removeEventListener('nexus:padmap', h);
    });
  });

  const cur = () => actions()[step()];
  return (
    <>
      <div class="stitle">Mapear botões</div>
      <div class="setpane wide pm">
        <p class="pane-sub">{p.pad.name} · {BUS[p.pad.bus]} — o mapa vale para os emuladores (RetroArch). Use o teclado ou o mouse para pular ou salvar.</p>
        <Show when={!done()} fallback={<div class="pm-now"><b>Pronto!</b><small>Confira a lista e salve.</small></div>}>
          <div class="pm-now">
            <small>{step() + 1} de {actions().length || 25}</small>
            <b>Aperte: {cur()?.label ?? '…'}</b>
            <div class="bar"><i style={{ width: `${(step() / (actions().length || 25)) * 100}%` }} /></div>
          </div>
        </Show>
        <div class="lacts pe-act">
          <Show when={!done()}><div class="btn f" data-k="pm-skip" data-r="p" data-pad="8" data-a="padmap:skip" data-verb="Pular"><Icon n="right" /> Pular</div></Show>
          <div class="btn pri f" data-k="pm-save" data-autofocus data-r="p" data-pad="8" data-a="padmap:save" data-verb="Salvar"><Icon n="check" /> Salvar</div>
          <div class="btn f" data-k="pm-cancel" data-r="p" data-pad="8" data-a="padmap:cancel" data-verb="Cancelar"><Icon n="x" /> Cancelar</div>
        </div>
        <div class="pm-grid">
          <For each={actions()}>
            {(a, i) => (
              <div class={`pm-row ${i() === step() && !done() ? 'on' : ''}`}><span>{a.label}</span><code>{fmt(map()[a.id])}</code></div>
            )}
          </For>
        </div>
      </div>
    </>
  );
}

// ---------- Cadastro de um controle: nome, jogador, mapear, excluir ----------
const PLAYERS: (number | null)[] = [null, 1, 2, 3, 4];

export function PadEditor(p: { pad: Pad; oid: number }) {
  const [pad, setPad] = createSignal<Pad>(p.pad);
  const save = async (patch: { nickname?: string; player?: number | null }, ok?: string) => {
    try {
      const r = await api.padSave(pad().id, patch);
      setPad({ ...pad(), ...r, registered: true });
      sfx('on');
      if (ok) toast(ok, r.nickname || r.name, 'check');
    } catch (e) {
      fail(e);
    }
  };
  onMount(() => {
    cycHandlers.set('pe:player', (d) => {
      const i = PLAYERS.indexOf(pad().player);
      void save({ player: PLAYERS[(i + d + PLAYERS.length) % PLAYERS.length] });
    });
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      if (a === 'name') void askText({ title: 'Nome do controle', value: pad().nickname || pad().name, max: 24 }).then((v) => {
          if (v !== null) void save({ nickname: v }, 'Nome salvo');
        });
      else if (a === 'player') cycHandlers.get('pe:player')?.(1);
      else if (a === 'register') void save({}, 'Controle cadastrado');
      else if (a === 'map') void pushOv('padmap', { pad: pad() }).then(() => setPad({ ...pad(), mapped: true, registered: true }));
      else if (a === 'delete')
        void askConfirm({ title: `Excluir ${pad().nickname || pad().name}?`, sub: 'O nome, o jogador e o mapeamento deste controle serão apagados.', yes: 'Excluir', danger: true }).then(async (ok) => {
          if (!ok) return;
          try {
            await api.padDelete(pad().id);
            toast('Controle excluído', pad().nickname || pad().name, 'trash');
            closeOv(p.oid);
          } catch (e) {
            fail(e);
          }
        });
      else if (a === 'done') closeOv(p.oid);
    };
    document.addEventListener('nexus:padedit', h);
    onCleanup(() => {
      cycHandlers.delete('pe:player');
      document.removeEventListener('nexus:padedit', h);
    });
  });
  return (
    <>
      <div class="stitle">Controle</div>
      <div class="setpane wide">
        <p class="pane-sub">{pad().name} · {pad().connected ? BUS[pad().bus] : 'desconectado'}</p>
        <div class="srow f" data-k="pd-name" data-autofocus data-r="22" data-a="padedit:name" data-verb="Editar">
          <div><b>Nome</b><small>{pad().nickname || 'Usar o nome do aparelho'}</small></div>
          <Icon n="gear" />
        </div>
        <div class="srow f" data-k="pd-player" data-r="22" data-cyc="pe:player" data-a="padedit:player" data-verb="Alterar">
          <div><b>Jogador</b><small>Nos emuladores, este controle fica sempre nessa posição (um controle por jogador)</small></div>
          <div class="cv"><Icon n="left" /><span>{pad().player ? `Jogador ${pad().player}` : 'Livre'}</span><Icon n="right" /></div>
        </div>
        <Show when={pad().connected} fallback={<div class="srow info"><div><b>Mapear botões</b><small>Conecte o controle para mapear</small></div></div>}>
          <div class="srow f" data-k="pd-map" data-r="22" data-a="padedit:map" data-verb="Mapear">
            <div><b>Mapear botões</b><small>{pad().mapped ? 'Mapeado · toque para refazer' : '25 ações: direcional, botões, gatilhos, analógicos e hotkey'}</small></div>
            <Icon n="pad" />
          </div>
        </Show>
        <div class="lacts pe-act">
          <div class="btn pri f" data-k="pd-done" data-r="p" data-pad="8" data-a="padedit:done" data-verb="Concluir"><Icon n="check" /> Concluir</div>
          <Show when={!pad().registered}><div class="btn f" data-k="pd-reg" data-r="p" data-pad="8" data-a="padedit:register" data-verb="Cadastrar"><Icon n="plus" /> Cadastrar</div></Show>
          <Show when={pad().registered}><div class="btn f" data-k="pd-del" data-r="p" data-pad="8" data-a="padedit:delete" data-verb="Excluir" style={{ color: 'var(--danger)' }}><Icon n="trash" /> Excluir cadastro</div></Show>
        </div>
      </div>
    </>
  );
}

export { S };
