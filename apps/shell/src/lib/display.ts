// Margem da tela (overscan): a interface inteira encolhe por igual para caber em TVs que cortam as bordas.
// O valor é do aparelho (Core, `display.margin`), não do perfil, e vale desde a tela de perfis.
import { createSignal } from 'solid-js';
import { api, client } from './api';

const [margin, setMarginSig] = createSignal(0);
export { margin };

export function fitStage() {
  const m = margin();
  const s = Math.min(innerWidth / 1920, innerHeight / 1080) * (1 - (2 * m) / 100);
  document.documentElement.style.setProperty('--s', String(s));
}

/** Só na tela (prévia do assistente); não salva. */
export function previewMargin(m: number) {
  setMarginSig(Math.max(0, Math.min(10, Math.round(m))));
  fitStage();
}

export function loadMargin() {
  void api.displayStatus().then((st) => previewMargin(st.margin)).catch(() => {});
}

client.on('display.margin', (p: { margin: number }) => previewMargin(p.margin));
