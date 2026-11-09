// Sons de interface gerados com Web Audio (sem arquivos). Portado do protótipo aprovado.
type ToneOpts = { type?: OscillatorType; vol?: number; to?: number | null; attack?: number };
type NoiseOpts = { vol?: number; f?: number; to?: number | null; q?: number; type?: BiquadFilterType };

let AC: AudioContext | null = null;
let MASTER: GainNode;
let NOISE: AudioBuffer;
let enabled = true;
let volume = 60;

export function configureSfx(o: { enabled?: boolean; volume?: number }) {
  if (o.enabled !== undefined) enabled = o.enabled;
  if (o.volume !== undefined) volume = o.volume;
}

export function audioInit() {
  if (AC) {
    if (AC.state === 'suspended') void AC.resume();
    return;
  }
  try {
    const Ctor = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
    AC = new Ctor();
    MASTER = AC.createGain();
    MASTER.gain.value = volume / 100;
    const comp = AC.createDynamicsCompressor();
    MASTER.connect(comp).connect(AC.destination);
    const d = AC.createDelay(), fb = AC.createGain(), wet = AC.createGain(), lp = AC.createBiquadFilter();
    d.delayTime.value = 0.09; fb.gain.value = 0.25; wet.gain.value = 0.2; lp.frequency.value = 2600;
    MASTER.connect(d); d.connect(lp).connect(fb).connect(d); lp.connect(wet).connect(comp);
    NOISE = AC.createBuffer(1, AC.sampleRate * 2, AC.sampleRate);
    const ch = NOISE.getChannelData(0);
    for (let i = 0; i < ch.length; i++) ch[i] = Math.random() * 2 - 1;
  } catch {
    AC = null;
  }
}

function tone(f: number, t0: number, dur: number, { type = 'sine', vol = 0.2, to = null, attack = 0.004 }: ToneOpts = {}) {
  const o = AC!.createOscillator(), g = AC!.createGain();
  o.type = type; o.frequency.setValueAtTime(f, t0);
  if (to) o.frequency.exponentialRampToValueAtTime(to, t0 + dur);
  g.gain.setValueAtTime(0, t0); g.gain.linearRampToValueAtTime(vol, t0 + attack); g.gain.exponentialRampToValueAtTime(0.0001, t0 + dur);
  o.connect(g).connect(MASTER); o.start(t0); o.stop(t0 + dur + 0.05);
}

function noise(t0: number, dur: number, { vol = 0.15, f = 2000, to = null, q = 1, type = 'bandpass' }: NoiseOpts = {}) {
  const src = AC!.createBufferSource(), bf = AC!.createBiquadFilter(), g = AC!.createGain();
  src.buffer = NOISE; bf.type = type; bf.Q.value = q; bf.frequency.setValueAtTime(f, t0);
  if (to) bf.frequency.exponentialRampToValueAtTime(to, t0 + dur);
  g.gain.setValueAtTime(0, t0); g.gain.linearRampToValueAtTime(vol, t0 + dur * 0.3); g.gain.exponentialRampToValueAtTime(0.0001, t0 + dur);
  src.connect(bf).connect(g).connect(MASTER); src.start(t0); src.stop(t0 + dur + 0.05);
}

const SFX = {
  move: (t: number) => { tone(1900, t, 0.035, { vol: 0.06 }); noise(t, 0.025, { vol: 0.05, f: 5200, q: 2 }); },
  tab: (t: number) => { noise(t, 0.15, { vol: 0.08, f: 700, to: 3200, q: 1.2 }); tone(660, t, 0.09, { vol: 0.045, to: 990 }); },
  select: (t: number) => { tone(880, t, 0.09, { vol: 0.11, type: 'triangle' }); tone(1320, t + 0.05, 0.16, { vol: 0.09 }); },
  back: (t: number) => { tone(740, t, 0.09, { vol: 0.08, type: 'triangle', to: 494 }); noise(t, 0.06, { vol: 0.035, f: 1500 }); },
  open: (t: number) => { noise(t, 0.3, { vol: 0.09, f: 380, to: 2600, q: 0.8 }); tone(523, t + 0.04, 0.24, { vol: 0.05 }); tone(784, t + 0.1, 0.28, { vol: 0.045 }); },
  close: (t: number) => { noise(t, 0.22, { vol: 0.07, f: 2600, to: 380, q: 0.8 }); tone(587, t, 0.12, { vol: 0.04, to: 440 }); },
  deny: (t: number) => { tone(185, t, 0.08, { type: 'square', vol: 0.04 }); tone(155, t + 0.1, 0.12, { type: 'square', vol: 0.04 }); },
  bump: (t: number) => tone(240, t, 0.07, { type: 'triangle', vol: 0.07, to: 170 }),
  key: (t: number) => { noise(t, 0.03, { vol: 0.08, f: 3600, q: 3 }); tone(1250, t, 0.03, { vol: 0.03 }); },
  on: (t: number) => { tone(988, t, 0.05, { vol: 0.08, type: 'triangle' }); tone(1480, t + 0.045, 0.08, { vol: 0.07 }); },
  off: (t: number) => { tone(1480, t, 0.05, { vol: 0.07, type: 'triangle' }); tone(988, t + 0.045, 0.08, { vol: 0.06 }); },
  tick: (t: number) => tone(2600, t, 0.02, { vol: 0.05 }),
  notify: (t: number) => { [1047, 1319, 1568].forEach((f, i) => tone(f, t + i * 0.07, 0.4, { vol: 0.055 })); },
  achieve: (t: number) => { [784, 988, 1175, 1568].forEach((f, i) => tone(f, t + i * 0.08, 0.6, { vol: 0.07, type: 'triangle' })); noise(t + 0.3, 0.7, { vol: 0.035, f: 6000, q: 0.5, type: 'highpass' }); },
  launch: (t: number) => { noise(t, 1.3, { vol: 0.09, f: 180, to: 5200, q: 0.7 }); tone(220, t, 1.3, { vol: 0.05, to: 880, type: 'triangle', attack: 0.5 }); tone(330, t + 0.2, 1.1, { vol: 0.04, to: 1320, attack: 0.5 }); },
  boot: (t: number) => { [130.8, 261.6, 392, 523.3, 659.3, 784].forEach((f, i) => tone(f, t + i * 0.13, 2.8 - i * 0.25, { vol: i ? 0.06 : 0.09, type: i % 2 ? 'triangle' : 'sine', attack: 0.3 })); noise(t, 2, { vol: 0.05, f: 250, to: 4500, q: 0.6 }); },
  power: (t: number) => { [784, 523.3, 392, 261.6].forEach((f, i) => tone(f, t + i * 0.11, 0.7, { vol: 0.06, type: 'triangle' })); },
  profile: (t: number) => { tone(659, t, 0.3, { vol: 0.07 }); tone(988, t + 0.08, 0.4, { vol: 0.06 }); tone(1319, t + 0.16, 0.6, { vol: 0.05 }); },
  pad: (t: number) => tone(1320, t, 0.08, { vol: 0.05 }),
} as const;

export type SfxName = keyof typeof SFX;

export function sfx(n: SfxName) {
  if (!enabled || !AC || !SFX[n]) return;
  try {
    MASTER.gain.value = volume / 100;
    SFX[n](AC.currentTime + 0.005);
  } catch {
    /* sem áudio */
  }
}
