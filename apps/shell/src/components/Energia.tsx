// Ajustes → Energia (B10): suspender/reiniciar/desligar, apagar a tela e suspender por inatividade (o Core cuida do
// swayidle e o desliga durante jogos), o que o botão físico de energia faz e o estado de "abrir direto no Nexus".
import { createSignal, onCleanup, onMount, Show } from 'solid-js';
import type { PowerStatus } from '@nexus/protocol';
import { sfx } from '@nexus/ui';
import { api } from '../lib/api';
import { cycHandlers } from '../lib/actions';
import { fail, guarded } from '../lib/state';
import { Icon } from './ui';

const KEY_LABEL: Record<string, [string, string]> = {
  ask: ['Perguntar', 'Mostra “Suspender” ou “Desligar”'],
  suspend: ['Suspender', 'Dorme na hora'],
  off: ['Desligar', 'Desliga na hora'],
  nothing: ['Nada', 'O botão não faz nada (evita desligar sem querer)'],
};
const KEYS = ['ask', 'suspend', 'off', 'nothing'];
const mins = (m: number) => (m === 0 ? 'Nunca' : m < 60 ? `${m} min` : `${m / 60} h`);

export function PowerPanel() {
  const [st, setSt] = createSignal<PowerStatus | null>(null);
  const set = async (p: { screenMin?: number; suspendMin?: number; key?: string }) => {
    try {
      const r = await guarded((a) => api.powerSet({ ...p, ...a }), 'Conta da casa: PIN dos responsáveis');
      if (r) {
        setSt(r);
        sfx('tick');
      }
    } catch (e) {
      fail(e);
    }
  };
  const step = (opts: number[], cur: number, d: number) => opts[Math.max(0, Math.min(opts.length - 1, opts.indexOf(cur) + d))];
  onMount(() => {
    void api.powerStatus().then(setSt).catch(fail);
    cycHandlers.set('pw:screen', (d) => {
      const s = st();
      if (s) void set({ screenMin: step(s.screenOpts, s.screenMin, d) });
    });
    cycHandlers.set('pw:suspend', (d) => {
      const s = st();
      if (s) void set({ suspendMin: step(s.suspendOpts, s.suspendMin, d) });
    });
    cycHandlers.set('pw:key', (d) => {
      const s = st();
      if (s) void set({ key: KEYS[(KEYS.indexOf(s.key) + d + KEYS.length) % KEYS.length] });
    });
    onCleanup(() => ['pw:screen', 'pw:suspend', 'pw:key'].forEach((k) => cycHandlers.delete(k)));
  });
  const cv = (k: string, label: string, desc: string, value: string) => (
    <div class="srow f" data-k={`pw-${k}`} data-r="22" data-cyc={`pw:${k}`} data-a={`cyc:pw:${k}`} data-verb="Alterar">
      <div><b>{label}</b><small>{desc}</small></div>
      <div class="cv"><Icon n="left" /><span>{value}</span><Icon n="right" /></div>
    </div>
  );
  return (
    <>
      <div class="srow f" data-k="pw-suspend-now" data-r="22" data-a="suspend" data-verb="Suspender"><div><b>Suspender agora</b><small>O console dorme; o jogo aberto continua de onde parou</small></div><Icon n="moon" /></div>
      <div class="srow f" data-k="pw-restart" data-r="22" data-a="restart" data-verb="Reiniciar"><div><b>Reiniciar</b><small>Pede confirmação</small></div><Icon n="refresh" /></div>
      <div class="srow f" data-k="pw-off" data-r="22" data-a="off" data-verb="Desligar" style={{ color: 'var(--danger)' }}><div><b>Desligar</b><small>Pede confirmação</small></div><Icon n="power" /></div>
      <Show when={st()} fallback={<div class="srow info"><div><b>Carregando…</b></div></div>}>
        {(s) => (
          <>
            <h3 class="sh2">Quando ninguém usa</h3>
            <Show when={!s().idleAvailable}>
              <div class="srow info" data-k="pw-noidle"><div><b>Temporizadores indisponíveis</b><small>O programa swayidle não está instalado. Reinstale o Nexus para ativá-los.</small></div></div>
            </Show>
            {cv('screen', 'Apagar a tela após', 'Sem mexer no controle, a TV fica sem imagem (não vale durante jogos)', mins(s().screenMin))}
            {cv('suspend', 'Suspender após', 'Depois disso o console dorme (não vale durante jogos)', mins(s().suspendMin))}
            <h3 class="sh2">Botão de energia</h3>
            {cv('key', 'Ao apertar o botão do aparelho', KEY_LABEL[s().key][1], KEY_LABEL[s().key][0])}
            <h3 class="sh2">Ao ligar</h3>
            <div class="srow info" data-k="pw-auto"><div><b>Abrir direto no Nexus: {s().autologin.enabled ? 'ativado' : 'desativado'}</b><small>{s().autologin.enabled ? `Ao ligar, o aparelho entra no Nexus sozinho (${s().autologin.dm}). Para desligar, use Ajustes → Recuperação.` : 'O aparelho pede o login do sistema antes de abrir o Nexus.'}</small></div></div>
          </>
        )}
      </Show>
    </>
  );
}
