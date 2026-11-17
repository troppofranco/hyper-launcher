// Ponte verso il backend Rust (comandi ed eventi Tauri). Nel browser
// (`npm run dev`, senza Tauri) usa un backend simulato per provare la UI.

import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { listen as tauriListen, type UnlistenFn } from '@tauri-apps/api/event';
import { mock } from './mock';

export type LoaderKind = 'fabric';
export interface Loader { kind: LoaderKind; name: string; version: string }
export interface Instance {
  name: string;
  version: string;
  loader: Loader | null;
  created: number;
  lastPlayed: number | null;
  playtimeSecs: number;
  memoryMb: number | null;
  javaPath: string | null;
  jvmArgs: string[];
}
export interface Account { name: string; uuid: string; kind: 'offline' | 'microsoft'; default: boolean }
export interface VersionInfo { id: string; kind: string; releaseTime: string }
export interface Versions { latestRelease: string; latestSnapshot: string; versions: VersionInfo[] }
export interface Mod { file: string; name: string; enabled: boolean; sizeBytes: number }
export interface World { name: string; lastPlayed: number | null }
export interface News { title: string; date: string; text: string; image: string | null; url: string }
export interface AppInfo { version: string; build: number | null; dataDir: string }
export interface UpdateInfo { current: number | null; available: number | null }
export interface Settings {
  defaultMemoryMb: number;
  javaPath: string;
  msaClientId: string;
  accent: string;
  minimizeOnLaunch: boolean;
}
export interface InstanceSettings { memoryMb: number | null; javaPath: string | null; jvmArgs: string }

export interface ProgressEvent { instance: string; stage: 'libraries' | 'assets' | 'java'; done: number; total: number; downloadedBytes: number }
export interface LogEvent { instance: string; line: string }
export interface StateEvent { instance: string; state: 'installing' | 'running' | 'stopped'; error: string | null; exitCode: number | null }
export interface DeviceCodeEvent { userCode: string; verificationUri: string; expiresIn: number }

interface Events {
  'launch-progress': ProgressEvent;
  'game-log': LogEvent;
  'instance-state': StateEvent;
  'msa-code': DeviceCodeEvent;
}

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return inTauri ? tauriInvoke<T>(cmd, args) : mock.invoke<T>(cmd, args);
}

export function on<K extends keyof Events>(event: K, handler: (payload: Events[K]) => void): Promise<UnlistenFn> {
  if (!inTauri) return Promise.resolve(mock.listen(event, handler as (p: unknown) => void));
  return tauriListen<Events[K]>(event, (e) => handler(e.payload));
}

export const api = {
  appInfo: () => call<AppInfo>('app_info'),
  listInstances: () => call<Instance[]>('list_instances'),
  createInstance: (input: { name: string; version: string; loader: LoaderKind | null }) =>
    call<Instance>('create_instance', { input }),
  updateInstance: (name: string, settings: InstanceSettings) => call<Instance>('update_instance', { name, settings }),
  deleteInstance: (name: string) => call<void>('delete_instance', { name }),
  listVersions: () => call<Versions>('list_versions'),
  loaderVersions: (loader: LoaderKind, game: string) => call<string[]>('loader_versions', { loader, game }),
  listAccounts: () => call<Account[]>('list_accounts'),
  addOfflineAccount: (name: string) => call<Account[]>('add_offline_account', { name }),
  addMicrosoftAccount: () => call<Account[]>('add_microsoft_account'),
  setDefaultAccount: (name: string) => call<Account[]>('set_default_account', { name }),
  removeAccount: (name: string) => call<Account[]>('remove_account', { name }),
  getSettings: () => call<Settings>('get_settings'),
  saveSettings: (settings: Settings) => call<void>('save_settings', { settings }),
  listMods: (name: string) => call<Mod[]>('list_mods', { name }),
  setModEnabled: (name: string, file: string, enabled: boolean) => call<void>('set_mod_enabled', { name, file, enabled }),
  deleteMod: (name: string, file: string) => call<void>('delete_mod', { name, file }),
  listWorlds: (name: string) => call<World[]>('list_worlds', { name }),
  openFolder: (name: string | null, folder: string) => call<void>('open_folder', { name, folder }),
  openUrl: (url: string) => call<void>('open_url', { url }),
  news: () => call<News[]>('news'),
  checkUpdate: () => call<UpdateInfo>('check_update'),
  installUpdate: () => call<void>('install_update'),
  launch: (name: string) => call<void>('launch_instance', { name }),
};

/** Comandi della finestra (barra del titolo personalizzata). */
export async function windowAction(action: 'minimize' | 'maximize' | 'close') {
  if (!inTauri) return;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const w = getCurrentWindow();
  if (action === 'minimize') await w.minimize();
  else if (action === 'maximize') await w.toggleMaximize();
  else await w.close();
}
