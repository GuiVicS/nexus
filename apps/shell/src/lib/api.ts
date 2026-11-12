import { RpcClient, createApi, webSocketTransport, type Transport } from '@nexus/protocol';

declare global {
  interface Window {
    nexusBridge?: {
      send(text: string): void;
      onMessage(cb: (text: string) => void): void;
      onStatus(cb: (up: boolean) => void): void;
      status(): Promise<boolean>;
      /** Recuperação quando o Core não responde (só ações fixas, executadas pelo processo do Electron). */
      recover?(action: 'restart-core' | 'restart-session' | 'reboot'): Promise<boolean>;
    };
  }
}

export const IS_ELECTRON = typeof window !== 'undefined' && !!window.nexusBridge;

/** Pede ao Electron para reiniciar o Core, a sessão ou o aparelho (o Core pode estar fora do ar). */
export async function recover(action: 'restart-core' | 'restart-session' | 'reboot'): Promise<boolean> {
  const r = window.nexusBridge?.recover;
  if (!r) return false;
  try {
    return await r(action);
  } catch {
    return false;
  }
}

function bridgeTransport(): Transport {
  const b = window.nexusBridge!;
  const t: Transport = { onmessage: null, onstatus: null, send: (x) => b.send(x), close: () => {} };
  b.onMessage((m) => t.onmessage?.(m));
  b.onStatus((up) => t.onstatus?.(up));
  void b.status().then((up) => t.onstatus?.(up));
  return t;
}

function makeTransport(): Transport {
  if (IS_ELECTRON) return bridgeTransport();
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  return webSocketTransport(`${proto}://${location.host}/rpc`);
}

export const client = new RpcClient(makeTransport());
export const api = createApi(client);

export type ArtKind = 'cover' | 'hero' | 'icon' | 'photo' | 'frame' | `shot${0 | 1 | 2 | 3}`;
export const artUrl = (id: string, kind: ArtKind) => (IS_ELECTRON ? `nexus-art://art/${encodeURIComponent(id)}/${kind}` : `/art/${encodeURIComponent(id)}/${kind}`);
