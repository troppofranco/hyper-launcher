// Stato globale dell'interfaccia (rune di Svelte 5).

import { api, on, type Account, type AppInfo, type Instance, type News, type Settings, type UpdateInfo, type Versions } from './api';

export type Route =
  | { view: 'home' }
  | { view: 'library' }
  | { view: 'instance'; name: string }
  | { view: 'settings' };

export interface LaunchState {
  phase: 'installing' | 'running';
  stage: string;
  done: number;
  total: number;
  downloadedBytes: number;
}

/** Riga di console con id crescente, usato come chiave stabile. */
export interface LogLine { id: number; text: string }

const MAX_LOG_LINES = 2500;
const TRIM_TO = 2000;
const NO_PROGRESS = { stage: '', done: 0, total: 0, downloadedBytes: 0 };

class AppStore {
  route = $state<Route>({ view: 'home' });
  instances = $state<Instance[]>([]);
  accounts = $state<Account[]>([]);
  settings = $state<Settings | null>(null);
  info = $state<AppInfo | null>(null);
  news = $state<News[]>([]);
  update = $state<UpdateInfo | null>(null);
  launches = $state<Record<string, LaunchState>>({});
  logs = $state<Record<string, LogLine[]>>({});
  toast = $state<{ kind: 'error' | 'info'; text: string } | null>(null);
  newInstanceOpen = $state(false);
  addAccountOpen = $state(false);
  private versions: Promise<Versions> | null = null;
  private nextLogId = 0;

  activeAccount = $derived(this.accounts.find((a) => a.default) ?? this.accounts[0] ?? null);

  recent = $derived(
    [...this.instances].sort((a, b) => (b.lastPlayed ?? b.created) - (a.lastPlayed ?? a.created)),
  );

  async init() {
    // Tutto in parallelo: nessuna di queste chiamate dipende dalle altre.
    const [info, settings] = await Promise.all([
      api.appInfo(),
      api.getSettings(),
      on('launch-progress', (p) => {
        this.launches[p.instance] = { phase: 'installing', stage: p.stage, done: p.done, total: p.total, downloadedBytes: p.downloadedBytes };
      }),
      on('game-log', (l) => this.appendLog(l.instance, l.line)),
      on('instance-state', (s) => {
        if (s.state === 'stopped') {
          delete this.launches[s.instance];
          if (s.error) this.error(s.error);
          else if (s.exitCode !== null && s.exitCode !== 0) this.error(`Minecraft si è chiuso con un errore (codice ${s.exitCode}). Guarda la console.`);
          void this.refreshInstances();
        } else {
          this.launches[s.instance] = { ...(this.launches[s.instance] ?? NO_PROGRESS), phase: s.state };
          if (s.state === 'installing') this.logs[s.instance] = [];
        }
      }),
      this.refreshInstances(),
      this.refreshAccounts(),
    ]);
    this.info = info;
    this.applySettings(settings);
    // Non bloccanti: la rete potrebbe mancare.
    api.news().then((n) => (this.news = n)).catch(() => (this.news = []));
    api.checkUpdate().then((u) => (this.update = u)).catch(() => {});
  }

  /** Elenco versioni di Minecraft, scaricato una volta per sessione. */
  loadVersions(): Promise<Versions> {
    this.versions ??= api.listVersions().catch((e) => {
      this.versions = null;
      throw e;
    });
    return this.versions;
  }

  private appendLog(instance: string, text: string) {
    const lines = (this.logs[instance] ??= []);
    lines.push({ id: this.nextLogId++, text });
    // Si accorcia a blocchi: togliere una riga alla volta riscriverebbe
    // tutta la console a ogni nuova riga.
    if (lines.length > MAX_LOG_LINES) lines.splice(0, lines.length - TRIM_TO);
  }

  applySettings(settings: Settings) {
    this.settings = settings;
    document.documentElement.style.setProperty('--accent', settings.accent);
  }

  async refreshInstances() {
    try {
      this.instances = await api.listInstances();
    } catch (e) {
      this.error(e);
    }
  }

  async refreshAccounts() {
    try {
      this.accounts = await api.listAccounts();
    } catch (e) {
      this.error(e);
    }
  }

  go(route: Route) {
    this.route = route;
  }

  async play(name: string) {
    if (!this.activeAccount) {
      this.addAccountOpen = true;
      this.info_('Aggiungi un account per giocare.');
      return;
    }
    try {
      await api.launch(name);
    } catch (e) {
      this.error(e);
    }
  }

  error(e: unknown) {
    this.toast = { kind: 'error', text: typeof e === 'string' ? e : e instanceof Error ? e.message : String(e) };
  }

  info_(text: string) {
    this.toast = { kind: 'info', text };
  }
}

export const app = new AppStore();
