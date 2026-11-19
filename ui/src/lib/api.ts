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
  /** Immagine scelta dall'utente (data: URL). */
  icon: string | null;
}
export interface Skin { id: string; name: string; slim: boolean; image: string }
export interface Skins { skins: Skin[]; selected: string | null; offline: boolean }
export interface SavedServer { name: string; address: string; icon: string | null; instances: string[] }
export interface ServerStatus { online: number; max: number; version: string; motd: string; latencyMs: number }
export interface Account { name: string; uuid: string; kind: 'offline' | 'microsoft'; default: boolean; skin: string | null }
export interface VersionInfo { id: string; kind: string; releaseTime: string }
export interface Versions { latestRelease: string; latestSnapshot: string; versions: VersionInfo[] }
export type ContentKind = 'mod' | 'resourcepack' | 'shader' | 'datapack' | 'modpack' | 'plugin';
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
  /** Dove è stato riconosciuto il file. */
  source: 'modrinth' | 'curseforge' | null;
  /** Pagina del progetto (solo CurseForge). */
  website: string | null;
}
/** Un progetto CurseForge. */
export interface CfProject {
  id: number; name: string; summary: string; author: string; iconUrl: string | null;
  downloads: number; kind: ContentKind | null; website: string; updated: string;
}
export interface CfFeatured { featured: CfProject[]; popular: CfProject[]; recent: CfProject[] }
/** File di un modpack da scaricare a mano dal sito CurseForge. */
export interface BlockedFile { name: string; fileName: string; url: string }
export interface PackImport { instance: Instance; blocked: BlockedFile[] }
export interface ExportResult { path: string; linked: number; bundled: number }
export interface SearchInput { text: string; kind: ContentKind; instance: string | null; gameVersion: string | null; loaders: string[]; categories: string[]; sort: SearchSort; offset: number }
export interface World { name: string; lastPlayed: number | null }
export interface News { title: string; date: string; text: string; image: string | null; url: string }
export interface AppInfo { version: string; dev: boolean; dataDir: string; curseforge: boolean; windowEffect: boolean }
export interface UpdateInfo { current: string | null; available: string | null; url: string | null }
export interface Settings {
  defaultMemoryMb: number;
  javaPath: string;
  msaClientId: string;
  accent: string;
  minimizeOnLaunch: boolean;
  showConsole: boolean;
  theme: 'dark' | 'light';
  language: 'it' | 'en';
  setupDone: boolean;
  sync: SyncSettings;
  appearance: Appearance;
  downloadConcurrency: number;
  /** Argomenti JVM predefiniti, separati da spazi. */
  defaultJvmArgs: string;
  gameWidth: number | null;
  gameHeight: number | null;
  gameFullscreen: boolean;
}
export type WidgetSize = 'small' | 'medium' | 'large';
export interface WidgetConfig { kind: string; size: WidgetSize; options: Record<string, number> }
export interface Appearance {
  background: 'solid' | 'transparent' | 'image';
  /** Intensità della sfocatura: blur = leggera, acrylic = media, mica = forte. */
  effect: 'blur' | 'acrylic' | 'mica';
  blur: number;
  dim: number;
  panelOpacity: number;
  uiScale: number;
  radius: 'sharp' | 'medium' | 'round';
  buttons: 'standard' | 'liquid';
  animations: 'full' | 'reduced' | 'off';
  sidebarSide: 'left' | 'right';
  sidebarLabels: boolean;
  sidebarInstances: number;
  showNews: boolean;
  wideContent: boolean;
  /** null = Home predefinita (diversa da una Home svuotata). */
  home: WidgetConfig[] | null;
}
export interface StorageUsage { instances: number; libraries: number; assets: number; versions: number; runtimes: number; cache: number }
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
  'server-progress': ServerProgressEvent;
  'playit-claim': PlayitClaimStatus;
  'playit-progress': PlayitProgressEvent;
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
  /** Salva; restituisce se lo sfondo trasparente è attivo. */
  saveSettings: (settings: Settings) => call<boolean>('save_settings', { settings }),
  background: () => call<string | null>('background'),
  setBackground: (image: string | null) => call<void>('set_background', { image }),
  storageUsage: () => call<StorageUsage>('storage_usage'),
  clearDownloadCache: () => call<void>('clear_download_cache'),
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
  packImport: () => call<PackImport | null>('pack_import'),
  cfFeatured: (kind: ContentKind, instance: string | null) => call<CfFeatured>('cf_featured', { kind, instance }),
  cfProject: (id: number) => call<CfProject>('cf_project', { id }),
  cfInstall: (instance: string, id: number, world: string | null) => call<number>('cf_install', { instance, id, world }),
  cfPackInstall: (id: number) => call<PackImport>('cf_pack_install', { id }),
  packExport: (instance: string) => call<ExportResult | null>('pack_export', { instance }),
  listWorlds: (name: string) => call<World[]>('list_worlds', { name }),
  setInstanceIcon: (name: string, png: string | null) => call<Instance>('set_instance_icon', { name, png }),
  listSkins: () => call<Skins>('list_skins'),
  defaultSkins: () => call<Skin[]>('default_skins'),
  addSkin: (name: string, png: string, slim: boolean) => call<Skins>('add_skin', { name, png, slim }),
  removeSkin: (id: string) => call<Skins>('remove_skin', { id }),
  selectSkin: (id: string | null) => call<Skins>('select_skin', { id }),
  listServers: () => call<SavedServer[]>('list_servers'),
  pingServer: (address: string) => call<ServerStatus>('ping_server', { address }),
  recentSessions: (days: number) => call<PlaySession[]>('recent_sessions', { days }),
  openFolder: (name: string | null, folder: string) => call<void>('open_folder', { name, folder }),
  openUrl: (url: string) => call<void>('open_url', { url }),
  news: () => call<News[]>('news'),
  checkUpdate: () => call<UpdateInfo>('check_update'),
  installUpdate: () => call<void>('install_update'),
  launch: (name: string) => call<void>('launch_instance', { name }),

  // ---------- Server ospitati ----------
  serverList: () => call<ServerInfo[]>('server_list'),
  /** Versioni disponibili per un tipo di server (dalla più recente). */
  serverVersions: (kind: ServerKind) => call<string[]>('server_versions', { kind }),
  /** Crea, scarica e prepara un server; avanzamento con l'evento `server-progress`. */
  serverCreate: (input: NewServer) => call<ServerInfo>('server_create', { input }),
  serverDelete: (name: string) => call<void>('server_delete', { name }),
  serverStart: (name: string) => call<void>('server_start', { name }),
  serverStop: (name: string) => call<void>('server_stop', { name }),
  serverRestart: (name: string) => call<void>('server_restart', { name }),
  /** Invia una riga alla console del server (es. `say ciao`, `op Marco`). */
  serverCommand: (name: string, line: string) => call<void>('server_command', { name, line }),
  /** Righe di console dalla numero `since` in poi. */
  serverConsole: (name: string, since: number) => call<ConsoleChunk>('server_console', { name, since }),
  serverStats: (name: string) => call<ServerStats>('server_stats', { name }),
  serverUpdate: (name: string, update: ServerUpdate) => call<ServerInfo>('server_update', { name, update }),
  /** Imposta (PNG 64×64 in base64) o toglie l'immagine del server. */
  serverSetIcon: (name: string, png: string | null) => call<ServerInfo>('server_set_icon', { name, png }),
  /** playit.gg collegato a questo PC? */
  playitStatus: () => call<{ connected: boolean }>('playit_status'),
  /** Apre il browser per approvare e aspetta (evento `playit-claim`). */
  playitConnect: () => call<void>('playit_connect'),
  playitDisconnect: () => call<void>('playit_disconnect'),
  /** Rende il server raggiungibile da internet (evento `playit-progress`). */
  serverPlayitEnable: (name: string) => call<ServerInfo>('server_playit_enable', { name }),
  serverPlayitDisable: (name: string) => call<ServerInfo>('server_playit_disable', { name }),
  serverPlayers: (name: string) => call<ServerPlayers>('server_players', { name }),
  serverProperties: (name: string) => call<ServerProperties>('server_properties', { name }),
  serverSaveProperties: (name: string, properties: ServerProperties) => call<void>('server_save_properties', { name, properties }),
  /** File di una cartella del server (`''` = cartella principale). */
  serverFiles: (name: string, path: string) => call<ServerFile[]>('server_files', { name, path }),
  serverReadFile: (name: string, path: string) => call<string>('server_read_file', { name, path }),
  serverWriteFile: (name: string, path: string, content: string) => call<void>('server_write_file', { name, path, content }),
  serverDeleteFile: (name: string, path: string) => call<void>('server_delete_file', { name, path }),
  serverPlugins: (name: string) => call<ServerPlugin[]>('server_plugins', { name }),
  /** Installa un plugin da Modrinth con le dipendenze; restituisce i file scaricati. */
  serverPluginInstall: (name: string, project: string) => call<string[]>('server_plugin_install', { name, project }),
  /** Attiva o disattiva un plugin; restituisce il nuovo nome del file. */
  serverPluginSetEnabled: (name: string, file: string, enabled: boolean) => call<string>('server_plugin_set_enabled', { name, file, enabled }),
  serverPluginDelete: (name: string, file: string) => call<void>('server_plugin_delete', { name, file }),
};

export type ServerKind = 'vanilla' | 'paper';
export interface ServerInfo {
  name: string;
  kind: ServerKind;
  version: string;
  /** Build di Paper (null per Vanilla). */
  build: number | null;
  memoryMb: number;
  port: number;
  keepRunning: boolean;
  autostart: boolean;
  created: number;
  running: boolean;
  /** Secondi Unix dell'avvio, se acceso. */
  startedAt: number | null;
  players: number;
  maxPlayers: number;
  /** Indirizzo nella rete di casa (es. 192.168.1.20:25565). */
  address: string;
  /** Immagine del server (PNG 64×64 come `data:` URL): logo qui e icona nel gioco. */
  icon: string | null;
  /** Indirizzo pubblico (playit.gg) se l'accesso da internet è attivo. */
  publicAddress: string | null;
}
export interface NewServer { name: string; kind: ServerKind; version: string; memoryMb: number; port: number; keepRunning: boolean; autostart: boolean; eula: boolean }
export interface ServerUpdate { memoryMb: number; keepRunning: boolean; autostart: boolean }
export interface ConsoleChunk { lines: string[]; next: number }
/** Un campione ogni 2 s (ultimi 5 minuti). */
export interface StatsPoint { t: number; cpu: number; memMb: number; players: number; latencyMs: number | null }
export interface ServerStats { points: StatsPoint[]; memMaxMb: number; online: string[] }
export interface ServerFile { name: string; dir: boolean; size: number; modified: number }
/** Un plugin nella cartella `plugins/` di un server Paper. */
export interface ServerPlugin {
  file: string; enabled: boolean; sizeBytes: number;
  projectId: string | null; title: string | null; iconUrl: string | null; versionNumber: string | null;
}
export interface PlayerEntry { name: string; uuid: string }
export interface ServerPlayers { online: string[]; ops: PlayerEntry[]; whitelist: PlayerEntry[]; banned: (PlayerEntry & { reason: string })[] }
export interface ServerProperties {
  maxPlayers: number;
  /** false = entrano anche gli account offline. */
  onlineMode: boolean;
  motd: string;
  difficulty: 'peaceful' | 'easy' | 'normal' | 'hard';
  gamemode: 'survival' | 'creative' | 'adventure' | 'spectator';
  pvp: boolean;
  whitelist: boolean;
  viewDistance: number;
  simulationDistance: number;
  hardcore: boolean;
  allowFlight: boolean;
  spawnProtection: number;
  seed: string;
  port: number;
}
export interface ServerProgressEvent { name: string; stage: 'download' | 'java' | 'setup'; done: number; total: number }
/** Stato dell'approvazione su playit.gg (nel browser). */
export type PlayitClaimStatus = 'WaitingForUserVisit' | 'WaitingForUser' | 'UserAccepted' | 'UserRejected';
export interface PlayitProgressEvent { name: string; stage: 'agent' | 'tunnel' | 'address' }

/** Comandi della finestra (barra del titolo personalizzata). */
export async function windowAction(action: 'minimize' | 'maximize' | 'close') {
  if (!inTauri) return;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const w = getCurrentWindow();
  if (action === 'minimize') await w.minimize();
  else if (action === 'maximize') await w.toggleMaximize();
  else await w.close();
}
