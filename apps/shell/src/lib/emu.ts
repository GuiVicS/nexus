import type { Item } from '@nexus/protocol';

export const sysOf = (it: Item) => it.tags.find((t) => t.startsWith('sys:'))?.slice(4) ?? '';
export const isEmu = (it: Item) => it.tags.includes('emu');

/** Sigla grande do console no card (marca d'água), como nas capas de sistema do EmulationStation. */
export const SYS_ABBR: Record<string, string> = {
  nes: 'NES', snes: 'SNES', n64: 'N64', gb: 'GB', gbc: 'GBC', gba: 'GBA', nds: 'DS', virtualboy: 'VB', pokemini: 'MINI',
  gamecube: 'GC', wii: 'Wii', megadrive: 'MD', mastersystem: 'SMS', gamegear: 'GG', segacd: 'CD', sega32x: '32X', saturn: 'SAT',
  dreamcast: 'DC', psx: 'PS1', psp: 'PSP', atari2600: '2600', atari5200: '5200', atari7800: '7800', lynx: 'LYNX', jaguar: 'JAG',
  pcengine: 'PCE', neogeo: 'NEO', arcade: 'ARC', mame: 'MAME', ngp: 'NGP', wonderswan: 'WS', colecovision: 'CV', intellivision: 'INTV',
  vectrex: 'VEC', odyssey2: 'O²', '3do': '3DO', msx: 'MSX', c64: 'C64', amiga: 'AMI', dos: 'DOS', scummvm: 'SCUMM', doom: 'DOOM',
};

