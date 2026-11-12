import { DrivePanel } from './Drive';
import { createEffect, createMemo, createSignal, For, Match, on, onCleanup, onMount, Show, Switch } from 'solid-js';
import { E, RpcError } from '@nexus/protocol';
import type { Profile, SteamAccount } from '@nexus/protocol';
import { sfx } from '@nexus/ui';
import { api } from '../lib/api';
import { cycHandlers, editProfile } from '../lib/actions';
import { enterProfile } from '../lib/session';
import { askConfirm, askPin, askText, closeOv, fail, fx, guarded, me, pushOv, S, setS, toast } from '../lib/state';
import { ratingLabel } from '../lib/format';
import { Avatar, Glyph, Icon } from './ui';
import { setParentalPin } from '../lib/flows';
import { LocalePanel, NetPanel } from './System';
import { PadsSection } from './Pads';

// ---------- Ligar / abertura ----------
export function Standby() {
  return (
    <div class="center">
      <div class="press f" data-autofocus data-a="boot" data-r="p" data-verb="Ligar"><Icon n="power" /> Pressione <Glyph k="A" /> para ligar</div>
    </div>
  );
}

export function Boot() {
  return (
    <div class="center">
      <svg class="bm" viewBox="0 0 100 100"><path d="M20 80 L20 32 Q20 16 32 26 L68 72 Q80 84 80 68 L80 20" /></svg>
      <div class="wm"><For each={'Nexus'.split('')}>{(c, i) => <span style={{ 'animation-delay': `${1 + i() * 0.1}s` }}>{c}</span>}</For></div>
      <div class="bbar"><i /></div>
      <small>{S.hello?.demo ? 'Modo demonstração' : 'Carregando…'}</small>
    </div>
  );
}

// ---------- Quem vai jogar? ----------
export function ProfilePicker() {
  const h = new Date().getHours();
  return (
    <div class="center">
      <h1>Quem vai jogar?</h1>
      <small>{h < 12 ? 'Bom dia' : h < 18 ? 'Boa tarde' : 'Boa noite'} · escolha um perfil</small>
      <div class="profs">
        <For each={S.profiles}>
          {(p, i) => (
            <div class="prof f" data-autofocus={i() === 0 ? '' : undefined} data-a={`prof:${p.id}`} data-r="p" data-pad="14" data-verb="Entrar" aria-label={p.name}>
              <Avatar p={p}>
                <Show when={p.kind === 'kid'}><span class="kid">KIDS</span></Show>
                <Show when={p.hasPin}><span class="lk"><Icon n="lock" /></span></Show>
              </Avatar>
              {p.name}
            </div>
          )}
        </For>
        <div class="prof add f" data-a="profadd" data-r="p" data-pad="14" data-verb="Adicionar" aria-label="Adicionar perfil">
          <div class="av plus"><Icon n="plus" /></div>
          Adicionar
        </div>
      </div>
    </div>
  );
}

// ---------- Primeiro uso (A7) ----------
// Etapas: boas-vindas → idioma/fuso/teclado → rede → controle → perfil → PIN → jogos (ROMs) → celular → Google Drive → Steam →
// pronto. Tudo só com o controle: ↑↓ entre opções, A escolhe, B volta uma etapa. Rede, controle, jogos, celular e
// Steam têm "Pular". Antes do perfil o Core libera rede/relógio/controle sem sessão (primeiro uso); depois do perfil
// o assistente já entra na sessão dele para as etapas seguintes.
type Step = 'welcome' | 'locale' | 'net' | 'pad' | 'profile' | 'pin' | 'roms' | 'phone' | 'drive' | 'steam' | 'done';

export function Onboarding() {
  const hw = () => S.hw;
  const steamStep = () => hw()?.architecture === 'x86_64';
  const steps = (): Step[] => ['welcome', 'locale', 'net', 'pad', 'profile', 'pin', 'roms', 'phone', 'drive', ...(steamStep() ? (['steam'] as Step[]) : []), 'done'];
  const [i, setI] = createSignal(0);
  const step = () => steps()[i()];
  const [created, setCreated] = createSignal<Profile | null>(null);
  const [inSession, setInSession] = createSignal(false);
  const [online, setOnline] = createSignal(false);
  const [steam, setSteam] = createSignal<SteamAccount | null>(null);
  const go = (to: Step) => {
    setI(Math.max(0, steps().indexOf(to)));
    queueMicrotask(() => requestAnimationFrame(() => fx.f?.first(document.getElementById('onboarding'))));
  };
  const next = () => go(steps()[Math.min(steps().length - 1, i() + 1)]);
  const prev = () => {
    // perfil já criado: não volta para antes dele (o perfil existe; editar é em Ajustes)
    const floor = created() ? steps().indexOf('pin') : 0;
    if (i() > floor) go(steps()[i() - 1]);
    else sfx('bump');
  };
  const startProfile = async () => {
    const r = (await editProfile(null, { first: true })) as Profile | undefined;
    if (!r) return;
    setCreated(r);
    try {
      await api.selectProfile({ id: r.id });
      setInSession(true);
    } catch {
      setInSession(false); // perfil com PIN próprio: as etapas seguintes ficam para depois, em Ajustes
    }
    go('pin');
  };
  createEffect(on(step, (st) => {
    if (st === 'steam') void api.steamAccount().then(setSteam).catch(() => setSteam(null));
  }));
  onMount(() => {
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      if (a === 'next' || a === 'skip') next();
      else if (a === 'back') prev();
      else if (a === 'profile') void startProfile();
      else if (a === 'pin') void setParentalPin().then(() => go('roms'));
      else if (a === 'enter' && created()) void enterProfile(created()!.id);
    };
    document.addEventListener('nexus:onb', h);
    onCleanup(() => document.removeEventListener('nexus:onb', h));
  });
  // rodapé: Voltar + um botão principal ("Pular" quando a etapa é opcional e nada foi feito, senão "Continuar")
  const Foot = (p: { skip?: boolean }) => (
    <div class="lacts onbfoot">
      <Show when={i() > 0}><div class="btn f" data-k="onb-back" data-r="p" data-pad="8" data-a="onb:back" data-verb="Voltar"><Icon n="left" /> Voltar</div></Show>
      <div class="btn pri f" data-k={p.skip ? 'onb-skip' : 'onb-next'} data-autofocus data-r="p" data-pad="8" data-a={p.skip ? 'onb:skip' : 'onb:next'} data-verb={p.skip ? 'Pular' : 'Continuar'}>{p.skip ? 'Pular' : 'Continuar'} <Icon n="right" /></div>
    </div>
  );
  const Head = (p: { icon: string; title: string; sub: string }) => (
    <>
      <div class="ob-ic sm"><Icon n={p.icon} /></div>
      <h1>{p.title}</h1>
      <small>{p.sub}</small>
    </>
  );
  return (
    <div class={`center onb s-${step()}`}>
      <Switch>
        <Match when={step() === 'welcome'}>
          <svg class="bm" viewBox="0 0 100 100"><path d="M20 80 L20 32 Q20 16 32 26 L68 72 Q80 84 80 68 L80 20" /></svg>
          <h1>Bem-vindo ao Nexus</h1>
          <small>Seu Linux, com cara de console. Vamos deixar tudo pronto em poucos passos, só com o controle.</small>
          <Show when={hw()}><div class="hwline">{hw()!.os} · {Math.round(hw()!.memoryMb / 1024)} GB de memória · {hw()!.architecture}</div></Show>
          <div class="btn pri f" data-k="onb-next" data-autofocus data-r="p" data-pad="8" data-a="onb:next" data-verb="Começar"><Icon n="play" /> Começar</div>
        </Match>
        <Match when={step() === 'locale'}>
          <Head icon="clock" title="Idioma, hora e teclado" sub="Use ← → nas opções para trocar. Tudo pode ser mudado depois nos Ajustes, em Idioma e hora." />
          <div class="opane"><LocalePanel /></div>
          <Foot />
        </Match>
        <Match when={step() === 'net'}>
          <Head icon="wifi" title="Internet" sub={online() ? 'Você já está conectado. Pode continuar.' : 'Conecte o cabo de rede ou escolha uma rede Wi-Fi. A senha pode ser digitada pelo celular depois que ele for pareado.'} />
          <div class="opane"><NetPanel wizard onOnline={() => setOnline(true)} /></div>
          <Foot skip={!online()} />
        </Match>
        <Match when={step() === 'pad'}>
          <Head icon="pad" title="Controle" sub="Controles USB já funcionam. Para um controle sem fio, coloque-o em modo de pareamento e escolha “Parear controle Bluetooth”." />
          <div class="opane"><PadsSection /></div>
          <Foot skip />
        </Match>
        <Match when={step() === 'profile'}>
          <Head icon="user" title="Crie seu perfil" sub="Escolha um nome e um avatar. Outros perfis (inclusive infantis) podem ser criados depois." />
          <div class="lacts onbfoot">
            <div class="btn f" data-k="onb-back" data-r="p" data-pad="8" data-a="onb:back" data-verb="Voltar"><Icon n="left" /> Voltar</div>
            <div class="btn pri f" data-k="onb-profile" data-autofocus data-r="p" data-pad="8" data-a="onb:profile" data-verb="Criar"><Icon n="plus" /> Criar perfil</div>
          </div>
        </Match>
        <Match when={step() === 'pin'}>
          <Head icon="lock" title="PIN dos responsáveis" sub="Opcional. Protege perfis infantis, compras e ajustes importantes (rede avançada, recuperação). Pode ser definido depois." />
          <div class="lacts onbfoot">
            <div class="btn pri f" data-k="onb-pin" data-autofocus data-r="p" data-pad="8" data-a="onb:pin" data-verb="Definir"><Icon n="lock" /> Definir PIN</div>
            <div class="btn f" data-k="onb-skip" data-r="p" data-pad="8" data-a="onb:skip" data-verb="Pular">Agora não</div>
          </div>
        </Match>
        <Match when={step() === 'roms'}>
          <Head icon="drive" title="Jogos de console (ROMs)" sub={`Os jogos ficam em ${'~/ROMs'}, uma pasta por console. Copie de um pendrive ou envie pelo celular agora, ou depois pela aba Emuladores.`} />
          <div class="lacts onbfoot">
            <div class="btn f" data-k="onb-back" data-r="p" data-pad="8" data-a="onb:back" data-verb="Voltar"><Icon n="left" /> Voltar</div>
            <Show when={inSession()}><div class="btn f" data-k="onb-roms" data-r="p" data-pad="8" data-a="romsadd" data-verb="Adicionar"><Icon n="plus" /> Adicionar jogos</div></Show>
            <div class="btn pri f" data-k="onb-next" data-autofocus data-r="p" data-pad="8" data-a="onb:next" data-verb="Continuar">Continuar <Icon n="right" /></div>
          </div>
        </Match>
        <Match when={step() === 'phone'}>
          <Head icon="tv" title="Celular como controle" sub="Aponte a câmera do celular para o código: ele vira controle, mouse e teclado (útil para digitar senhas)." />
          <Show when={inSession()} fallback={<small>Disponível depois, em Ajustes → Controle no celular.</small>}>
            <PhonePairing />
          </Show>
          <Foot skip />
        </Match>
        <Match when={step() === 'drive'}>
          <Head icon="cloud" title="Google Drive" sub="Opcional. Guarde jogos (ROMs) no seu Drive e jogue na TV sem ocupar espaço. O Nexus usa só a pasta “Nexus” (e a cria se não existir). Para digitar a senha, use o teclado do celular." />
          <Show when={inSession()} fallback={<small>Disponível depois, em Ajustes → Armazenamento.</small>}>
            <div class="opane"><DrivePanel wizard onDone={next} /></div>
          </Show>
          <Show when={!inSession()}><Foot skip /></Show>
        </Match>
        <Match when={step() === 'steam'}>
          <Head icon="download" title="Steam" sub="Os jogos da Steam aparecem na Biblioteca do Nexus. Entre com sua conta (o login mostra um QR code para o app Steam do celular)." />
          <div class="opane">
            <Show when={steam()} fallback={<div class="srow info"><div><b>Verificando a Steam…</b></div></div>}>
              {(s) => (
                <Show when={s().installed} fallback={<div class="srow info"><div><b>Steam não instalada</b><small>Instale depois com: sudo nexus-installer install --steam --yes</small></div></div>}>
                  <Show when={s().persona} fallback={<div class="srow f" data-k="onb-steam" data-r="22" data-a="steamopen" data-verb="Entrar"><div><b>Entrar na Steam</b><small>Abre o login da Steam em tela cheia. Volte com o botão Guia.</small></div><Icon n="play" /></div>}>
                    <div class="srow info"><div><b>Conectado como {s().persona}</b><small>{s().libraryGames} jogos na biblioteca</small></div></div>
                  </Show>
                </Show>
              )}
            </Show>
          </div>
          <Foot skip />
        </Match>
        <Match when={step() === 'done'}>
          <div class="ob-ic ok"><Icon n="check" /></div>
          <h1>Tudo pronto{created() ? `, ${created()!.name}` : ''}!</h1>
          <small>O botão Guia (ou F1, ou Select+Start) abre o menu rápido em qualquer lugar, inclusive durante os jogos.</small>
          <div class="btn pri f" data-k="onb-enter" data-autofocus data-r="p" data-pad="8" data-a="onb:enter" data-verb="Entrar"><Icon n="home" /> Entrar no Nexus</div>
        </Match>
      </Switch>
      <div class="steps"><For each={steps()}>{(_, k) => <i class={k() === i() ? 'on' : k() < i() ? 'done' : ''} />}</For></div>
    </div>
  );
}

/** QR do Remote (assistente): mesmo conteúdo de Ajustes → Controle no celular, compacto. */
function PhonePairing() {
  onMount(() => void api.pairing().then((p) => setS('pairing', p)).catch(fail));
  onCleanup(() => setS('pairing', null));
  return (
    <Show when={S.pairing} fallback={<small>Gerando código…</small>}>
      {(p) => (
        <div class="qrcard onbqr">
          <div class="qr" innerHTML={p().qrSvg} role="img" aria-label="QR code para abrir o controle no celular" />
          <div class="qrinfo">
            <h3>Mesma rede Wi-Fi do aparelho</h3>
            <code>{p().url.replace(/\?t=.*/, '')}</code>
            <small>{p().devices.length ? `${p().devices.length} aparelho(s) conectado(s)` : 'Esperando o celular…'}</small>
          </div>
        </div>
      )}
    </Show>
  );
}

// ---------- Editor de perfil ----------
const AVATARS = ['lia', 'rafa', 'teo', 'bia', 'nina', 'pipo', 'initial'];
const HUES = [180, 210, 260, 320, 0, 30, 50, 120];
const AGES = [0, 6, 10, 12, 14, 16];
const LIMITS: (number | null)[] = [null, 30, 60, 90, 120, 180];

export function ProfileEditor(p: { id: string | null; first?: boolean; oid: number }) {
  const existing = () => (p.id ? S.profiles.find((x) => x.id === p.id) : undefined);
  const [name, setName] = createSignal(existing()?.name ?? '');
  const [avatar, setAvatar] = createSignal(existing()?.avatar ?? 'initial');
  const [hue, setHue] = createSignal(existing()?.hue ?? HUES[Math.floor(Math.random() * HUES.length)]);
  const [kid, setKid] = createSignal(existing()?.kind === 'kid');
  const [age, setAge] = createSignal(existing()?.ageLimit ?? 10);
  const [limit, setLimit] = createSignal<number | null>(existing()?.dailyLimitMin ?? (existing() ? null : 60));
  const [pin, setPin] = createSignal<string | null | undefined>(undefined);
  const [busy, setBusy] = createSignal(false);
  const editing = () => !!existing();
  const preview = createMemo(() => ({ name: name() || '?', avatar: avatar(), hue: hue() }));
  const hasPin = () => (pin() === undefined ? !!existing()?.hasPin : pin() !== null);

  onMount(() => {
    cycHandlers.set('pe:age', (d) => {
      const i = AGES.indexOf(age());
      setAge(AGES[(i + d + AGES.length) % AGES.length]);
      sfx('tab');
    });
    cycHandlers.set('pe:limit', (d) => {
      const i = LIMITS.indexOf(limit());
      setLimit(LIMITS[(i + d + LIMITS.length) % LIMITS.length]);
      sfx('tab');
    });
    cycHandlers.set('pe:kind', () => toggleKid());
    const h = (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      if (a === 'photo') void choosePhoto();
      else if (a.startsWith('avatar:')) setAvatar(a.slice(7));
      else if (a.startsWith('hue:')) setHue(+a.slice(4));
      else if (a === 'name') void editName();
      else if (a === 'pin') void editPin();
      else if (a === 'nopin') setPin(null);
      else if (a === 'kind') toggleKid();
      else if (a === 'save') void save();
      else if (a === 'delete') void remove();
      else if (a === 'cancel') closeOv(p.oid, undefined);
    };
    document.addEventListener('nexus:pe', h);
    onCleanup(() => {
      document.removeEventListener('nexus:pe', h);
      ['pe:age', 'pe:limit', 'pe:kind'].forEach((k) => cycHandlers.delete(k));
    });
    if (!editing()) void editName();
  });

  const toggleKid = () => {
    if (editing()) return;
    setKid(!kid());
    sfx('tab');
    if (kid() && !S.parental.hasPin) toast('Você vai definir o PIN dos responsáveis ao salvar', '', 'lock');
  };

  const editName = async () => {
    const v = await askText({ title: 'Nome do perfil', value: name(), max: 16 });
    if (v !== null && v !== '') setName(v);
    else if (!editing() && !name()) closeOv(p.oid, undefined);
  };

  // foto: o navegador de imagens devolve um caminho; o Core valida, copia e devolve "photo:<arquivo>"
  const [photo, setPhoto] = createSignal<string | null>(existing()?.avatar.startsWith('photo:') ? existing()!.avatar : null);
  const choosePhoto = async () => {
    const path = (await pushOv<string>('filepick', { title: 'Escolher foto do perfil' })) as string | undefined;
    if (!path) return;
    const r = await guarded((a) => api.importAvatar(path, a), 'Confirme para escolher a foto');
    if (!r) return;
    setPhoto(r.avatar);
    setAvatar(r.avatar);
    sfx('on');
    toast('Foto escolhida', 'Salve o perfil para usar', 'check');
  };

  const editPin = async () => {
    const v = await askPin({ title: 'PIN do perfil', sub: 'De 4 a 6 números. Pedido ao entrar neste perfil.', confirm: true });
    if (v !== null) setPin(v);
  };

  const save = async () => {
    if (busy()) return;
    const n = name().trim();
    if (!n) return void toast('Dê um nome ao perfil', '', 'user');
    setBusy(true);
    try {
      if (kid() && !editing() && !S.parental.hasPin) {
        await setParentalPin();
        if (!S.parental.hasPin) return;
      }
      let saved: Profile | undefined;
      if (!editing()) {
        saved = await guarded(
          (a) => api.createProfile({ name: n, avatar: avatar(), hue: hue(), kind: kid() ? 'kid' : 'adult', ageLimit: kid() ? age() : undefined, dailyLimitMin: kid() ? (limit() ?? undefined) : undefined, pin: pin() ?? undefined, ...(a.grant ? { grant: a.grant } : {}) }),
          'Confirme para criar o perfil',
        );
      } else {
        saved = await guarded(
          (a) =>
            api.updateProfile({
              id: p.id!,
              name: n,
              avatar: avatar(),
              hue: hue(),
              ...(existing()!.kind === 'kid' ? { ageLimit: age(), dailyLimitMin: limit() } : {}),
              ...(pin() !== undefined ? { pin: pin() } : {}),
              ...a,
            }),
          'Confirme para alterar o perfil',
        );
      }
      if (!saved) return;
      sfx('on');
      toast(editing() ? 'Perfil atualizado' : 'Perfil criado', n, 'check');
      document.dispatchEvent(new CustomEvent('nexus:profile-saved', { detail: saved }));
      closeOv(p.oid, saved);
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    const e = existing();
    if (!e) return;
    if (me()?.id === e.id) return void toast('Este é o perfil ativo', 'Troque de perfil antes de excluir', 'info');
    if (!(await askConfirm({ title: `Excluir o perfil ${e.name}?`, sub: 'O tempo de jogo e as preferências deste perfil serão apagados.', yes: 'Excluir', danger: true }))) return;
    let pinv: string | undefined;
    if (e.hasPin && !S.parental.hasPin) {
      const v = await askPin({ title: `PIN de ${e.name}` });
      if (v === null) return;
      pinv = v;
    }
    try {
      const r = await guarded((a) => api.deleteProfile({ id: e.id, ...a, ...(pinv ? { pin: pinv } : {}) }), 'Confirme para excluir o perfil');
      if (r) {
        toast('Perfil excluído', e.name, 'trash');
        closeOv(p.oid, undefined);
      }
    } catch (err) {
      fail(err instanceof RpcError && err.code === E.FORBIDDEN ? err : err);
    }
  };

  return (
    <>
      <div class="stitle">{editing() ? 'Editar perfil' : p.first ? 'Seu perfil' : 'Novo perfil'}</div>
      <div class="pe-prev">
        <Avatar p={preview()} size={240} />
        <h2>{name() || 'Sem nome'}</h2>
        <Show when={kid()}><span class="kid solo">KIDS</span></Show>
      </div>
      <div class="setpane pe">
        <div class="srow f" data-k="pe-name" data-autofocus data-r="22" data-a="pe:name" data-verb="Editar">
          <div><b>Nome</b><small>{name() || 'Toque para digitar'}</small></div>
          <Icon n="gear" />
        </div>
        <div class="srow col">
          <b>Avatar</b>
          <div class="chips">
            <div class="btn f" data-k="pe-photo" data-r="p" data-pad="8" data-a="pe:photo" data-verb="Escolher"><Icon n="grid" /> {photo() ? 'Trocar foto' : 'Escolher foto'}</div>
            <Show when={photo()}>
              {(ph) => (
                <div class={`avc f ${avatar() === ph() ? 'on' : ''}`} data-k="pe-av-photo" data-r="p" data-pad="8" data-a={`pe:avatar:${ph()}`} data-verb="Escolher" aria-label="Foto">
                  <Avatar p={{ avatar: ph(), hue: hue(), name: name() || '?' }} size={72} />
                </div>
              )}
            </Show>
            <For each={AVATARS}>
              {(a) => (
                <div class={`avc f ${avatar() === a ? 'on' : ''}`} data-k={`pe-av-${a}`} data-r="p" data-pad="8" data-a={`pe:avatar:${a}`} data-verb="Escolher" aria-label={`Avatar ${a}`}>
                  <Avatar p={{ avatar: a, hue: hue(), name: name() || '?' }} size={72} />
                </div>
              )}
            </For>
          </div>
        </div>
        <div class="srow col">
          <b>Cor</b>
          <div class="chips">
            <For each={HUES}>{(h) => <div class={`huec f ${hue() === h ? 'on' : ''}`} data-k={`pe-h-${h}`} data-r="p" data-pad="8" data-a={`pe:hue:${h}`} data-verb="Escolher" style={{ background: `hsl(${h} 60% 45%)` }} aria-label={`Cor ${h}`} />}</For>
          </div>
        </div>
        <Show when={!editing()} fallback={<div class="srow info"><div><b>Tipo</b><small>{kid() ? 'Perfil infantil' : 'Perfil adulto'}</small></div></div>}>
          <div class="srow f" data-k="pe-kind" data-r="22" data-cyc="pe:kind" data-a="pe:kind" data-verb="Alterar">
            <div><b>Tipo de perfil</b><small>Perfis infantis têm limites e só abrem o que for liberado</small></div>
            <div class="cv"><Icon n="left" /><span>{kid() ? 'Infantil' : 'Adulto'}</span><Icon n="right" /></div>
          </div>
        </Show>
        <Show when={kid()}>
          <div class="srow f" data-k="pe-age" data-r="22" data-cyc="pe:age" data-a="noop" data-verb="Alterar">
            <div><b>Classificação máxima</b><small>Jogos acima disso ficam ocultos</small></div>
            <div class="cv"><Icon n="left" /><span>{ratingLabel(age())}</span><Icon n="right" /></div>
          </div>
          <div class="srow f" data-k="pe-limit" data-r="22" data-cyc="pe:limit" data-a="noop" data-verb="Alterar">
            <div><b>Tempo de jogo por dia</b><small>O jogo é fechado quando o tempo acaba</small></div>
            <div class="cv"><Icon n="left" /><span>{limit() ? `${limit()} min` : 'Sem limite'}</span><Icon n="right" /></div>
          </div>
        </Show>
        <Show when={!kid()}>
          <div class="srow f" data-k="pe-pin" data-r="22" data-a="pe:pin" data-verb="Definir">
            <div><b>PIN do perfil</b><small>{hasPin() ? 'Protegido · toque para trocar' : 'Sem PIN'}</small></div>
            <Icon n="lock" />
          </div>
          <Show when={hasPin()}>
            <div class="srow f" data-k="pe-nopin" data-r="22" data-a="pe:nopin" data-verb="Remover"><div><b>Remover PIN</b></div><Icon n="trash" /></div>
          </Show>
        </Show>
        <div class="lacts pe-act">
          <div class={`btn pri f ${busy() ? 'off' : ''}`} data-k="pe-save" data-r="p" data-pad="8" data-a="pe:save" data-verb="Salvar"><Icon n="check" /> Salvar</div>
          <Show when={!p.first}><div class="btn f" data-k="pe-cancel" data-r="p" data-pad="8" data-a="pe:cancel" data-verb="Cancelar"><Icon n="x" /> Cancelar</div></Show>
          <Show when={editing() && !p.first}><div class="btn f" data-k="pe-del" data-r="p" data-pad="8" data-a="pe:delete" data-verb="Excluir" style={{ color: 'var(--danger)' }}><Icon n="trash" /> Excluir</div></Show>
        </div>
      </div>
    </>
  );
}

export { setS };
