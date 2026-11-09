import type { GameSession, Item, Notification, Profile, RemoteDevice, SessionInfo } from './generated';

/** Eventos enviados pelo Core (notificações JSON-RPC) */
export interface Events {
  'library.changed': { ids: string[]; removed: string[] };
  'game.launching': { session: GameSession };
  'game.started': { session: GameSession };
  'game.exited': { itemId: string; code: number | null; playSec: number };
  'game.failed': { itemId: string; reason: string };
  'session.changed': SessionInfo | null;
  'profiles.changed': Record<string, never>;
  'notification.added': Notification;
  'parental.warning': { minutes: number };
  'parental.limit_reached': { profileId: string };
  'remote.devices': { devices: RemoteDevice[] };
  'remote.input': { key?: string; down?: boolean; reset?: boolean };
  'remote.activity': { mode: string };
  'remote.pointer': { dx: number; dy: number };
  'remote.button': { b: string; down: boolean };
  'remote.scroll': { v: number };
  'remote.key': { key: string; mods: string[] };
  'remote.text': { back: number; text: string };
  'input.guide': Record<string, never>;
  'system.power': { action: string; simulated: boolean };
}

export type { Profile };
