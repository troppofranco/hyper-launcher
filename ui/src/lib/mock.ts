// Backend simulato per `npm run dev` nel browser: stessi comandi ed eventi
// della app vera, con dati di esempio in memoria. Non entra nella app Tauri
// se non come codice inutilizzato.

import type { Account, InstalledContent, Instance, SearchHit, Settings } from './api';

type Handler = (payload: unknown) => void;
const handlers = new Map<string, Set<Handler>>();
const now = () => Math.floor(Date.now() / 1000);

const state: { instances: Instance[]; accounts: Account[]; settings: Settings; mods: InstalledContent[] } = {
  instances: [
    { name: 'Sopravvivenza', version: '26.3', loader: { kind: 'fabric', name: 'Fabric', version: '0.20.1' }, created: now() - 86400 * 3, lastPlayed: now() - 3600, playtimeSecs: 5880, memoryMb: 6144, javaPath: null, jvmArgs: [], syncSettings: true },
    { name: 'Minecraft 26.3', version: '26.3', loader: null, created: now() - 86400 * 5, lastPlayed: now() - 86400, playtimeSecs: 720, memoryMb: null, javaPath: null, jvmArgs: [], syncSettings: true },
    { name: 'Snapshot', version: '26.4-snapshot-8', loader: null, created: now() - 3600, lastPlayed: null, playtimeSecs: 0, memoryMb: null, javaPath: null, jvmArgs: [], syncSettings: true },
  ],
  accounts: [
    { name: 'TroppoFranco', uuid: '069a79f4-44e9-4726-a5be-fca90e38aaf5', kind: 'microsoft', default: true },
    { name: 'Dev', uuid: '380df991-f603-344c-a090-369bad2a924a', kind: 'offline', default: false },
  ],
  settings: { defaultMemoryMb: 4096, javaPath: '', msaClientId: '', accent: '#6e5ae6', minimizeOnLaunch: true, sync: { enabled: false, keybinds: true, video: true, audio: true, controls: true } },
  mods: [
    { file: 'sodium-fabric-0.9.2+mc26.3.jar', name: 'sodium-fabric-0.9.2+mc26.3', enabled: true, sizeBytes: 1_200_000, projectId: 'AANobbMI', slug: 'sodium', title: 'Sodium', iconUrl: null, versionNumber: 'mc26.3-0.9.2-fabric', update: 'mc26.3-0.9.3-fabric' },
    { file: 'fabric-api-0.166.2+26.3.jar', name: 'fabric-api-0.166.2+26.3', enabled: true, sizeBytes: 2_400_000, projectId: 'P7dR8mSH', slug: 'fabric-api', title: 'Fabric API', iconUrl: null, versionNumber: '0.166.2+26.3', update: null },
    { file: 'modmenu-21.0.1.jar.disabled', name: 'modmenu-21.0.1', enabled: false, sizeBytes: 800_000, projectId: 'mOgUt4GM', slug: 'modmenu', title: 'Mod Menu', iconUrl: null, versionNumber: '21.0.1', update: null },
    { file: 'mia-mod-test.jar', name: 'mia-mod-test', enabled: true, sizeBytes: 40_000, projectId: null, slug: null, title: null, iconUrl: null, versionNumber: null, update: null },
  ],
};

const HITS: SearchHit[] = [
  { id: 'AANobbMI', slug: 'sodium', title: 'Sodium', description: 'A high-performance rendering engine replacement for Minecraft.', author: 'jellysquid3', iconUrl: null, image: null, color: 0x2f6fdf, downloads: 253_432_494, updated: '2026-11-12T01:12:22Z', categories: ['optimization'] },
  { id: 'YL57xq9U', slug: 'iris', title: 'Iris Shaders', description: 'A modern shader pack loader for Minecraft intended to be compatible with OptiFine shaders.', author: 'coderbot', iconUrl: null, image: null, color: 0x2f6fdf, downloads: 120_000_000, updated: '2026-09-30T02:57:43Z', categories: ['optimization'] },
  { id: 'gvQqBUqZ', slug: 'lithium', title: 'Lithium', description: 'No-compromises game logic optimization mod.', author: 'jellysquid3', iconUrl: null, image: null, color: 0x2f6fdf, downloads: 80_000_000, updated: '2026-11-05T00:00:00Z', categories: ['optimization'] },
  { id: 'NNAgCjsB', slug: 'entityculling', title: 'Entity Culling', description: 'Using async path-tracing to hide Block-/Entities that are not visible.', author: 'tr7zw', iconUrl: null, image: null, color: 0x2f6fdf, downloads: 60_000_000, updated: '2026-10-20T00:00:00Z', categories: ['optimization'] },
];

function simulateDownload(instance: string, total: number) {
  return new Promise<void>((resolve) => {
    let done = 0;
    const timer = setInterval(() => {
      done += 1;
      emit('content-progress', { instance, done, total });
      if (done >= total) { clearInterval(timer); resolve(); }
    }, 350);
  });
}

function emit(event: string, payload: unknown) {
  handlers.get(event)?.forEach((h) => h(payload));
}

function simulateLaunch(name: string) {
  const inst = state.instances.find((i) => i.name === name);

  emit('instance-state', { instance: name, state: 'installing', error: null, exitCode: null });
  let done = 0;
  const total = 5149;
  const timer = setInterval(() => {
    done = Math.min(total, done + 400);
    emit('launch-progress', { instance: name, stage: 'assets', done, total, downloadedBytes: done * 110_000 });
    if (done >= total) {
      clearInterval(timer);
      emit('instance-state', { instance: name, state: 'running', error: null, exitCode: null });
      ['[UTC] [main/INFO]: Loading Minecraft 26.3 with Fabric Loader 0.20.1', '[UTC] [Render thread/INFO]: Setting user: TroppoFranco', '[UTC] [Render thread/INFO]: Sound engine started']
        .forEach((line, i) => setTimeout(() => emit('game-log', { instance: name, line }), 300 * (i + 1)));
      setTimeout(() => {
        if (inst) { inst.lastPlayed = now(); inst.playtimeSecs += 60; }
        emit('instance-state', { instance: name, state: 'stopped', error: null, exitCode: 0 });
      }, 4000);
    }
  }, 150);
}

const commands: Record<string, (args: Record<string, any>) => unknown> = {
  app_info: () => ({ version: '0.1.0', build: null, dataDir: 'C:\\Users\\utente\\AppData\\Roaming\\HyperLauncher' }),
  list_instances: () => structuredClone(state.instances),
  create_instance: ({ input }) => {
    const loader = input.loader === 'fabric' ? { kind: 'fabric' as const, name: 'Fabric', version: '0.20.1' } : null;
    const inst = { name: input.name || `${loader ? 'Fabric' : 'Minecraft'} ${input.version}`, version: input.version, loader, created: now(), lastPlayed: null, playtimeSecs: 0, memoryMb: null, javaPath: null, jvmArgs: [], syncSettings: true };
    state.instances.push(inst);
    return structuredClone(inst);
  },
  update_instance: ({ name, settings }) => {
    const inst = state.instances.find((i) => i.name === name)!;
    Object.assign(inst, { memoryMb: settings.memoryMb, javaPath: settings.javaPath, jvmArgs: settings.jvmArgs.split(/\s+/).filter(Boolean), syncSettings: settings.syncSettings });
    return structuredClone(inst);
  },
  delete_instance: ({ name }) => { state.instances = state.instances.filter((i) => i.name !== name); },
  list_versions: () => ({
    latestRelease: '26.3', latestSnapshot: '26.4-snapshot-8',
    versions: [
      { id: '26.4-snapshot-8', kind: 'snapshot', releaseTime: '2026-11-10' },
      { id: '26.3', kind: 'release', releaseTime: '2026-09-15' },
      { id: '26.2', kind: 'release', releaseTime: '2026-06-16' },
      { id: '26.1.2', kind: 'release', releaseTime: '2026-04-09' },
    ],
  }),
  loader_versions: ({ game }) => (game.includes('snapshot') ? [] : ['0.20.1', '0.20.0']),
  list_accounts: () => structuredClone(state.accounts),
  add_offline_account: ({ name }) => {
    state.accounts.forEach((a) => (a.default = false));
    state.accounts.push({ name, uuid: crypto.randomUUID(), kind: 'offline', default: true });
    return structuredClone(state.accounts);
  },
  add_microsoft_account: () => {
    emit('msa-code', { userCode: 'ABCD-1234', verificationUri: 'https://microsoft.com/link', expiresIn: 900 });
    return new Promise((_, reject) => setTimeout(() => reject('Simulazione: nel browser il login Microsoft non è disponibile.'), 4000));
  },
  set_default_account: ({ name }) => {
    state.accounts.forEach((a) => (a.default = a.name === name));
    return structuredClone(state.accounts);
  },
  remove_account: ({ name }) => {
    state.accounts = state.accounts.filter((a) => a.name !== name);
    return structuredClone(state.accounts);
  },
  get_settings: () => structuredClone(state.settings),
  save_settings: ({ settings }) => { state.settings = settings; },
  content_search: ({ input }) => {
    const q = String(input.text).toLowerCase();
    const hits = HITS.filter((h) => !q || h.title.toLowerCase().includes(q) || h.description.toLowerCase().includes(q));
    return { hits: input.offset ? [] : hits, total: hits.length };
  },
  content_project: ({ id }) => {
    const h = HITS.find((x) => x.id === id) ?? HITS[0];
    return { id: h.id, slug: h.slug, title: h.title, description: h.description, projectType: 'mod', downloads: h.downloads, iconUrl: null, updated: h.updated, categories: h.categories, gallery: [], license: 'LGPL-3.0', sourceUrl: null, pageUrl: `https://modrinth.com/mod/${h.slug}` };
  },
  content_plan: ({ project }) => {
    const h = HITS.find((x) => x.id === project) ?? HITS[0];
    const files = [{ projectId: h.id, name: h.title, versionNumber: '1.0.0', file: `${h.slug}.jar`, sizeBytes: 1_500_000, folder: 'mods', replaces: false }];
    if (h.slug === 'iris') files.push({ projectId: 'AANobbMI', name: 'Sodium', versionNumber: '0.9.2', file: 'sodium.jar', sizeBytes: 1_200_000, folder: 'mods', replaces: false });
    return { files, totalBytes: files.reduce((n, f) => n + f.sizeBytes, 0) };
  },
  content_install: async ({ instance }) => { await simulateDownload(instance, 2); return 2; },
  content_installed: ({ dir }) => (dir === 'mods' ? structuredClone(state.mods) : []),
  set_content_enabled: ({ file, enabled }) => {
    const m = state.mods.find((x) => x.file === file)!;
    m.enabled = enabled;
    m.file = enabled ? m.file.replace(/\.disabled$/, '') : `${m.file}.disabled`;
  },
  delete_content: ({ file }) => { state.mods = state.mods.filter((m) => m.file !== file); },
  content_update: async ({ instance, files }) => {
    const targets = state.mods.filter((m) => m.update && (!files.length || files.includes(m.file)));
    await simulateDownload(instance, Math.max(1, targets.length));
    targets.forEach((m) => { m.versionNumber = m.update; m.update = null; });
    return targets.length;
  },
  pack_install: async () => {
    await simulateDownload('', 4);
    const inst = { name: 'Fabulously Optimized', version: '26.3', loader: { kind: 'fabric' as const, name: 'Fabric', version: '0.20.1' }, created: now(), lastPlayed: null, playtimeSecs: 0, memoryMb: null, javaPath: null, jvmArgs: [], syncSettings: true };
    state.instances.push(inst);
    return structuredClone(inst);
  },
  pack_import: () => null,
  pack_export: ({ instance }) => ({ path: `C:\\Users\\utente\\Documents\\${instance}.mrpack`, linked: 3, bundled: 1 }),
  recent_sessions: () => {
    const day = 86400;
    const t = now();
    return [
      { instance: 'Sopravvivenza', start: t - day * 3 - 5400, end: t - day * 3 },
      { instance: 'Sopravvivenza', start: t - day - 1800, end: t - day },
      { instance: 'Minecraft 26.3', start: t - 2600, end: t - 600 },
    ];
  },
  list_worlds: () => [{ name: 'Nuovo mondo', lastPlayed: now() - 3600 }, { name: 'Creativa', lastPlayed: now() - 86400 * 3 }],
  open_folder: () => undefined,
  open_url: ({ url }) => { window.open(url, '_blank'); },
  news: () => [
    { title: 'Halloween on Realms', date: '2026-10-30', text: '12 new maps to send a shiver down your spine!', image: null, url: 'https://www.minecraft.net/' },
    { title: 'New on Java Realms: Grinding & Galaxy', date: '2026-09-28', text: '', image: null, url: 'https://www.minecraft.net/' },
  ],
  check_update: () => ({ current: null, available: null }),
  install_update: () => undefined,
  launch_instance: ({ name }) => { simulateLaunch(name); },
};

export const mock = {
  async invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
    await new Promise((r) => setTimeout(r, 120));
    const fn = commands[cmd];
    if (!fn) throw new Error(`comando simulato mancante: ${cmd}`);
    return (await fn(args)) as T;
  },
  listen(event: string, handler: Handler) {
    if (!handlers.has(event)) handlers.set(event, new Set());
    handlers.get(event)!.add(handler);
    return () => handlers.get(event)?.delete(handler);
  },
};
