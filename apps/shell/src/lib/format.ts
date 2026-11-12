import type { Item } from '@nexus/protocol';

export const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
export const mono = (t: string) => t.replace(/[^\p{L}\p{N} ]/gu, '').split(/\s+/).filter(Boolean).slice(0, 2).map((w) => w[0]).join('').toUpperCase();

export function fmtBytes(n?: number | null) {
  if (!n) return '—';
  const u = [['TB', 1e12], ['GB', 1e9], ['MB', 1e6], ['KB', 1e3]] as const;
  for (const [l, v] of u) if (n >= v) return `${(n / v).toFixed(n / v >= 100 ? 0 : 1).replace('.', ',')} ${l}`;
  return `${n} B`;
}

export function fmtSpeed(bps?: number) {
  return bps ? `${fmtBytes(bps)}/s` : '—';
}

export function fmtEta(sec?: number | null) {
  if (sec === null || sec === undefined) return '—';
  if (sec < 60) return `${sec} s`;
  if (sec < 3600) return `${Math.round(sec / 60)} min`;
  return `${Math.floor(sec / 3600)} h ${Math.round((sec % 3600) / 60)} min`;
}

export function fmtPlaytime(sec: number) {
  if (!sec) return 'Ainda não jogou';
  if (sec < 3600) return `${Math.max(1, Math.round(sec / 60))} min jogados`;
  return `${(sec / 3600).toFixed(sec >= 36000 ? 0 : 1).replace('.', ',')} h jogadas`;
}

export function fmtAgo(unix?: number | null) {
  if (!unix) return '—';
  const d = Math.floor((Date.now() / 1000 - unix) / 86400);
  if (d <= 0) return 'hoje';
  if (d === 1) return 'ontem';
  if (d < 30) return `há ${d} dias`;
  if (d < 365) return `há ${Math.floor(d / 30)} ${Math.floor(d / 30) === 1 ? 'mês' : 'meses'}`;
  return `há ${Math.floor(d / 365)} ano(s)`;
}

export const ratingLabel = (r?: number | null) => (r === null || r === undefined ? '' : r === 0 ? 'Livre' : `${r} anos`);

export function metaOf(it: Item) {
  if (it.kind === 'app') return it.tags.includes('media') ? it.subtitle || it.category || 'Mídia' : it.category || it.subtitle || 'App';
  return `${it.subtitle}${it.year ? ' · ' + it.year : ''}`;
}

export const priceNum = (s?: string | null) => (!s || s === 'Grátis' ? 0 : parseFloat(String(s).replace(/[^\d,]/g, '').replace(',', '.')) || 0);
export const money = (n: number) => `R$ ${n.toFixed(2).replace('.', ',')}`;
export const effectivePrice = (it: Item) => it.offer?.new ?? it.price ?? null;
