import type { GameSession, Item, Notification, Profile, RemoteDevice, SessionInfo } from './generated';
import type { AppUpdate, StatusSnapshot } from './api';

/** Eventos enviados pelo Core (notificações JSON-RPC) */
export interface Events {
  'library.changed': { ids: string[]; removed: string[] };
  'game.launching': { session: GameSession };
  'game.started': { session: GameSession };
  'game.exited': { itemId: string; code: number | null; playSec: number };
  'game.failed': { itemId: string; reason: string };
  'game.waiting': { itemId: string; hint: string };
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
  'input.guide': { frame?: number | null; toggle?: boolean; close?: boolean; ms?: number };
  'power.button': { game: string | null };
  'input.emergency': { title: string };
  'game.toast': { title: string; body: string; secs?: number };
  'steam.ui': { open: boolean };
  'steam.dialog': { open: boolean; title?: string };
  'steam.eula': { itemId: string; title: string };
  'roms.progress': { done: number; total: number; name: string };
  'roms.received': { name: string; size: number };
  'apps.updates': AppUpdate[];
  'apps.job': { id: string; op: 'update' | 'restore'; state: 'running' | 'done' | 'failed'; error?: string | null };
  'status.changed': StatusSnapshot;
  'system.power': { action: string; simulated: boolean };
}

export type { Profile };
