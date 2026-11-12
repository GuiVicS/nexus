// Telas de sistema compartilhadas entre o assistente de primeiro uso e os Ajustes: Rede (B1), Bluetooth (B2),
// Áudio (B3) e Idioma/hora (B8).
// Tudo passa pelo Core (sysnet.rs, locale.rs); a Shell só mostra e pede.
import { createSignal, For, onCleanup, onMount, Show } from 'solid-js';
import type { AudioNode, BtDevice, BtStatus, LocaleStatus, NetStatus, NetTest, WifiNet } from '@nexus/protocol';
import { audio, audioSet, loadAudio } from '../lib/audio';
import { sfx } from '@nexus/ui';
import { api, client } from '../lib/api';
import { cycHandlers } from '../lib/actions';
import { askChoice, askConfirm, askText, fail, guarded, toast } from '../lib/state';
import { Icon, Switch } from './ui';

function Info(p: { label: string; desc?: string; k?: string }) {
  return <div class="srow info" data-k={p.k}><div><b>{p.label}</b><Show when={p.desc}><small>{p.desc}</small></Show></div></div>;
}

const bars = (s: number) => (s >= 75 ? '▂▄▆█' : s >= 50 ? '▂▄▆' : s >= 25 ? '▂▄' : '▂');

export function netSummary(st: NetStatus | null): string {
  if (!st) return 'Verificando…';
  if (!st.available) return st.error || 'Gerenciador de rede indisponível';
  const m = st.main;
  if (!m) return 'Sem conexão';
  const via = m.type === 'wifi' ? `Wi-Fi “${m.connection}”${st.signal != null ? ` · sinal ${st.signal}%` : ''}` : 'Cabo de rede';
  const net = st.online ? 'internet ok' : st.connectivity === 'portal' ? 'precisa entrar pelo navegador (portal)' : 'sem internet';
  return `${via} · ${net}`;
}

/** Rede: estado, Wi-Fi (lista, conectar, esquecer, oculta), teste e — fora do assistente — IP fixo. */
export function NetPanel(p: { wizard?: boolean; onOnline?: () => void }) {
  const [st, setSt] = createSignal<NetStatus | null>(null);
  const [nets, setNets] = createSignal<WifiNet[] | null>(null);
  const [scanning, setScanning] = createSignal(false);
  const [busy, setBusy] = createSignal<string | null>(null);
  const [test, setTest] = createSignal<NetTest | 'run' | null>(null);

  const load = async () => {
    try {
      const s = await api.netStatus();
      setSt(s);
      if (s.online) p.onOnline?.();
      if (s.available && s.hasWifi && s.wifiEnabled && nets() === null) void scan(false);
    } catch (e) {
      setSt({ available: false, error: 'Não foi possível consultar a rede' });
      fail(e);
    }
  };
  const scan = async (rescan = true) => {
    if (scanning()) return;
    setScanning(true);
    try {
      setNets((await api.wifiList(rescan)).networks);
    } catch (e) {
      fail(e);
      setNets([]);
    } finally {
      setScanning(false);
    }
  };
  const connect = async (n: WifiNet, hidden = false) => {
    let pw: string | undefined;
    if (n.secure && !n.known) {
      const v = await askText({ title: `Senha de “${n.ssid}”`, sub: 'Digite aqui ou pelo teclado do celular (Ajustes → Controle no celular)', kind: 'secret', max: 63 });
      if (v === null) return;
      pw = v;
    }
    setBusy(n.ssid);
    try {
      const r = await guarded((a) => api.wifiConnect(n.ssid, pw, hidden, a), 'Confirme para mudar a rede');
      if (r) {
        sfx('on');
        toast('Conectado', n.ssid, 'wifi');
      }
    } catch (e) {
      fail(e);
    } finally {
      setBusy(null);
      void load();
      void scan(false);
    }
  };
  const forget = async (n: WifiNet) => {
    if (!(await askConfirm({ title: `Esquecer “${n.ssid}”?`, sub: 'A senha salva é apagada. Para voltar, conecte de novo.', yes: 'Esquecer', no: 'Cancelar', danger: true }))) return;
    const r = await guarded((a) => api.netForget(n.ssid, a), 'Confirme para mudar a rede');
    if (r) toast('Rede esquecida', n.ssid, 'trash');
    void load();
    void scan(false);
  };
  const hidden = async () => {
    const ssid = await askText({ title: 'Nome da rede oculta', sub: 'Exatamente como está no roteador', kind: 'secret', max: 32 });
    if (!ssid) return;
    const pw = await askText({ title: `Senha de “${ssid}”`, sub: 'Deixe vazio se a rede não tiver senha', kind: 'secret', max: 63 });
    if (pw === null) return;
    setBusy(ssid);
    try {
      const r = await guarded((a) => api.wifiConnect(ssid, pw || undefined, true, a), 'Confirme para mudar a rede');
      if (r) toast('Conectado', ssid, 'wifi');
    } catch (e) {
      fail(e);
    } finally {
      setBusy(null);
      void load();
    }
  };
  const runTest = async () => {
    setTest('run');
    try {
      setTest(await api.netTest());
    } catch (e) {
      setTest(null);
      fail(e);
    }
  };
  const staticIp = async () => {
    const s = st();
    const con = s?.main?.connection;
    if (!con) return toast('Sem conexão ativa', 'Conecte-se a uma rede primeiro', 'info');
    const manual = await askChoice({ title: 'Endereço IP', sub: `Conexão “${con}”. Automático (DHCP) serve para quase todo mundo.`, yes: 'IP fixo', no: 'Automático' });
    if (manual === null) return;
    if (!manual) {
      const r = await guarded((a) => api.setIpv4({ connection: con, method: 'auto', ...a }), 'Ajuste avançado: PIN dos responsáveis');
      if (r) toast('IP automático', con, 'check');
      return void load();
    }
    const address = await askText({ title: 'Endereço com máscara', sub: 'Ex.: 192.168.0.50/24', kind: 'ip', max: 18, value: s?.ip?.address ?? '' });
    if (!address) return;
    const gateway = await askText({ title: 'Gateway (roteador)', sub: 'Ex.: 192.168.0.1', kind: 'ip', max: 15, value: s?.ip?.gateway ?? '' });
    if (!gateway) return;
    const dns = await askText({ title: 'DNS', sub: 'Um ou mais, separados por vírgula. Ex.: 1.1.1.1,8.8.8.8', kind: 'ip', max: 47, value: (s?.ip?.dns ?? []).join(',') });
    if (!dns) return;
    const r = await guarded((a) => api.setIpv4({ connection: con, method: 'manual', address, gateway, dns: dns.split(',').map((x) => x.trim()).filter(Boolean), ...a }), 'Ajuste avançado: PIN dos responsáveis');
    if (r) toast('IP fixo aplicado', address, 'check');
    void load();
  };

  onMount(() => {
    void load();
    const off = client.on('net.changed', () => void load());
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      const n = (i: string) => (nets() ?? [])[Number(i)];
      if (a === 'reload') void load();
      else if (a === 'scan') void scan(true);
      else if (a === 'test') void runTest();
      else if (a === 'hidden') void hidden();
      else if (a === 'ip') void staticIp();
      else if (a === 'wifi') {
        const on = !st()?.wifiEnabled;
        void guarded((x) => api.wifiEnable(on, x)).then(() => {
          setNets(null);
          void load();
        });
      } else if (a.startsWith('join:') && n(a.slice(5))) {
        const w = n(a.slice(5));
        if (w.active) void forget(w);
        else void connect(w);
      } else if (a.startsWith('forget:') && n(a.slice(7))) void forget(n(a.slice(7)));
    };
    document.addEventListener('nexus:net', h);
    // o sinal e o estado mudam sozinhos (roteador reiniciou, cabo saiu)
    const t = setInterval(() => void api.netStatus().then(setSt).catch(() => {}), 10000);
    onCleanup(() => {
      off();
      clearInterval(t);
      document.removeEventListener('nexus:net', h);
    });
  });

  const testText = () => {
    const t = test();
    if (t === 'run') return 'Testando…';
    if (!t) return '';
    if (t.online) return `Internet funcionando${t.latencyMs != null ? ` · resposta em ${t.latencyMs} ms` : ''}`;
    if (!t.dns && t.latencyMs != null) return 'Conectado, mas o DNS não responde — tente o DNS 1.1.1.1 em “IP e DNS”';
    if (t.connectivity === 'portal') return 'A rede pede login (hotel, café): não dá para entrar pelo Nexus ainda';
    return 'Sem internet. Confira o roteador ou o cabo.';
  };

  return (
    <>
      <div class="srow info netsum" data-k="net-sum">
        <Icon n={st()?.main?.type === 'wifi' ? 'wifi' : st()?.online ? 'check' : 'info'} />
        <div><b>{netSummary(st())}</b><Show when={st()?.ip?.address}><small>IP {st()!.ip!.address.split('/')[0]}{st()!.ipv4Method === 'manual' ? ' (fixo)' : ''}</small></Show></div>
      </div>
      <Show when={st() && !st()!.available}>
        <div class="srow f" data-k="net-reload" data-r="22" data-a="net:reload" data-verb="Tentar"><div><b>Tentar de novo</b><small>Consulta o gerenciador de rede outra vez</small></div><Icon n="refresh" /></div>
      </Show>
      <Show when={st()?.available}>
        <div class="srow f" data-k="net-test" data-r="22" data-a="net:test" data-verb="Testar"><div><b>Testar conexão</b><small>{testText() || 'Confere internet, DNS e tempo de resposta'}</small></div><Icon n="refresh" /></div>
        <Show when={st()!.hasWifi}>
          <div class="srow f" data-k="net-wifi" data-r="22" data-a="net:wifi" data-verb="Alternar"><div><b>Wi-Fi</b><small>{st()!.wifiEnabled ? 'Ligado' : 'Desligado'}</small></div><Switch on={!!st()!.wifiEnabled} /></div>
          <Show when={st()!.wifiEnabled}>
            <h3 class="sh2">Redes Wi-Fi</h3>
            <div class={`srow f ${scanning() ? 'off' : ''}`} data-k="net-scan" data-r="22" data-a="net:scan" data-verb="Procurar"><div><b>{scanning() ? 'Procurando redes…' : 'Procurar redes'}</b><small>Atualiza a lista</small></div><Icon n="search" /></div>
            <For each={nets() ?? []} fallback={<Info label={nets() === null ? 'Procurando…' : 'Nenhuma rede encontrada'} desc={nets() === null ? '' : 'Chegue mais perto do roteador e procure de novo'} />}>
              {(n, i) => (
                <div class={`srow f ${busy() === n.ssid ? 'off' : ''}`} data-k={`wifi-${i()}`} data-r="22" data-a={`net:join:${i()}`} data-verb={n.active ? 'Esquecer' : 'Conectar'}>
                  <div>
                    <b>{n.ssid}</b>
                    <small>{busy() === n.ssid ? 'Conectando…' : `${n.active ? 'Conectado · ' : n.known ? 'Salva · ' : ''}Sinal ${n.signal}% ${bars(n.signal)}${n.secure ? ' · protegida' : ' · aberta'}`}</small>
                  </div>
                  <Icon n={n.active ? 'check' : n.secure ? 'lock' : 'wifi'} />
                </div>
              )}
            </For>
            <div class="srow f" data-k="net-hidden" data-r="22" data-a="net:hidden" data-verb="Conectar"><div><b>Rede oculta</b><small>Digite o nome e a senha de uma rede que não aparece na lista</small></div><Icon n="plus" /></div>
          </Show>
        </Show>
        <Show when={!p.wizard}>
          <h3 class="sh2">Detalhes</h3>
          <Show when={st()!.ip} fallback={<Info label="Sem endereço" desc="Conecte-se a uma rede" />}>
            <Info k="net-ip" label="Endereço e DNS" desc={`IP ${st()!.ip!.address} · gateway ${st()!.ip!.gateway || '—'} · DNS ${st()!.ip!.dns.join(', ') || '—'}${st()!.ip!.mac ? ` · MAC ${st()!.ip!.mac}` : ''}`} />
          </Show>
          <div class="srow f" data-k="net-ipfix" data-r="22" data-a="net:ip" data-verb="Ajustar"><div><b>IP fixo e DNS (avançado)</b><small>{st()!.ipv4Method === 'manual' ? 'IP fixo em uso' : 'Automático (DHCP)'} · pede o PIN dos responsáveis</small></div><Icon n="lock" /></div>
        </Show>
      </Show>
    </>
  );
}

/** Idioma, fuso, hora automática e teclado. As mudanças valem na hora. */
export function LocalePanel() {
  const [st, setSt] = createSignal<LocaleStatus | null>(null);
  const [tz, setTz] = createSignal('');
  const [kb, setKb] = createSignal('');
  const [now, setNow] = createSignal(new Date());
  let tzT: ReturnType<typeof setTimeout> | undefined;
  let kbT: ReturnType<typeof setTimeout> | undefined;
  const apply = async (p: { timezone?: string; keyboard?: string; ntp?: boolean }) => {
    try {
      const r = await guarded((a) => api.localeSet({ ...p, ...a }), 'Confirme para mudar o relógio ou o teclado');
      if (r) {
        setSt(r);
        setTz(r.timezone);
        setKb(r.keyboard);
        setNow(new Date());
      }
    } catch (e) {
      fail(e);
      const s = st();
      if (s) {
        setTz(s.timezone);
        setKb(s.keyboard);
      }
    }
  };
  const step = <T,>(list: T[], cur: number, d: 1 | -1) => list[(cur + d + list.length) % list.length];
  onMount(() => {
    void api.localeStatus().then((s) => {
      setSt(s);
      setTz(s.timezone);
      setKb(s.keyboard);
    }).catch(fail);
    // ←/→ escolhe; aplica quando o usuário para (não chama o sistema a cada passo)
    cycHandlers.set('loc:tz', (d) => {
      const l = st()?.timezones ?? [];
      if (!l.length) return;
      const nx = step(l, l.findIndex((x) => x.id === tz()), d);
      setTz(nx.id);
      sfx('tab');
      clearTimeout(tzT);
      tzT = setTimeout(() => void apply({ timezone: nx.id }), 900);
    });
    cycHandlers.set('loc:kb', (d) => {
      const l = st()?.keyboards ?? [];
      if (!l.length) return;
      const nx = step(l, l.findIndex((x) => x.id === kb()), d);
      setKb(nx.id);
      sfx('tab');
      clearTimeout(kbT);
      kbT = setTimeout(() => void apply({ keyboard: nx.id }), 900);
    });
    const h = (e: Event) => {
      if ((e as CustomEvent<string>).detail === 'ntp') void apply({ ntp: !st()?.ntp });
    };
    document.addEventListener('nexus:loc', h);
    const t = setInterval(() => setNow(new Date()), 15000);
    onCleanup(() => {
      cycHandlers.delete('loc:tz');
      cycHandlers.delete('loc:kb');
      clearTimeout(tzT);
      clearTimeout(kbT);
      clearInterval(t);
      document.removeEventListener('nexus:loc', h);
    });
  });
  const tzName = () => st()?.timezones.find((x) => x.id === tz())?.name ?? (tz() || '—');
  const kbName = () => st()?.keyboards.find((x) => x.id === kb())?.name ?? kb();
  return (
    <Show when={st()} fallback={<Info label="Carregando…" />}>
      <Info k="loc-lang" label="Idioma" desc="Português (Brasil) — outros idiomas ainda não estão disponíveis" />
      <div class="srow f" data-k="loc-tz" data-r="22" data-cyc="loc:tz" data-a="cyc:loc:tz" data-verb="Alterar">
        <div><b>Fuso horário</b><small>{now().toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit', timeZone: tz() || undefined })} agora · use ← →</small></div>
        <div class="cv"><Icon n="left" /><span>{tzName()}</span><Icon n="right" /></div>
      </div>
      <div class="srow f" data-k="loc-ntp" data-r="22" data-a="loc:ntp" data-verb="Alternar">
        <div><b>Acertar a hora pela internet</b><small>{st()!.ntp ? (st()!.synced ? 'Ligado · hora certa' : 'Ligado · aguardando a internet') : 'Desligado'}</small></div>
        <Switch on={st()!.ntp} />
      </div>
      <div class="srow f" data-k="loc-kb" data-r="22" data-cyc="loc:kb" data-a="cyc:loc:kb" data-verb="Alterar">
        <div><b>Teclado</b><small>Para teclado físico ligado ao aparelho · use ← →</small></div>
        <div class="cv"><Icon n="left" /><span>{kbName()}</span><Icon n="right" /></div>
      </div>
    </Show>
  );
}

// ---------- B2 Bluetooth ----------
const BT_KIND: Record<string, [string, string]> = {
  controle: ['Controle', 'pad'],
  fone: ['Fone ou caixa de som', 'music'],
  teclado: ['Teclado', 'grid'],
  mouse: ['Mouse', 'grid'],
  celular: ['Celular', 'bt'],
  outro: ['Aparelho Bluetooth', 'bt'],
};
const btKind = (d: BtDevice) => BT_KIND[d.kind ?? (d.gamepad ? 'controle' : 'outro')] ?? BT_KIND.outro;

/** Como colocar cada controle em modo de pareamento: o caso que mais trava quem está começando. */
export const PAD_HELP: [string, string][] = [
  ['Xbox (Series, One com Bluetooth)', 'Ligue com o botão Xbox e segure o botão de sincronizar (em cima, perto do USB) até o Xbox piscar rápido.'],
  ['PlayStation 5 (DualSense)', 'Com o controle desligado, segure Create + PS até a luz em volta do touchpad piscar.'],
  ['PlayStation 4 (DualShock 4)', 'Com o controle desligado, segure Share + PS até a barra de luz piscar em dupla.'],
  ['Nintendo Switch Pro / Joy-Con', 'Segure o botão de sincronizar (pequeno, perto do USB ou no trilho do Joy-Con) até as luzes correrem.'],
  ['8BitDo', 'Ligue em modo Switch/X-input (Start + Y ou Start + X) e segure o botão de pareamento por 3 s.'],
  ['Outros', 'Procure no manual “modo de pareamento”. Se não aparecer, use o cabo USB: funciona sem configurar nada.'],
];

/** Bluetooth: liga/desliga, aparelhos pareados (tipo, bateria, conectar), procurar e parear, e ajuda por modelo. */
export function BtPanel(p: { onlyPads?: boolean }) {
  const [st, setSt] = createSignal<BtStatus | null>(null);
  const [found, setFound] = createSignal<BtDevice[] | null>(null);
  const [scanning, setScanning] = createSignal(false);
  const [busy, setBusy] = createSignal<string | null>(null);
  const [help, setHelp] = createSignal(0);
  const load = () => void api.btStatus().then(setSt).catch(() => setSt({ available: false, powered: false, paired: [] }));

  const scan = async () => {
    if (scanning()) return;
    setScanning(true);
    setFound(null);
    try {
      const r = (await api.btScan(12)).devices;
      setFound(p.onlyPads ? r.filter((d) => d.gamepad) : r);
      load();
    } catch (e) {
      fail(e);
      setFound([]);
    } finally {
      setScanning(false);
    }
  };
  const pair = async (d: BtDevice) => {
    setBusy(d.mac);
    try {
      const r = await guarded((a) => api.btPair(d.mac, a), 'Confirme para parear o aparelho');
      if (r) {
        sfx('on');
        toast('Pareado', d.name, 'check');
        setFound((f) => (f ?? []).filter((x) => x.mac !== d.mac));
      }
    } catch (e) {
      fail(e);
    } finally {
      setBusy(null);
      load();
    }
  };
  const manage = async (d: BtDevice) => {
    const c = await askChoice({ title: d.name, sub: `${btKind(d)[0]}${d.battery != null ? ` · bateria ${d.battery}%` : ''} · ${d.connected ? 'conectado' : 'desconectado'}`, yes: d.connected ? 'Desconectar' : 'Conectar', no: 'Esquecer' });
    if (c === null) return;
    if (c) {
      setBusy(d.mac);
      try {
        await (d.connected ? api.btDisconnect(d.mac) : api.btConnect(d.mac));
        toast(d.connected ? 'Desconectado' : 'Conectado', d.name, d.connected ? 'bt' : 'check');
      } catch (e) {
        fail(e);
      } finally {
        setBusy(null);
        load();
      }
      return;
    }
    if (!(await askConfirm({ title: `Esquecer “${d.name}”?`, sub: 'Para usar de novo, será preciso parear outra vez.', yes: 'Esquecer', no: 'Cancelar', danger: true }))) return;
    const r = await guarded((a) => api.btRemove(d.mac, a), 'Confirme para esquecer o aparelho');
    if (r) toast('Aparelho esquecido', d.name, 'trash');
    load();
  };
  const power = async () => {
    const on = !st()?.powered;
    try {
      await api.btPower(on);
      if (!on) setFound(null);
    } catch (e) {
      fail(e);
    }
    load();
  };

  onMount(() => {
    load();
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      if (a === 'reload') load();
      else if (a === 'power') void power();
      else if (a === 'scan') void scan();
      else if (a.startsWith('dev:')) {
        const d = st()?.paired[Number(a.slice(4))];
        if (d) void manage(d);
      } else if (a.startsWith('pair:')) {
        const d = (found() ?? [])[Number(a.slice(5))];
        if (d) void pair(d);
      }
    };
    document.addEventListener('nexus:bt', h);
    cycHandlers.set('bt:help', (d) => {
      setHelp((i) => (i + d + PAD_HELP.length) % PAD_HELP.length);
      sfx('tab');
    });
    // conexão e bateria mudam sozinhas (controle desligou, fone saiu de perto)
    const t = setInterval(load, 8000);
    onCleanup(() => {
      clearInterval(t);
      cycHandlers.delete('bt:help');
      document.removeEventListener('nexus:bt', h);
    });
  });

  const paired = () => (st()?.paired ?? []).filter((d) => !p.onlyPads || d.gamepad);
  return (
    <Show when={st()} fallback={<Info label="Verificando o Bluetooth…" />}>
      {(b) => (
        <Show
          when={b().available}
          fallback={
            <>
              <Info k="bt-none" label="Bluetooth indisponível" desc="Este aparelho não tem adaptador Bluetooth (ou o serviço está desligado). Use um controle USB ou um adaptador Bluetooth USB." />
              <div class="srow f" data-k="bt-reload" data-r="22" data-a="bt:reload" data-verb="Tentar"><div><b>Tentar de novo</b><small>Verifica o adaptador outra vez</small></div><Icon n="refresh" /></div>
            </>
          }
        >
          <div class="srow f" data-k="bt-power" data-r="22" data-a="bt:power" data-verb="Alternar">
            <div><b>Bluetooth</b><small>{b().powered ? 'Ligado' : 'Desligado · controles e fones sem fio não conectam'}</small></div>
            <Switch on={b().powered} />
          </div>
          <Show when={b().powered}>
            <h3 class="sh2">Meus aparelhos</h3>
            <For each={paired()} fallback={<Info label="Nenhum aparelho pareado" desc="Procure abaixo para parear um controle, fone ou teclado" />}>
              {(d) => {
                const i = () => b().paired.indexOf(d);
                return (
                  <div class={`srow f ${busy() === d.mac ? 'off' : ''}`} data-k={`bt-d-${i()}`} data-r="22" data-a={`bt:dev:${i()}`} data-verb="Opções">
                    <Icon n={btKind(d)[1]} />
                    <div>
                      <b>{d.name}</b>
                      <small>{busy() === d.mac ? 'Aguarde…' : `${btKind(d)[0]} · ${d.connected ? 'conectado' : 'desconectado'}${d.battery != null ? ` · bateria ${d.battery}%${d.battery <= 20 ? ' (baixa)' : ''}` : ''}`}</small>
                    </div>
                    <Icon n={d.connected ? 'check' : 'more'} />
                  </div>
                );
              }}
            </For>
            <h3 class="sh2">Parear novo</h3>
            <div class={`srow f ${scanning() ? 'off' : ''}`} data-k="bt-scan" data-r="22" data-a="bt:scan" data-verb="Procurar">
              <div><b>{scanning() ? 'Procurando… (12 s)' : p.onlyPads ? 'Procurar controles' : 'Procurar aparelhos'}</b><small>Deixe o aparelho em modo de pareamento antes</small></div>
              <Icon n="search" />
            </div>
            <For each={found() ?? []} fallback={<Show when={found()}><Info k="bt-empty" label="Nada encontrado" desc="Confira se o aparelho está piscando (modo de pareamento) e procure de novo" /></Show>}>
              {(d, i) => (
                <div class={`srow f ${busy() === d.mac ? 'off' : ''}`} data-k={`bt-f-${i()}`} data-r="22" data-a={`bt:pair:${i()}`} data-verb="Parear">
                  <Icon n={btKind(d)[1]} />
                  <div><b>{d.name}</b><small>{busy() === d.mac ? 'Pareando…' : `${btKind(d)[0]} · toque para parear`}</small></div>
                  <Icon n="plus" />
                </div>
              )}
            </For>
            <div class="srow f" data-k="bt-help" data-r="22" data-cyc="bt:help" data-a="cyc:bt:help" data-verb="Trocar">
              <div><b>Controle não pareia? {PAD_HELP[help()][0]}</b><small>{PAD_HELP[help()][1]}</small></div>
              <div class="cv"><Icon n="left" /><span>{help() + 1}/{PAD_HELP.length}</span><Icon n="right" /></div>
            </div>
          </Show>
        </Show>
      )}
    </Show>
  );
}

// ---------- B3 Áudio ----------
const OUT_KIND: Record<AudioNode['kind'], [string, string]> = {
  hdmi: ['TV ou monitor (HDMI)', 'tv'],
  analogica: ['Alto-falantes ou fone com fio', 'vol'],
  bluetooth: ['Bluetooth', 'bt'],
  usb: ['USB', 'vol'],
};

/** Áudio: volume geral, mudo, saída (lembrada por perfil), teste de som e microfone. */
export function AudioPanel() {
  const [testing, setTesting] = createSignal(false);
  const runTest = async () => {
    setTesting(true);
    try {
      await api.audioTest();
      toast('Som de teste tocado', 'Ouviu dois bipes? Se não, troque a saída abaixo', 'vol');
    } catch (e) {
      fail(e);
    } finally {
      setTesting(false);
    }
  };
  onMount(() => {
    void loadAudio();
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      const st = audio();
      if (a === 'reload') void loadAudio();
      else if (a === 'test') void runTest();
      else if (a.startsWith('out:') && st) {
        const o = st.sinks[Number(a.slice(4))];
        if (o && !o.default) {
          void audioSet({ default: o.id }).then(() => {
            sfx('on');
            toast('Saída de som', o.name, 'vol');
          });
        }
      }
    };
    document.addEventListener('nexus:audio', h);
    // fone Bluetooth ou TV ligados/desligados mudam a lista sozinhos
    const t = setInterval(() => void loadAudio(), 6000);
    onCleanup(() => {
      clearInterval(t);
      document.removeEventListener('nexus:audio', h);
    });
  });
  return (
    <Show when={audio()} fallback={<Info label="Carregando o som…" />}>
      {(a) => (
        <Show
          when={a().available && a().sinks.length}
          fallback={
            <>
              <Info k="au-none" label={a().available ? 'Nenhuma saída de som' : 'Som indisponível'} desc={a().available ? 'Ligue uma TV pelo HDMI, um fone ou caixa de som' : a().error || 'O serviço de som não respondeu'} />
              <div class="srow f" data-k="au-reload" data-r="22" data-a="audio:reload" data-verb="Tentar"><div><b>Tentar de novo</b><small>Consulta o som outra vez</small></div><Icon n="refresh" /></div>
            </>
          }
        >
          <div class="srow f slider" data-k="au-vol" data-r="22" data-slider="sysvol" data-a="audio:mute" data-verb="Mudo">
            <b>Volume{a().muted ? ' (mudo)' : ''}</b><div class="tr"><i style={{ width: `${a().muted ? 0 : a().volume}%` }} /></div><span class="v">{a().volume}</span>
          </div>
          <div class="srow f" data-k="au-mute" data-r="22" data-a="audio:mute" data-verb="Alternar"><div><b>Sem som (mudo)</b><small>Também nos jogos e apps</small></div><Switch on={a().muted} /></div>
          <div class={`srow f ${testing() ? 'off' : ''}`} data-k="au-test" data-r="22" data-a="audio:test" data-verb="Tocar"><div><b>{testing() ? 'Tocando…' : 'Testar o som'}</b><small>Toca dois bipes na saída escolhida</small></div><Icon n="play" /></div>
          <h3 class="sh2">Saída</h3>
          <For each={a().sinks}>
            {(o, i) => (
              <div class="srow f" data-k={`au-out-${i()}`} data-r="22" data-a={`audio:out:${i()}`} data-verb="Usar">
                <Icon n={OUT_KIND[o.kind]?.[1] ?? 'vol'} />
                <div><b>{o.name}</b><small>{OUT_KIND[o.kind]?.[0] ?? 'Saída de som'}{o.default ? ' · em uso (lembrada para este perfil)' : ''}</small></div>
                <Icon n={o.default ? 'check' : 'more'} />
              </div>
            )}
          </For>
          <Show when={a().mic}>
            <h3 class="sh2">Microfone</h3>
            <div class="srow f slider" data-k="au-mic" data-r="22" data-slider="micvol" data-a="audio:micmute" data-verb="Mudo">
              <b>{a().sources.find((x) => x.default)?.name ?? 'Microfone'}{a().mic!.muted ? ' (mudo)' : ''}</b><div class="tr"><i style={{ width: `${a().mic!.muted ? 0 : a().mic!.volume}%` }} /></div><span class="v">{a().mic!.volume}</span>
            </div>
          </Show>
        </Show>
      )}
    </Show>
  );
}
