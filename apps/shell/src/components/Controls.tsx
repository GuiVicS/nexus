// Ajustes → Controles (B6): teste ao vivo de botões e analógicos, zona morta, vibração de teste e ajuda para
// controle sem suporte. Tudo na Shell (Gamepad API); o mapa de botões dos emuladores continua em Pads.tsx.
import { createSignal, For, onCleanup, onMount, Show } from 'solid-js';
import { sfx } from '@nexus/ui';
import { cycHandlers } from '../lib/actions';
import { deadzone, firstPad, rumble, setDeadzone } from '../lib/controls';
import { closeOv, pushOv, setS, toast } from '../lib/state';
import { Icon } from './ui';

const BTN: [number, string][] = [
  [0, 'A'], [1, 'B'], [2, 'X'], [3, 'Y'], [4, 'LB'], [5, 'RB'], [6, 'LT'], [7, 'RT'],
  [8, 'Select'], [9, 'Start'], [10, 'L3'], [11, 'R3'], [12, '↑'], [13, '↓'], [14, '←'], [15, '→'], [16, 'Guia'],
];

/** Tela cheia: mostra ao vivo cada botão e analógico do primeiro controle conectado. */
export function PadTest(p: { oid: number }) {
  const [pad, setPad] = createSignal<{ name: string; b: number[]; ax: number[] } | null>(null);
  const [last, setLast] = createSignal('');
  let raf = 0;
  let holdB = 0;
  let prevSel = false;
  const vib = async () => {
    const ok = await rumble();
    toast(ok ? 'Vibração enviada' : 'Este controle não vibra', ok ? 'Sentiu? Se não, o controle ou a conexão não suportam' : 'Alguns controles e conexões não têm motor de vibração', ok ? 'pad' : 'info');
  };
  const tick = (t: number) => {
    const g = firstPad();
    if (g) {
      const b = g.buttons.map((x) => Math.round((x.value || (x.pressed ? 1 : 0)) * 100));
      setPad({ name: g.id.replace(/\(.*?\)/g, '').trim() || 'Controle', b, ax: [...g.axes].map((v) => Math.round(v * 100)) });
      const names = BTN.filter(([i]) => g.buttons[i]?.pressed).map(([, n]) => n);
      if (names.length) setLast(names.join(' + '));
      // sem navegar nos menus durante o teste: segurar B por 1 s sai; Select faz vibrar
      if (g.buttons[1]?.pressed) {
        holdB ||= t;
        if (t - holdB > 1000) return void closeOv(p.oid);
      } else holdB = 0;
      const sel = !!g.buttons[8]?.pressed;
      if (sel && !prevSel) void vib();
      prevSel = sel;
    } else setPad(null);
    raf = requestAnimationFrame(tick);
  };
  onMount(() => {
    setS('capturing', true);
    raf = requestAnimationFrame(tick);
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      if (a === 'vib') void vib();
      else if (a === 'done') closeOv(p.oid);
    };
    document.addEventListener('nexus:ctl', h);
    onCleanup(() => {
      cancelAnimationFrame(raf);
      // só libera a navegação quando todos os botões soltarem (senão um botão segurado navegaria nos Ajustes)
      const free = (n: number) => (firstPad()?.buttons.some((b) => b.pressed) && n < 40 ? void setTimeout(() => free(n + 1), 100) : setS('capturing', false));
      free(0);
      document.removeEventListener('nexus:ctl', h);
    });
  });
  const dz = () => deadzone();
  const stick = (x: number, y: number) => {
    const m = Math.hypot(x, y);
    return { style: { left: `${50 + x / 2}%`, top: `${50 + y / 2}%` }, dead: m < dz() ? '1' : '0' };
  };
  return (
    <>
      <div class="stitle">Testar controle</div>
      <div class="setpane wide pt" data-pt>
        <Show when={pad()} fallback={<div class="pm-now" data-k="pt-none"><b>Nenhum controle detectado</b><small>Conecte por USB ou pareie por Bluetooth e aperte qualquer botão. Alguns controles só aparecem depois do primeiro aperto.</small></div>}>
          {(g) => (
            <>
              <p class="pane-sub" data-k="pt-name">{g().name} · Último: <b data-k="pt-last">{last() || '—'}</b></p>
              <div class="pt-btns">
                <For each={BTN}>{([i, n]) => <span class={`pt-b ${(g().b[i] ?? 0) > 50 ? 'on' : ''}`} data-b={n}>{n}</span>}</For>
              </div>
              <div class="pt-sticks">
                <div class="pt-stick" data-k="pt-l"><div class="pt-dead" style={{ width: `${dz()}%`, height: `${dz()}%` }} /><i style={stick(g().ax[0] ?? 0, g().ax[1] ?? 0).style} data-dead={stick(g().ax[0] ?? 0, g().ax[1] ?? 0).dead} /><small>Analógico esquerdo {g().ax[0] ?? 0}, {g().ax[1] ?? 0}</small></div>
                <div class="pt-trig">
                  <small>LT {g().b[6] ?? 0}%</small><div class="tr"><i style={{ width: `${g().b[6] ?? 0}%` }} /></div>
                  <small>RT {g().b[7] ?? 0}%</small><div class="tr"><i style={{ width: `${g().b[7] ?? 0}%` }} /></div>
                </div>
                <div class="pt-stick" data-k="pt-r"><div class="pt-dead" style={{ width: `${dz()}%`, height: `${dz()}%` }} /><i style={stick(g().ax[2] ?? 0, g().ax[3] ?? 0).style} data-dead={stick(g().ax[2] ?? 0, g().ax[3] ?? 0).dead} /><small>Analógico direito {g().ax[2] ?? 0}, {g().ax[3] ?? 0}</small></div>
              </div>
            </>
          )}
        </Show>
        <p class="pane-sub">Segure <b>B</b> por 1 segundo para sair · <b>Select</b> faz o controle vibrar · o círculo cinza é a zona morta ({dz()}%)</p>
        <div class="lacts pe-act">
          <div class="btn f" data-k="pt-vib" data-r="p" data-a="ctl:vib" data-verb="Vibrar"><Icon n="pad" /> Vibrar</div>
          <div class="btn pri f" data-k="pt-done" data-autofocus data-r="p" data-a="ctl:done" data-verb="Concluir"><Icon n="check" /> Concluir</div>
        </div>
      </div>
    </>
  );
}

const HELP = [
  ['Controle de Xbox, PlayStation, Switch Pro, 8BitDo', 'Funcionam sozinhos por USB ou Bluetooth. Para parear, use Ajustes → Bluetooth.'],
  ['O controle não aparece nesta lista', 'Troque o cabo (alguns só carregam), tente outra porta USB e aperte um botão. No Bluetooth, segure o botão de pareamento até a luz piscar rápido.'],
  ['Aparece, mas os botões estão trocados', 'Use “Mapear botões” no controle, em Controles conectados: o mapa vale para os emuladores.'],
  ['Controle genérico ou sem marca', 'Muitos usam o modo “X-input”. Procure no manual como mudar de modo (normalmente segurar Home ou Start+Y ao ligar).'],
  ['Nada funciona', 'Use o celular como controle (Ajustes → Controle no celular) e peça ajuda no repositório do Nexus informando o nome do controle.'],
];

/** Linhas extras de Ajustes → Controles. */
export function ControlsExtras() {
  onMount(() => {
    cycHandlers.set('ctl:dz', (d) => {
      setDeadzone(deadzone() + d * 2);
      sfx('tab');
    });
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      if (a === 'test') void pushOv('padtest', {});
      else if (a === 'vib') void rumble().then((ok) => toast(ok ? 'Vibração enviada' : 'Nenhum controle vibrou', ok ? '' : 'Conecte um controle com vibração e tente de novo', ok ? 'pad' : 'info'));
    };
    document.addEventListener('nexus:ctl', h);
    onCleanup(() => {
      cycHandlers.delete('ctl:dz');
      document.removeEventListener('nexus:ctl', h);
    });
  });
  return (
    <>
      <h3 class="sh2">Testar</h3>
      <div class="srow f" data-k="ctl-test" data-r="22" data-a="ctl:test" data-verb="Testar"><div><b>Testar botões e analógicos</b><small>Mostra ao vivo cada botão, gatilho e analógico do controle conectado</small></div><Icon n="pad" /></div>
      <div class="srow f" data-k="ctl-vib" data-r="22" data-a="ctl:vib" data-verb="Vibrar"><div><b>Vibração de teste</b><small>Faz o controle vibrar por meio segundo</small></div><Icon n="play" /></div>
      <div class="srow f slider" data-k="ctl-dz" data-r="22" data-slider="ctl:dz" data-a="noop" data-verb="Ajustar">
        <div><b>Zona morta dos analógicos</b><small>Se o cursor anda sozinho, aumente. Vale na interface do Nexus.</small></div><div class="tr"><i style={{ width: `${(deadzone() / 40) * 100}%` }} /></div><span class="v">{deadzone()}%</span>
      </div>
      <h3 class="sh2">Controle sem suporte?</h3>
      <For each={HELP}>{([t, d], i) => <div class="srow info" data-k={`ctl-help-${i()}`}><div><b>{t}</b><small>{d}</small></div></div>}</For>
    </>
  );
}
