import { createSignal, For, onCleanup, onMount, Show } from 'solid-js';
import { ICONS } from '@nexus/ui';
import { connect, type Link, type Mode } from './link';
import { Gamepad } from './Gamepad';
import { Touchpad } from './Touchpad';

const Ic = (p: { n: string }) => <svg class="i" viewBox="0 0 24 24" innerHTML={ICONS[p.n] ?? ''} />;

type View = 'menu' | 'gamepad' | 'touchpad';
type Status = 'connecting' | 'open' | 'closed';

function getToken(): string {
  const u = new URL(location.href);
  const t = u.searchParams.get('t');
  if (t) {
    try { sessionStorage.setItem('nx-token', t); } catch { /* sem storage */ }
    u.searchParams.delete('t');
    history.replaceState(null, '', u.pathname + (u.search || ''));
    return t;
  }
  try { return sessionStorage.getItem('nx-token') ?? ''; } catch { return ''; }
}

const deviceName = () => {
  const ua = navigator.userAgent;
  if (/iPhone/.test(ua)) return 'iPhone';
  if (/iPad/.test(ua)) return 'iPad';
  const m = /Android[^;]*; ([^)]+?)(?: Build|\))/.exec(ua);
  return m ? m[1].trim() : /Android/.test(ua) ? 'Android' : 'Celular';
};

export function App() {
  const token = getToken();
  const [view, setView] = createSignal<View>('menu');
  const [auth, setAuth] = createSignal<'checking' | 'ok' | 'bad'>('checking');
  const [err, setErr] = createSignal('');
  const [taken, setTaken] = createSignal<number[]>([]);
  const [uinput, setUinput] = createSignal(false);
  const [status, setStatus] = createSignal<Status>('closed');
  const [link, setLink] = createSignal<Link | null>(null);
  const [player, setPlayer] = createSignal(1);
  const [kicked, setKicked] = createSignal(false);

  const hello = async () => {
    if (!token) return setAuth('bad'), setErr('Escaneie o QR code mostrado no console para abrir o controle.');
    try {
      const r = await fetch(`remote/hello?t=${encodeURIComponent(token)}`);
      const j = await r.json();
      if (!r.ok || !j.ok) {
        setAuth('bad');
        setErr(j.error ?? 'Não foi possível conectar.');
        return;
      }
      setTaken(j.playersTaken ?? []);
      setUinput(!!j.uinput);
      setAuth('ok');
    } catch {
      setAuth('bad');
      setErr('Sem conexão com o console. Confira se o celular está na mesma rede Wi-Fi.');
    }
  };
  onMount(() => void hello());

  const start = (mode: Mode, p?: number) => {
    setErr('');
    setKicked(false);
    if (mode === 'gamepad') setPlayer(p ?? 1);
    const l = connect(token, { mode, player: p, name: deviceName() }, {
      onJoined: (i) => { setUinput(i.uinput); setView(i.mode); },
      onError: (m) => { setErr(m); setView('menu'); l.close(); setLink(null); void hello(); },
      onStatus: setStatus,
      onKicked: () => { setKicked(true); setView('menu'); l.close(); setLink(null); setAuth('bad'); setErr('Este aparelho foi desconectado pelo console.'); },
    });
    setLink(l);
  };
  const leave = () => {
    link()?.close();
    setLink(null);
    setView('menu');
    void hello();
  };
  onCleanup(() => link()?.close());

  const wake = async () => {
    try { await (navigator as any).wakeLock?.request('screen'); } catch { /* sem wake lock */ }
  };
  const fullscreen = () => { void document.documentElement.requestFullscreen?.().catch(() => {}); void wake(); };

  return (
    <div class="app">
      <Show when={view() === 'menu'}>
        <div class="menu">
          <svg class="logo" viewBox="0 0 100 100"><path d="M20 80 L20 32 Q20 16 32 26 L68 72 Q80 84 80 68 L80 20" /></svg>
          <h1>Nexus</h1>
          <p class="sub">Use o celular como controle, mouse e teclado</p>
          <Show when={auth() === 'checking'}><p class="note"><span class="spin" /> Conectando…</p></Show>
          <Show when={auth() === 'bad'}>
            <div class="alert"><Ic n="lock" /><span>{err()}</span></div>
            <button class="btn" onClick={() => void hello()}><Ic n="refresh" /> Tentar de novo</button>
          </Show>
          <Show when={auth() === 'ok'}>
            <Show when={err()}><div class="alert"><Ic n="info" /><span>{err()}</span></div></Show>
            <div class="cards">
              <For each={[1, 2]}>
                {(p) => (
                  <button class="card" disabled={taken().includes(p)} onClick={() => { fullscreen(); start('gamepad', p); }}>
                    <Ic n="pad" />
                    <b>Controle {p}</b>
                    <small>{taken().includes(p) ? 'Em uso' : p === 1 ? 'Navega no Nexus e joga' : 'Segundo jogador'}</small>
                  </button>
                )}
              </For>
              <button class="card wide" onClick={() => { fullscreen(); start('touchpad'); }}>
                <Ic n="tv" />
                <b>Mouse e teclado</b>
                <small>Touchpad, rolagem e digitação</small>
              </button>
            </div>
            <p class="note">{uinput() ? 'Funciona nos jogos como um controle Xbox 360.' : 'Controla o menu do Nexus. Jogos exigem /dev/uinput no console.'}</p>
          </Show>
        </div>
      </Show>
      <Show when={view() === 'gamepad'}>
        <Gamepad link={link()!} player={player()} status={status()} onLeave={leave} />
      </Show>
      <Show when={view() === 'touchpad'}>
        <Touchpad link={link()!} status={status()} onLeave={leave} />
      </Show>
      <Show when={kicked()}><div class="toastbar">Desconectado pelo console</div></Show>
    </div>
  );
}
