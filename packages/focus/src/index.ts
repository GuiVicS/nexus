// Motor de foco espacial (navegação XY) para controle, teclado e ponteiro. Extraído do protótipo aprovado.
// Opera sobre o DOM: elementos focáveis têm a classe `f`; atributos data-* descrevem o comportamento.
//   data-r      raio do anel ("p" = pílula)   data-pad  folga do anel em px
//   data-cyc    cicla valor com ◀ ▶            data-slider  ajusta valor com ◀ ▶
//   data-group  ao entrar no grupo vindo de fora, o foco vai ao item .on dele

export type Dir = 'u' | 'd' | 'l' | 'r';

export interface FocusOptions {
  stage: HTMLElement;
  ring: HTMLElement;
  /** raiz atual de navegação (camada de cima ou tela ativa) */
  getRoot: () => HTMLElement | null;
  reduceMotion: () => boolean;
  /** chamado quando o foco muda; `silent` quando não deve tocar som */
  onFocus?: (el: HTMLElement, o: SetFocusOptions) => void;
  /** ◀ ▶ sobre um controle ajustável; devolve true se tratou */
  onAdjust?: (el: HTMLElement, dir: 1 | -1) => boolean;
  /** foco não mudou por falta de vizinho */
  onBump?: () => void;
  /** o elemento focado saiu do DOM (re-render); permite reposicionar o foco */
  onLost?: () => void;
}

export interface SetFocusOptions {
  silent?: boolean;
  snap?: boolean;
  still?: boolean;
  mouse?: boolean;
}

const visible = (e: Element) => e.getClientRects().length > 0;

export function focusables(root: ParentNode): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>('.f')].filter((e) => !e.hidden && visible(e));
}

/** Escolhe o vizinho na direção `d` a partir do retângulo `r`. Pura, para poder ser testada. */
export function pickNeighbor(d: Dir, r: DOMRect, candidates: { el: HTMLElement; rect: DOMRect }[], from?: HTMLElement): HTMLElement | null {
  const cx = r.left + r.width / 2;
  const cy = r.top + r.height / 2;
  let best: HTMLElement | null = null;
  let bs = 1e9;
  for (const { el, rect: q } of candidates) {
    if (el === from) continue;
    if (!q.width && !q.height) continue;
    const ex = q.left + q.width / 2;
    const ey = q.top + q.height / 2;
    let s: number;
    if (d === 'l' || d === 'r') {
      if (d === 'r' ? ex - cx <= 3 : cx - ex <= 3) continue;
      const p = Math.max(0, d === 'r' ? q.left - r.right : r.left - q.right);
      const o = Math.max(0, q.top - r.bottom, r.top - q.bottom);
      s = p + o * 4 + Math.abs(ey - cy) * 0.3;
    } else {
      if (d === 'd' ? ey - cy <= 3 : cy - ey <= 3) continue;
      const p = Math.max(0, d === 'd' ? q.top - r.bottom : r.top - q.bottom);
      const o = Math.max(0, q.left - r.right, r.left - q.right);
      s = p + o * 1.6 + Math.abs(ex - cx) * 0.12;
    }
    if (s < bs) {
      bs = s;
      best = el;
    }
  }
  return best;
}

export class Focus {
  cur: HTMLElement | null = null;
  private rg = { x: 0, y: 0, w: 0, h: 0, init: false };
  /** Onde estava o item focado (centro, em px de tela): se ele sumir (ex.: item removido da lista), o foco vai para o
   * vizinho mais próximo dali, e não para o primeiro item da tela (que poderia trocar de aba/categoria). */
  private lastAt: { x: number; y: number } | null = null;
  private raf = 0;

  private o: FocusOptions;

  constructor(o: FocusOptions) {
    this.o = o;
  }

  start() {
    const loop = () => {
      this.paintRing();
      this.raf = requestAnimationFrame(loop);
    };
    this.raf = requestAnimationFrame(loop);
  }

  stop() {
    cancelAnimationFrame(this.raf);
  }

  root(): HTMLElement | null {
    return this.o.getRoot();
  }

  private target() {
    const el = this.cur;
    if (!el || !el.isConnected) return null;
    const r = el.getBoundingClientRect();
    if (!r.width) return null;
    this.lastAt = { x: r.left + r.width / 2, y: r.top + r.height / 2 };
    const sr = this.o.stage.getBoundingClientRect();
    const s = sr.width / 1920;
    const pad = +(el.dataset.pad || 12);
    return { x: (r.left - sr.left) / s - pad, y: (r.top - sr.top) / s - pad, w: r.width / s + pad * 2, h: r.height / s + pad * 2 };
  }

  private paintRing() {
    const ring = this.o.ring;
    if (this.cur && !this.cur.isConnected) {
      this.cur = null;
      this.o.onLost?.();
    }
    // o anel fica acima de tudo: se o foco ficou numa camada de baixo (ex.: a linha atrás do teclado na tela,
    // antes de a camada nova receber o foco), não desenha
    const r = this.root();
    const t = r && this.cur && !r.contains(this.cur) ? null : this.target();
    if (!t) {
      ring.style.opacity = '0';
      this.rg.init = false;
      return;
    }
    const g = this.rg;
    const k = this.o.reduceMotion() || !g.init ? 1 : 0.32;
    g.x += (t.x - g.x) * k;
    g.y += (t.y - g.y) * k;
    g.w += (t.w - g.w) * k;
    g.h += (t.h - g.h) * k;
    g.init = true;
    ring.style.opacity = '1';
    ring.style.transform = `translate(${g.x.toFixed(1)}px,${g.y.toFixed(1)}px)`;
    ring.style.width = g.w.toFixed(1) + 'px';
    ring.style.height = g.h.toFixed(1) + 'px';
  }

  hideRing() {
    this.o.ring.style.opacity = '0';
    this.rg.init = false;
  }

  set(el: HTMLElement | null | undefined, o: SetFocusOptions = {}) {
    if (this.cur) this.cur.classList.remove('focus');
    this.cur = el ?? null;
    if (!el) return;
    el.classList.add('focus');
    const ring = this.o.ring;
    ring.style.borderColor = el.classList.contains('pri') ? 'var(--ringpri)' : '';
    const rr = el.dataset.r;
    const pad = +(el.dataset.pad || 12);
    ring.style.borderRadius = rr === 'p' ? '999px' : +(rr || 22) + pad + 'px';
    if (o.snap) this.rg.init = false;
    if (!o.mouse && !o.still) el.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: this.o.reduceMotion() ? 'auto' : 'smooth' });
    this.o.onFocus?.(el, o);
  }

  /** Foca o primeiro elemento da raiz atual (ou do `scope`). */
  first(scope?: ParentNode | null, o: SetFocusOptions = { silent: true, snap: true }) {
    const r = scope ?? this.root();
    if (!r) return;
    this.set(r.querySelector<HTMLElement>('.f[data-autofocus]') ?? focusables(r)[0], o);
  }

  /** O focável da raiz mais perto do lugar onde o foco estava (null se não houver lugar guardado). */
  private nearest(r: ParentNode): HTMLElement | null {
    const at = this.lastAt;
    if (!at) return null;
    let best: HTMLElement | null = null;
    let bd = Infinity;
    for (const el of focusables(r)) {
      const b = el.getBoundingClientRect();
      if (!b.width) continue;
      const dx = Math.max(b.left - at.x, 0, at.x - b.right);
      const dy = Math.max(b.top - at.y, 0, at.y - b.bottom);
      const d = dx * 2 + dy; // prefere ficar na mesma coluna (mesmo painel)
      if (d < bd) {
        bd = d;
        best = el;
      }
    }
    return best;
  }

  /** Garante que o foco esteja em algo visível da raiz atual. Devolve true se precisou reposicionar. */
  ensure(): boolean {
    const r = this.root();
    if (!r) return false;
    if (this.cur && this.cur.isConnected && r.contains(this.cur) && visible(this.cur)) return false;
    // o item focado sumiu desta mesma camada: vizinho mais próximo; camada nova: o primeiro
    const lost = !this.cur || !this.cur.isConnected;
    const n = lost && !r.querySelector('.f[data-autofocus]') ? this.nearest(r) : null;
    if (n) this.set(n, { silent: true, still: true });
    else this.first(r);
    this.lastAt = null;
    return true;
  }

  move(d: Dir) {
    const L = this.root();
    if (!L) return;
    const cur = this.cur;
    if (!cur || !cur.isConnected || !L.contains(cur) || !visible(cur)) {
      this.ensure();
      return;
    }
    if ((cur.dataset.cyc || cur.dataset.slider) && (d === 'l' || d === 'r')) {
      if (this.o.onAdjust?.(cur, d === 'r' ? 1 : -1)) return;
    }
    const cands = focusables(L).map((el) => ({ el, rect: el.getBoundingClientRect() }));
    let best = pickNeighbor(d, cur.getBoundingClientRect(), cands, cur);
    // grupo (data-group, ex.: as abas em pílula da Loja): quem entra de fora cai no item ativo (.on), não no vizinho geométrico
    const g = best?.dataset.group;
    if (best && g && cur.dataset.group !== g) best = L.querySelector<HTMLElement>(`.f.on[data-group="${g}"]`) ?? best;
    if (best) this.set(best);
    else this.o.onBump?.();
  }

  page(dir: 1 | -1) {
    const inRow = !!this.cur?.closest('.drow');
    const d: Dir = inRow ? (dir > 0 ? 'r' : 'l') : dir > 0 ? 'd' : 'u';
    for (let i = 0; i < (inRow ? 4 : 3); i++) this.move(d);
  }
}
