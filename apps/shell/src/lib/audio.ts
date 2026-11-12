// Som do sistema (PipeWire, via Core): estado compartilhado entre a Central rápida, os Ajustes → Áudio e a barra.
// O volume muda na tela na hora (otimista) e vai para o Core depois de 120 ms parado, para não chamar o `wpctl` a cada
// toque no D-pad.
import { createSignal } from 'solid-js';
import type { AudioStatus } from '@nexus/protocol';
import { sfx } from '@nexus/ui';
import { api, client } from './api';
import { cycHandlers } from './actions';
import { fail, toast } from './state';

const [audio, setAudio] = createSignal<AudioStatus | null>(null);
export { audio };

let seq = 0;
let lastLocal = 0; // mudança feita aqui: o eco do Core logo depois não deve desfazer passos novos
export const loadAudio = () =>
  api.audioStatus().then(setAudio).catch(() => setAudio((a) => a ?? { available: false, error: 'Som indisponível', volume: 0, muted: false, sinks: [], sources: [], mic: null }));

/** Aplica no Core e só aceita a resposta da última mudança (respostas antigas não fazem o slider voltar). */
export async function audioSet(p: Parameters<typeof api.audioSet>[0]) {
  const my = ++seq;
  try {
    const r = await api.audioSet(p);
    if (my === seq) setAudio(r);
  } catch (e) {
    fail(e);
    if (my === seq) void loadAudio();
  }
}

let volT: ReturnType<typeof setTimeout> | undefined;
let volPending: number | null = null;
let micT: ReturnType<typeof setTimeout> | undefined;
export function sysVolStep(delta: number) {
  const a = audio();
  if (!a?.available) return;
  const v = Math.max(0, Math.min(100, a.volume + delta));
  lastLocal = Date.now();
  setAudio({ ...a, volume: v, muted: v === 0 ? a.muted : false });
  sfx('tick');
  ++seq;
  clearTimeout(volT);
  volPending = v;
  volT = setTimeout(() => {
    volPending = null;
    void audioSet({ volume: v });
  }, 120);
}
export function micVolStep(delta: number) {
  const a = audio();
  if (!a?.mic) return;
  const v = Math.max(0, Math.min(100, a.mic.volume + delta));
  setAudio({ ...a, mic: { ...a.mic, volume: v } });
  sfx('tick');
  ++seq;
  clearTimeout(micT);
  micT = setTimeout(() => void audioSet({ source: true, volume: v }), 120);
}

cycHandlers.set('sysvol', (d) => sysVolStep(d * 5));
cycHandlers.set('micvol', (d) => micVolStep(d * 5));
/** Central rápida: ←/→ troca a saída de som (TV, analógica, fone Bluetooth…). */
cycHandlers.set('gout', (d) => {
  const a = audio();
  if (!a?.available || a.sinks.length < 2) return;
  const i = Math.max(0, a.sinks.findIndex((s) => s.default));
  const n = a.sinks[(i + d + a.sinks.length) % a.sinks.length];
  sfx('tab');
  void audioSet({ default: n.id }).then(() => toast('Saída de som', n.name, 'vol'));
});
client.on('audio.changed', (p: { volume: number; muted: boolean }) => {
  if (Date.now() - lastLocal < 800) return;
  setAudio((a) => (a ? { ...a, volume: p.volume, muted: p.muted } : a));
});

// mudo vale em qualquer lugar (Central rápida, Ajustes)
document.addEventListener('nexus:audio', (e) => {
  const a = audio();
  const d = (e as CustomEvent<string>).detail;
  if (d === 'mute' && a?.available) {
    // volume ainda não enviado vai junto (senão ele chegaria depois e tiraria o mudo)
    clearTimeout(volT);
    const v = volPending;
    volPending = null;
    void audioSet(v === null ? { muted: !a.muted } : { volume: v, muted: !a.muted });
  }
  else if (d === 'micmute' && a?.mic) void audioSet({ source: true, muted: !a.mic.muted });
});
