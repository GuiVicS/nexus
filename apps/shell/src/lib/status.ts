// Estado do aparelho (barra de status, Central e overlay no jogo). Vem do Core: uma leitura ao entrar e depois só
// o evento `status.changed` (o Core consulta rede, Bluetooth, som e disco numa tarefa só).
import { createSignal } from 'solid-js';
import type { StatusSnapshot } from '@nexus/protocol';
import { api, client } from './api';

const [status, setStatus] = createSignal<StatusSnapshot | null>(null);
export { status };

export const loadStatus = () => api.status().then(setStatus).catch(() => {});

let started = false;
export function startStatus() {
  if (started) return;
  started = true;
  client.on('status.changed', (s) => setStatus(s));
}
