import { createMemo, For, Show } from 'solid-js';
import type { Item } from '@nexus/protocol';
import { S } from '../lib/state';
import { norm } from '../lib/format';
import { Icon, Tile } from './ui';
import { Strip } from './Views';
import { isEmu, SYS_ABBR, sysOf } from '../lib/emu';
export { isEmu, SYS_ABBR, sysOf };

/** Console (sistema) com jogos na pasta de ROMs — derivado dos itens do provedor "emu" (tag `sys:<pasta>`). */
export interface Sys {
  dir: string;
  name: string;
  hue: [number, number];
  games: Item[];
}

export function systems(): Sys[] {
  const by = new Map<string, Sys>();
  for (const it of S.lib) {
    if (!isEmu(it)) continue;
    const dir = sysOf(it);
    const s = by.get(dir) ?? { dir, name: it.category ?? dir, hue: it.hue as [number, number], games: [] };
    s.games.push(it);
    by.set(dir, s);
  }
  return [...by.values()].sort((a, b) => a.name.localeCompare(b.name));
}

const grad = (h: [number, number]) => `linear-gradient(135deg,hsl(${h[0]} 60% 36%),hsl(${h[1]} 60% 16%))`;

export function Emulators() {
  const sys = createMemo(systems);
  const cur = createMemo(() => sys().find((s) => s.dir === S.emuSys) ?? null);
  const total = () => sys().reduce((n, s) => n + s.games.length, 0);
  return (
    <Show when={cur()} fallback={
      <>
        <div class="strip w" id="strip">
          <h2>Emuladores</h2>
          <p>{sys().length ? `${sys().length} ${sys().length === 1 ? 'console' : 'consoles'} · ${total()} ${total() === 1 ? 'jogo' : 'jogos'}` : 'Nenhum jogo de console ainda'}</p>
        </div>
        <div class="sc w" id="sc">
          <div class="sysgrid">
            <For each={sys()}>
              {(s) => (
                <div class="sysc f" data-k={`sys-${s.dir}`} data-r="26" data-a={`emusys:${s.dir}`} data-verb="Abrir" aria-label={s.name} style={{ background: grad(s.hue) }}>
                  <span class="ab">{SYS_ABBR[s.dir] ?? s.dir.toUpperCase()}</span>
                  <b>{s.name}</b>
                  <small>{s.games.length} {s.games.length === 1 ? 'jogo' : 'jogos'}</small>
                </div>
              )}
            </For>
            <Show when={!S.session || S.session.profile.kind !== 'kid'}>
              <div class="sysc add f" data-k="sys-add" data-r="26" data-a="romsadd" data-verb="Abrir" aria-label="Adicionar jogos">
                <Icon n="plus" />
                <b>Adicionar jogos</b>
                <small>Pendrive, pasta ou celular</small>
              </div>
            </Show>
          </div>
        </div>
      </>
    }>
      {(s) => {
        const games = createMemo(() => [...s().games].sort((a, b) => norm(a.title).localeCompare(norm(b.title))));
        return (
          <>
            <Strip wide fallbackTitle={s().name} fallbackSub={`${games().length} ${games().length === 1 ? 'jogo' : 'jogos'}`} />
            <div class="sc w" id="sc">
              <div class="gridt">
                <For each={games()}>{(g) => <Tile it={g} a={`open:${g.id}`} />}</For>
              </div>
            </div>
          </>
        );
      }}
    </Show>
  );
}
