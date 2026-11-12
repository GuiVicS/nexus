// Ajustes → Controles (B6): zona morta dos analógicos (guardada neste aparelho) e vibração de teste.
import { createSignal } from 'solid-js';

const KEY = 'nexus.deadzone';
const read = (): number => {
  try {
    const raw = localStorage.getItem(KEY);
    const v = raw === null ? NaN : Number(raw);
    return Number.isFinite(v) && v >= 0 && v <= 40 ? Math.round(v) : 10;
  } catch {
    return 10;
  }
};
/** Zona morta em % (0–40). Abaixo disso o analógico conta como parado na interface do Nexus. */
export const [deadzone, setDz] = createSignal(read());
export function setDeadzone(pct: number) {
  const v = Math.max(0, Math.min(40, Math.round(pct)));
  setDz(v);
  try {
    localStorage.setItem(KEY, String(v));
  } catch { /* sem armazenamento: vale só até recarregar */ }
}

export const firstPad = (): Gamepad | undefined => [...(navigator.getGamepads ? navigator.getGamepads() : [])].find((p): p is Gamepad => !!p);

/** Aplica a zona morta a um eixo (-1..1). */
export const withDeadzone = (v: number): number => (Math.abs(v) * 100 < deadzone() ? 0 : v);

/** Vibra o controle por um instante (não faz nada se o controle ou o navegador não suportam). */
export async function rumble(p = firstPad()): Promise<boolean> {
  const act = (p as unknown as { vibrationActuator?: { playEffect(t: string, o: object): Promise<string> } } | undefined)?.vibrationActuator;
  if (!act) return false;
  try {
    await act.playEffect('dual-rumble', { startDelay: 0, duration: 450, weakMagnitude: 0.6, strongMagnitude: 0.9 });
    return true;
  } catch {
    return false;
  }
}
