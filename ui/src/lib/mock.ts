// Backend simulato per `npm run dev` nel browser: stessi comandi ed eventi
// della app vera, con dati di esempio in memoria. Non entra nella app Tauri
// se non come codice inutilizzato.

import type { Account, Instance, Mod, Settings } from './api';

type Handler = (payload: unknown) => void;
const handlers = new Map<string, Set<Handler>>();
const now = () => Math.floor(Date.now() / 1000);

const state: { instances: Instance[]; accounts: Account[]; settings: Settings; mods: Mod[] } = {
  instances: [
    { name: 'Sopravvivenza', version: '26.3', loader: { kind: 'fabric', name: 'Fabric', version: '0.20.1' }, created: now() - 86400 * 3, lastPlayed: now() - 3600, playtimeSecs: 5880, memoryMb: 6144, javaPath: null, jvmArgs: [] },
    { name: 'Minecraft 26.3', version: '26.3', loader: null, created: now() - 86400 * 5, lastPlayed: now() - 86400, playtimeSecs: 720, memoryMb: null, javaPath: null, jvmArgs: [] },
    { name: 'Snapshot', version: '26.4-snapshot-8', loader: null, created: now() - 3600, lastPlayed: null, playtimeSecs: 0, memoryMb: null, javaPath: null, jvmArgs: [] },
  ],
  accounts: [
    { name: 'TroppoFranco', uuid: '069a79f4-44e9-4726-a5be-fca90e38aaf5', kind: 'microsoft', default: true },
    { name: 'Dev', uuid: '380df991-f603-344c-a090-369bad2a924a', kind: 'offline', default: false },
  ],
  settings: { defaultMemoryMb: 4096, javaPath: '', msaClientId: '', accent: '#6e5ae6', minimizeOnLaunch: true },
  mods: [
    { file: 'sodium-fabric-0.9.2.jar', name: 'sodium-fabric-0.9.2', enabled: true, sizeBytes: 1_200_000 },
    { file: 'fabric-api-0.146.2.jar', name: 'fabric-api-0.146.2', enabled: true, sizeBytes: 2_400_000 },
    { file: 'lithium-fabric-0.26.3.jar.disabled', name: 'lithium-fabric-0.26.3', enabled: false, sizeBytes: 800_000 },
  ],
};

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
    const inst = { name: input.name || `${loader ? 'Fabric' : 'Minecraft'} ${input.version}`, version: input.version, loader, created: now(), lastPlayed: null, playtimeSecs: 0, memoryMb: null, javaPath: null, jvmArgs: [] };
    state.instances.push(inst);
    return structuredClone(inst);
  },
  update_instance: ({ name, settings }) => {
    const inst = state.instances.find((i) => i.name === name)!;
    Object.assign(inst, { memoryMb: settings.memoryMb, javaPath: settings.javaPath, jvmArgs: settings.jvmArgs.split(/\s+/).filter(Boolean) });
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
  list_mods: () => structuredClone(state.mods),
  set_mod_enabled: ({ file, enabled }) => {
    const m = state.mods.find((x) => x.file === file)!;
    m.enabled = enabled;
    m.file = enabled ? m.file.replace(/\.disabled$/, '') : `${m.file}.disabled`;
  },
  delete_mod: ({ file }) => { state.mods = state.mods.filter((m) => m.file !== file); },
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
