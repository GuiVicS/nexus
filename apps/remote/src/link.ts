// Conexão WebSocket do celular com o Core (mesma origem). Reconecta sozinha e reenvia o "join".
export type Mode = 'gamepad' | 'touchpad';

export interface Link {
  send(msg: Record<string, unknown>): void;
  close(): void;
}

export interface LinkEvents {
  onJoined(i: { mode: Mode; player?: number; uinput: boolean }): void;
  onError(msg: string): void;
  onStatus(s: 'connecting' | 'open' | 'closed'): void;
  onKicked(): void;
}

export function connect(token: string, join: { mode: Mode; player?: number; name: string }, ev: LinkEvents): Link {
  let ws: WebSocket | null = null;
  let closed = false;
  let fatal = false;
  let delay = 400;
  let ping: ReturnType<typeof setInterval> | undefined;
  const open = () => {
    ev.onStatus('connecting');
    const proto = location.protocol === 'https:' ? 'wss' : 'ws';
    ws = new WebSocket(`${proto}://${location.host}/remote/ws?t=${encodeURIComponent(token)}`);
    ws.onopen = () => {
      delay = 400;
      ev.onStatus('open');
      ws!.send(JSON.stringify({ t: 'join', ...join }));
      ping = setInterval(() => ws?.readyState === 1 && ws.send('{"t":"ping"}'), 4000);
    };
    ws.onmessage = (e) => {
      let m: any;
      try {
        m = JSON.parse(String(e.data));
      } catch {
        return;
      }
      if (m.t === 'joined') ev.onJoined({ mode: m.mode, player: m.player ?? undefined, uinput: !!m.uinput });
      else if (m.t === 'error') {
        fatal = true;
        ev.onError(m.error);
      } else if (m.t === 'kicked') {
        fatal = true;
        ev.onKicked();
      }
    };
    ws.onclose = () => {
      clearInterval(ping);
      ev.onStatus('closed');
      if (!closed && !fatal) setTimeout(open, (delay = Math.min(delay * 1.6, 3000)));
    };
    ws.onerror = () => ws?.close();
  };
  open();
  return {
    send: (m) => ws?.readyState === 1 && ws.send(JSON.stringify(m)),
    close: () => {
      closed = true;
      clearInterval(ping);
      ws?.close();
    },
  };
}
