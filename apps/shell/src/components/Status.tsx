// Barra de status (A1): rede, Bluetooth, som e controles com bateria no canto da barra superior. Só avisos
// quando há problema (sem internet, bateria de controle <= 20 %, disco quase cheio). Não recebe foco.
import { For, Show } from 'solid-js';
import { status } from '../lib/status';
import { topOv } from '../lib/state';
import { Icon } from './ui';

export function StatusBar() {
  const s = () => status();
  const pads = () => (s()?.bt.connected ?? []).filter((d) => d.kind === 'controle');
  // some durante o jogo (tela "em execução")
  const hidden = () => topOv()?.kind === 'launch';
  return (
    <Show when={s() && !hidden()}>
      <div id="sstat" aria-label="Estado do aparelho">
        <Show when={s()!.net.available}>
          <span class={`si ${s()!.net.online ? '' : 'bad'}`} data-k="st-net" title={s()!.net.kind === 'wifi' ? `Wi-Fi${s()!.net.signal != null ? ` · sinal ${s()!.net.signal}%` : ''}` : s()!.net.kind === 'cabo' ? 'Cabo de rede' : 'Sem rede'}>
            <Icon n={s()!.net.kind === 'cabo' ? 'chip' : 'wifi'} />
            <Show when={!s()!.net.online}><i class="slash" /></Show>
          </span>
        </Show>
        <Show when={s()!.bt.available && s()!.bt.powered}>
          <span class="si" data-k="st-bt" title="Bluetooth ligado"><Icon n="bt" /></span>
        </Show>
        <For each={pads()}>
          {(d) => (
            <span class={`si ${d.battery != null && d.battery <= 20 ? 'bad' : ''}`} data-k="st-pad" title={d.name}>
              <Icon n="pad" />
              <Show when={d.battery != null}><small>{d.battery}%</small></Show>
            </span>
          )}
        </For>
        <Show when={s()!.audio.available}>
          <span class={`si ${s()!.audio.muted ? 'bad' : ''}`} data-k="st-vol" title={s()!.audio.muted ? 'Mudo' : `Volume ${s()!.audio.volume}%`}>
            <Icon n="vol" />
            <Show when={s()!.audio.muted}><i class="slash" /></Show>
          </span>
        </Show>
        <Show when={s()!.alerts[0]}>
          {(a) => (
            <span class="salert" data-k="st-alert" role="status">
              <Icon n={a().icon} />
              <span>{a().text}</span>
              <Show when={s()!.alerts.length > 1}><small>+{s()!.alerts.length - 1}</small></Show>
            </span>
          )}
        </Show>
      </div>
    </Show>
  );
}
