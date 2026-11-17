// Ponte verso il backend Rust (comandi ed eventi Tauri). Nel browser
// (`npm run dev`, senza Tauri) usa un backend simulato per provare la UI.

import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { listen as tauriListen, type UnlistenFn } from '@tauri-apps/api/event';
import { mock } from './mock';

export type LoaderKind = 'fabric' | 'quilt' | 'neoforge' | 'forge';
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
  syncSettings: boolean;
}
export interface Account { name: string; uuid: string; kind: 'offline' | 'microsoft'; default: boolean }
export interface VersionInfo { id: string; kind: string; releaseTime: string }
export interface Versions { latestRelease: string; latestSnapshot: string; versions: VersionInfo[] }
export type ContentKind = 'mod' | 'resourcepack' | 'shader' | 'datapack' | 'modpack';
export type ContentDirName = 'mods' | 'resourcepacks' | 'shaderpacks';
export type SearchSort = 'relevance' | 'downloads' | 'updated';
export interface SearchHit { id: string; slug: string; title: string; description: string; author: string; iconUrl: string | null; image: string | null; color: number | null; downloads: number; updated: string; categories: string[] }
export interface SearchResults { hits: SearchHit[]; total: number }
export interface Project {
  id: string; slug: string; title: string; description: string; projectType: ContentKind; downloads: number;
  iconUrl: string | null; updated: string; categories: string[]; gallery: { url: string; title: string | null }[];
  license: string | null; sourceUrl: string | null; pageUrl: string;
}
export interface PlannedFile { projectId: string; name: string; versionNumber: string; file: string; sizeBytes: number; folder: string; replaces: boolean }
export interface InstallPlan { files: PlannedFile[]; totalBytes: number }
export interface InstalledContent {
  file: string; name: string; enabled: boolean; sizeBytes: number;
  projectId: string | null; slug: string | null; title: string | null; iconUrl: string | null;
  versionNumber: string | null; update: string | null;
}
export interface ExportResult { path: string; linked: number; bundled: number }
export interface SearchInput { text: string; kind: ContentKind; instance: string | null; gameVersion: string | null; loaders: string[]; categories: string[]; sort: SearchSort; offset: number }
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
  sync: SyncSettings;
}
export interface SyncSettings { enabled: boolean; keybinds: boolean; video: boolean; audio: boolean; controls: boolean }
export interface PlaySession { instance: string; start: number; end: number }
export interface InstanceSettings { memoryMb: number | null; javaPath: string | null; jvmArgs: string; syncSettings: boolean }

export interface ProgressEvent { instance: string; stage: 'libraries' | 'assets' | 'java'; done: number; total: number; downloadedBytes: number }
export interface LogEvent { instance: string; line: string }
export interface StateEvent { instance: string; state: 'installing' | 'running' | 'stopped'; error: string | null; exitCode: number | null }
export interface ContentProgressEvent { instance: string; done: number; total: number }
export interface DeviceCodeEvent { userCode: string; verificationUri: string; expiresIn: number }

interface Events {
  'launch-progress': ProgressEvent;
  'game-log': LogEvent;
  'instance-state': StateEvent;
  'msa-code': DeviceCodeEvent;
  'content-progress': ContentProgressEvent;
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
  search: (input: SearchInput) => call<SearchResults>('content_search', { input }),
  project: (id: string) => call<Project>('content_project', { id }),
  plan: (instance: string, project: string, kind: ContentKind, world: string | null) =>
    call<InstallPlan>('content_plan', { instance, project, kind, world }),
  install: (instance: string, project: string, kind: ContentKind, world: string | null) =>
    call<number>('content_install', { instance, project, kind, world }),
  installed: (instance: string, dir: ContentDirName, checkUpdates: boolean) =>
    call<InstalledContent[]>('content_installed', { instance, dir, checkUpdates }),
  setContentEnabled: (instance: string, dir: ContentDirName, file: string, enabled: boolean) =>
    call<void>('set_content_enabled', { instance, dir, file, enabled }),
  deleteContent: (instance: string, dir: ContentDirName, file: string) => call<void>('delete_content', { instance, dir, file }),
  updateContent: (instance: string, dir: ContentDirName, files: string[]) => call<number>('content_update', { instance, dir, files }),
  packInstall: (project: string) => call<Instance>('pack_install', { project }),
  packImport: () => call<Instance | null>('pack_import'),
  packExport: (instance: string) => call<ExportResult | null>('pack_export', { instance }),
  listWorlds: (name: string) => call<World[]>('list_worlds', { name }),
  recentSessions: (days: number) => call<PlaySession[]>('recent_sessions', { days }),
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
