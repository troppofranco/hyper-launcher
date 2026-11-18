// Stato globale dell'interfaccia (rune di Svelte 5).

import { api, on, type Account, type AppInfo, type BlockedFile, type PackImport, type ContentKind, type Instance, type News, type SearchHit, type SearchSort, type Settings, type UpdateInfo, type Versions } from './api';
import { setLang, t } from './i18n.svelte';

export type Route =
  | { view: 'home' }
  | { view: 'library' }
  | { view: 'instance'; name: string }
  | { view: 'explore' }
  | { view: 'project'; id: string; kind: ContentKind }
  | { view: 'customize' }
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

/** Filtri della pagina Esplora: restano uguali tornando dal dettaglio. */
export interface ExploreState {
  text: string;
  kind: ContentKind;
  /** Istanza in cui installare (filtra anche per compatibilità). */
  instance: string | null;
  category: string | null;
  sort: SearchSort;
  /** Filtri usati solo senza istanza (con un'istanza valgono i suoi). */
  gameVersion: string | null;
  loaders: string[];
  layout: 'grid' | 'list';
  /** Catalogo: Modrinth (con ricerca) o CurseForge (in evidenza e per ID). */
  source: 'modrinth' | 'curseforge';
}

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
  /** "Più tardi" sulla finestra di aggiornamento: non la ripropone in questa sessione. */
  updateDismissed = $state(false);
  launches = $state<Record<string, LaunchState>>({});
  logs = $state<Record<string, LogLine[]>>({});
  explore = $state<ExploreState>({
    text: '', kind: 'mod', instance: null, category: null, sort: 'relevance',
    gameVersion: null, loaders: [], layout: 'grid', source: 'modrinth',
  });
  /** Download di contenuti in corso, per istanza (`''` = import di un modpack). */
  contentProgress = $state<Record<string, { done: number; total: number }>>({});
  /** File di un modpack CurseForge da scaricare a mano (apre il dialogo). */
  blockedFiles = $state<BlockedFile[] | null>(null);
  /** Modpack in evidenza nella Home (caricati una volta per sessione). */
  featuredPacks = $state<SearchHit[] | null>(null);
  /** Cresce a ogni partita finita: chi mostra statistiche si ricarica. */
  sessionsRev = $state(0);
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
      on('content-progress', (p) => (this.contentProgress[p.instance] = { done: p.done, total: p.total })),
      on('instance-state', (s) => {
        if (s.state === 'stopped') {
          delete this.launches[s.instance];
          this.sessionsRev++;
          if (s.error) this.error(s.error);
          else if (s.exitCode !== null && s.exitCode !== 0) this.error(t('Minecraft si è chiuso con un errore (codice {codice}). Guarda la console.', { codice: s.exitCode }));
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
    document.documentElement.dataset.theme = settings.theme;
    setLang(settings.language);
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

  /** Apre Esplora già filtrata per un'istanza e un tipo di contenuto. */
  browse(instance: string | null, kind: ContentKind) {
    // Si modifica l'oggetto invece di sostituirlo: la pagina Esplora lo tiene in mano.
    Object.assign(this.explore, { instance, kind, category: null });
    this.go({ view: 'explore' });
  }

  /**
   * Apre il dettaglio di un progetto. Il tipo va passato: Modrinth descrive
   * i data pack come "mod". Con `instance` quella diventa la destinazione.
   */
  openProject(id: string, kind: ContentKind, instance?: string) {
    if (instance) this.explore.instance = instance;
    this.go({ view: 'project', id, kind });
  }

  /** Esegue un'operazione sui contenuti e poi toglie la sua barra di avanzamento. */
  async withContentProgress<T>(key: string, op: () => Promise<T>): Promise<T> {
    this.contentProgress[key] = { done: 0, total: 0 };
    try {
      return await op();
    } finally {
      delete this.contentProgress[key];
    }
  }

  /** Installa un modpack Modrinth come nuova istanza e la apre; `false` se fallisce. */
  async installPack(project: string): Promise<boolean> {
    try {
      const inst = await this.withContentProgress('', () => api.packInstall(project));
      await this.refreshInstances();
      this.info_(t('Modpack installato: nuova istanza “{nome}”.', { nome: inst.name }));
      this.go({ view: 'instance', name: inst.name });
      return true;
    } catch (e) {
      this.error(e);
      return false;
    }
  }

  /** Installa un progetto (con le dipendenze) in un'istanza; `false` se fallisce. */
  async installContent(instance: string, project: string, title: string, kind: ContentKind, world: string | null): Promise<boolean> {
    try {
      const n = await this.withContentProgress(instance, () => api.install(instance, project, kind, world));
      this.info_(n > 1 ? t('{titolo} installato in {istanza}, con {n} dipendenze.', { titolo: title, istanza: instance, n: n - 1 }) : t('{titolo} installato in {istanza}.', { titolo: title, istanza: instance }));
      return true;
    } catch (e) {
      this.error(e);
      return false;
    }
  }

  /** Dopo un import: apre la nuova istanza e segnala i file da prendere a mano. */
  private async afterPackImport(result: PackImport, message: string) {
    await this.refreshInstances();
    this.info_(t(message, { nome: result.instance.name }));
    this.go({ view: 'instance', name: result.instance.name });
    if (result.blocked.length) this.blockedFiles = result.blocked;
  }

  /** Importa un modpack (.mrpack o .zip di CurseForge) scelto dall'utente. */
  async importPack() {
    try {
      const result = await this.withContentProgress('', () => api.packImport());
      if (result) await this.afterPackImport(result, 'Modpack importato: {nome}.');
    } catch (e) {
      this.error(e);
    }
  }

  /** Installa un modpack CurseForge come nuova istanza; `false` se fallisce. */
  async installCfPack(id: number): Promise<boolean> {
    try {
      const result = await this.withContentProgress('', () => api.cfPackInstall(id));
      await this.afterPackImport(result, 'Modpack installato: nuova istanza “{nome}”.');
      return true;
    } catch (e) {
      this.error(e);
      return false;
    }
  }

  /** Installa un progetto CurseForge (con le dipendenze); `false` se fallisce. */
  async installCf(instance: string, id: number, title: string): Promise<boolean> {
    try {
      const n = await this.withContentProgress(instance, () => api.cfInstall(instance, id, null));
      this.info_(n > 1 ? t('{titolo} installato in {istanza}, con {n} dipendenze.', { titolo: title, istanza: instance, n: n - 1 }) : t('{titolo} installato in {istanza}.', { titolo: title, istanza: instance }));
      return true;
    } catch (e) {
      this.error(e);
      return false;
    }
  }

  async play(name: string) {
    if (!this.activeAccount) {
      this.addAccountOpen = true;
      this.info_(t('Aggiungi un account per giocare.'));
      return;
    }
    try {
      await api.launch(name);
    } catch (e) {
      this.error(e);
    }
  }

  error(e: unknown) {
    // I messaggi del backend sono in italiano: si traducono se noti.
    this.toast = { kind: 'error', text: t(typeof e === 'string' ? e : e instanceof Error ? e.message : String(e)) };
  }

  info_(text: string) {
    this.toast = { kind: 'info', text };
  }
}

export const app = new AppStore();
