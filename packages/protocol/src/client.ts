// Cliente JSON-RPC 2.0 com eventos. O transporte é plugável: WebSocket (dev/testes) ou a ponte do Electron.

export class RpcError extends Error {
  code: number;
  data: unknown;
  constructor(code: number, message: string, data?: unknown) {
    super(message);
    this.code = code;
    this.data = data;
  }
}

/** Códigos de erro de aplicação do Core (espelham core/crates/nexus-core/src/rpc.rs) */
export const E = {
  PIN_REQUIRED: 1001,
  PIN_INVALID: 1002,
  LOCKED: 1003,
  FORBIDDEN: 1004,
  BUSY: 1005,
  NOT_FOUND: 1006,
  LIMIT: 1007,
  UNSUPPORTED: 1008,
  NO_SESSION: 1009,
  PARENTAL_PIN: 1010,
  FAILED: 1011,
} as const;

export interface Transport {
  send(text: string): void;
  close(): void;
  onmessage: ((text: string) => void) | null;
  onstatus: ((up: boolean) => void) | null;
}

export function webSocketTransport(url: string): Transport {
  let ws: WebSocket | null = null;
  let closed = false;
  let delay = 300;
  const t: Transport = {
    onmessage: null,
    onstatus: null,
    send: (text) => {
      if (ws && ws.readyState === WebSocket.OPEN) ws.send(text);
    },
    close: () => {
      closed = true;
      ws?.close();
    },
  };
  const connect = () => {
    ws = new WebSocket(url);
    ws.onopen = () => {
      delay = 300;
      t.onstatus?.(true);
    };
    ws.onmessage = (e) => t.onmessage?.(String(e.data));
    ws.onclose = () => {
      t.onstatus?.(false);
      if (!closed) setTimeout(connect, (delay = Math.min(delay * 1.7, 4000)));
    };
    ws.onerror = () => ws?.close();
  };
  connect();
  return t;
}

type Handler = (params: any) => void;

export class RpcClient {
  private id = 0;
  private pending = new Map<number, { ok: (v: any) => void; err: (e: RpcError) => void; timer: ReturnType<typeof setTimeout> }>();
  private handlers = new Map<string, Set<Handler>>();
  private statusHandlers = new Set<(up: boolean) => void>();
  private waiters: (() => void)[] = [];
  up = false;
  private t: Transport;

  constructor(t: Transport) {
    this.t = t;
    t.onmessage = (text) => this.receive(text);
    t.onstatus = (up) => {
      this.up = up;
      if (up) this.waiters.splice(0).forEach((w) => w());
      else this.failAll();
      this.statusHandlers.forEach((h) => h(up));
    };
  }

  private failAll() {
    for (const [, p] of this.pending) {
      clearTimeout(p.timer);
      p.err(new RpcError(-1, 'Conexão com o Core perdida'));
    }
    this.pending.clear();
  }

  private receive(text: string) {
    let m: any;
    try {
      m = JSON.parse(text);
    } catch {
      return;
    }
    if (m.id !== undefined && m.id !== null && this.pending.has(m.id)) {
      const p = this.pending.get(m.id)!;
      this.pending.delete(m.id);
      clearTimeout(p.timer);
      if (m.error) p.err(new RpcError(m.error.code, m.error.message, m.error.data));
      else p.ok(m.result);
    } else if (m.method) {
      this.handlers.get(m.method)?.forEach((h) => h(m.params));
      this.handlers.get('*')?.forEach((h) => h({ method: m.method, params: m.params }));
    }
  }

  ready(): Promise<void> {
    return this.up ? Promise.resolve() : new Promise((r) => this.waiters.push(r));
  }

  async call<T = any>(method: string, params: Record<string, unknown> = {}, timeoutMs = 20000): Promise<T> {
    if (!this.up) await this.ready();
    return new Promise<T>((ok, err) => {
      const id = ++this.id;
      const timer = setTimeout(() => {
        this.pending.delete(id);
        err(new RpcError(-2, `Tempo esgotado: ${method}`));
      }, timeoutMs);
      this.pending.set(id, { ok, err, timer });
      this.t.send(JSON.stringify({ jsonrpc: '2.0', id, method, params }));
    });
  }

  on(method: string, h: Handler): () => void {
    let s = this.handlers.get(method);
    if (!s) this.handlers.set(method, (s = new Set()));
    s.add(h);
    return () => s!.delete(h);
  }

  onStatus(h: (up: boolean) => void): () => void {
    this.statusHandlers.add(h);
    return () => this.statusHandlers.delete(h);
  }

  close() {
    this.t.close();
  }
}
