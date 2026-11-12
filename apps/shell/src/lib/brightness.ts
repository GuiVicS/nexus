// Brilho da tela (só quando há backlight; TV/monitor por HDMI não têm). Muda na hora na tela e vai ao Core depois de
// 150 ms parado, como o volume.
import { createSignal } from 'solid-js';
import { sfx } from '@nexus/ui';
import { api } from './api';
import { cycHandlers } from './actions';
import { fail } from './state';

const [bright, setBright] = createSignal<{ available: boolean; percent: number } | null>(null);
export { bright };

export const loadBrightness = () => api.brightnessStatus().then(setBright).catch(() => setBright({ available: false, percent: 0 }));

let t: ReturnType<typeof setTimeout> | undefined;
export function brightStep(delta: number) {
  const b = bright();
  if (!b?.available) return;
  const v = Math.max(5, Math.min(100, b.percent + delta));
  setBright({ ...b, percent: v });
  sfx('tick');
  clearTimeout(t);
  t = setTimeout(() => void api.brightnessSet(v).catch(fail), 150);
}
cycHandlers.set('brightness', (d) => brightStep(d * 5));
