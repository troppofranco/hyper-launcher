// Backend simulato per `npm run dev` nel browser: stessi comandi ed eventi
// della app vera, con dati di esempio in memoria. Non entra nella app Tauri
// se non come codice inutilizzato.

import type { Route } from './store.svelte';
import type { Account, BackupInfo, BedrockKind, BedrockPack, InstalledContent, Instance, SearchHit, ServerInfo, ServerPlayers, ServerPlugin, ServerProperties, Settings, StatsPoint } from './api';

type Handler = (payload: unknown) => void;
const handlers = new Map<string, Set<Handler>>();
const now = () => Math.floor(Date.now() / 1000);

const state: { instances: Instance[]; accounts: Account[]; settings: Settings; mods: InstalledContent[] } = {
  instances: [
    { name: 'Sopravvivenza', version: '26.3', loader: { kind: 'fabric', name: 'Fabric', version: '0.20.1' }, created: now() - 86400 * 3, lastPlayed: now() - 3600, playtimeSecs: 5880, memoryMb: 6144, javaPath: null, jvmArgs: [], syncSettings: true, icon: null, edition: 'java' as const },
    { name: 'Minecraft 26.3', version: '26.3', loader: null, created: now() - 86400 * 5, lastPlayed: now() - 86400, playtimeSecs: 720, memoryMb: null, javaPath: null, jvmArgs: [], syncSettings: true, icon: null, edition: 'java' as const },
    { name: 'Avventura con Luca', version: '1.21.130', loader: null, created: now() - 86400 * 9, lastPlayed: now() - 86400 * 2, playtimeSecs: 43200, memoryMb: null, javaPath: null, jvmArgs: [], syncSettings: true, icon: null, edition: 'bedrock' as const },
    { name: 'Snapshot', version: '26.4-snapshot-8', loader: null, created: now() - 3600, lastPlayed: null, playtimeSecs: 0, memoryMb: null, javaPath: null, jvmArgs: [], syncSettings: true, icon: null, edition: 'java' as const },
  ],
  accounts: [
    { name: 'TroppoFranco', uuid: '069a79f4-44e9-4726-a5be-fca90e38aaf5', kind: 'microsoft', default: true, skin: null },
    { name: 'Dev', uuid: '380df991-f603-344c-a090-369bad2a924a', kind: 'offline', default: false, skin: null },
  ],
  settings: { defaultMemoryMb: 4096, javaPath: '', msaClientId: '', accent: '#6e5ae6', minimizeOnLaunch: true, showConsole: false, theme: 'dark', language: 'it', setupDone: false, sync: { enabled: false, keybinds: true, video: true, audio: true, controls: true },
    appearance: { background: 'solid', effect: 'mica', blur: 16, dim: 40, panelOpacity: 70, uiScale: 100, radius: 'round', buttons: 'standard', animations: 'full', sidebarSide: 'left', sidebarLabels: false, sidebarInstances: 5, showNews: true, wideContent: false, gradientFrom: '#6e5ae6', gradientTo: '#0f1115', gradientDirection: 'down', home: null },
    downloadConcurrency: 16, defaultJvmArgs: '', gameWidth: null, gameHeight: null, gameFullscreen: false },
  mods: [
    { file: 'sodium-fabric-0.9.2+mc26.3.jar', name: 'sodium-fabric-0.9.2+mc26.3', enabled: true, sizeBytes: 1_200_000, projectId: 'AANobbMI', slug: 'sodium', title: 'Sodium', iconUrl: null, versionNumber: 'mc26.3-0.9.2-fabric', update: 'mc26.3-0.9.3-fabric', source: 'modrinth', website: null },
    { file: 'fabric-api-0.166.2+26.3.jar', name: 'fabric-api-0.166.2+26.3', enabled: true, sizeBytes: 2_400_000, projectId: 'P7dR8mSH', slug: 'fabric-api', title: 'Fabric API', iconUrl: null, versionNumber: '0.166.2+26.3', update: null, source: 'modrinth', website: null },
    { file: 'modmenu-21.0.1.jar.disabled', name: 'modmenu-21.0.1', enabled: false, sizeBytes: 800_000, projectId: 'mOgUt4GM', slug: 'modmenu', title: 'Mod Menu', iconUrl: null, versionNumber: '21.0.1', update: null, source: 'modrinth', website: null },
    { file: 'villagesbeyond-1.0.0.jar', name: 'villagesbeyond-1.0.0', enabled: true, sizeBytes: 300_000, projectId: 'cf:1782682', slug: null, title: 'Villages Beyond', iconUrl: null, versionNumber: 'Villages Beyond 1.0.0', update: 'Villages Beyond 1.1.0', source: 'curseforge', website: 'https://www.curseforge.com/minecraft/mc-mods/villages-beyond' },
    { file: 'mia-mod-test.jar', name: 'mia-mod-test', enabled: true, sizeBytes: 40_000, projectId: null, slug: null, title: null, iconUrl: null, versionNumber: null, update: null, source: null, website: null },
  ],
};

const PLUGIN_HITS: SearchHit[] = [
  { id: 'hXiIvTyT', slug: 'essentialsx', title: 'EssentialsX', description: 'The essential plugin suite for Minecraft servers.', author: 'EssentialsX', iconUrl: null, image: null, color: 0xe0592a, downloads: 2_400_000, updated: '2026-10-02T00:00:00Z', categories: ['utility'] },
  { id: 'Vebnzrzj', slug: 'luckperms', title: 'LuckPerms', description: 'A permissions plugin for Minecraft servers.', author: 'Luck', iconUrl: null, image: null, color: 0x3fa7e0, downloads: 1_900_000, updated: '2026-09-21T00:00:00Z', categories: ['management'] },
  { id: 'fALzjamp', slug: 'chunky', title: 'Chunky', description: 'Pre-generates chunks, quickly and efficiently.', author: 'pop4959', iconUrl: null, image: null, color: 0x56b04a, downloads: 1_200_000, updated: '2026-11-01T00:00:00Z', categories: ['optimization'] },
  { id: 'swbUV1cr', slug: 'bluemap', title: 'BlueMap', description: 'A 3D map of your world, in the browser.', author: 'Blue', iconUrl: null, image: null, color: 0x2f6fdf, downloads: 900_000, updated: '2026-10-15T00:00:00Z', categories: ['utility'] },
];

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

/** Skin finta per il browser: blocchi di colore al posto giusto. */
function fakeSkin(color: string): string {
  const c = document.createElement('canvas');
  c.width = c.height = 64;
  const x = c.getContext('2d')!;
  const fill = (col: string, ...r: [number, number, number, number][]) => { x.fillStyle = col; r.forEach((a) => x.fillRect(...a)); };
  fill('#c99b75', [0, 8, 32, 8], [8, 0, 16, 8]);
  fill('#3b2a1e', [8, 0, 8, 8], [8, 8, 8, 2], [0, 8, 32, 2]);
  fill(color, [16, 20, 24, 12], [40, 20, 16, 12], [32, 52, 16, 12], [20, 16, 16, 4]);
  fill('#2c3e66', [0, 20, 16, 12], [16, 52, 16, 12], [4, 16, 8, 4]);
  return c.toDataURL('image/png');
}
const skins: { id: string; name: string; slim: boolean; image: string }[] = [];
let selectedSkin: string | null = null;
let backgroundImage: string | null = null;

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

// ---------- Server ospitati ----------
// Due server come nel mockup, uno acceso: la console cresce da sola, le
// statistiche si campionano ogni 2 s e i comandi dei giocatori cambiano
// gli elenchi.

interface MockServer {
  info: ServerInfo;
  props: ServerProperties;
  players: ServerPlayers;
  console: string[];
  points: StatsPoint[];
  files: Record<string, string | null>;
  plugins: ServerPlugin[];
}

const clock = () => new Date().toTimeString().slice(0, 8);
const uuid = () => crypto.randomUUID();
const CHATTER = [
  (s: MockServer) => `[${clock()} INFO]: There are ${s.players.online.length} of a max of ${s.props.maxPlayers} players online: ${s.players.online.join(', ')}`,
  () => `[${clock()} INFO]: [Server] §aBackup completato§r in §l2.1s`,
  () => `[${clock()} INFO]: <Marco> ciao a tutti!`,
  () => `[${clock()} WARN]: Can't keep up! Is the server overloaded? Running 2140ms behind, skipping 42 tick(s)`,
  () => `[${clock()} INFO]: <Giulia> qualcuno ha visto il villaggio a nord?`,
  () => `[${clock()} INFO]: Saving the game (this may take a moment!)`,
  () => `[${clock()} INFO]: Saved the game`,
  () => `[${clock()} ERROR]: Could not pass event PlayerMoveEvent to EssentialsX v2.21.0`,
];

function defaultProps(port: number): ServerProperties {
  return { maxPlayers: 20, onlineMode: false, motd: 'Benvenuti nel server degli amici!', difficulty: 'normal', gamemode: 'survival', pvp: true, whitelist: true, viewDistance: 10, simulationDistance: 10, hardcore: false, allowFlight: false, spawnProtection: 16, seed: '', port };
}

function serverFiles(kind: 'vanilla' | 'paper'): Record<string, string | null> {
  const files: Record<string, string | null> = {
    'server.properties': '#Minecraft server properties\nmotd=Benvenuti nel server degli amici!\nmax-players=20\nonline-mode=false\ndifficulty=normal\npvp=true\nwhite-list=true\nview-distance=10\n',
    'eula.txt': '#By changing the setting below to TRUE you are indicating your agreement to our EULA (https://aka.ms/MinecraftEULA).\neula=true\n',
    'ops.json': '[\n  {\n    "uuid": "8667ba71-b85a-4004-af54-457a9734eed7",\n    "name": "Marco",\n    "level": 4\n  }\n]\n',
    'banned-players.json': '[]\n',
    'server.jar': null,
    'logs/latest.log': '[21:14:02 INFO]: Starting minecraft server version 1.21.4\n[21:14:03 INFO]: Done (8.412s)! For help, type "help"\n',
    'world/level.dat': null,
    'world/region/r.0.0.mca': null,
  };
  if (kind === 'paper') {
    files['plugins/EssentialsX.jar'] = null;
    files['plugins/EssentialsX/config.yml'] = '# Messaggio di benvenuto\nnewbies:\n  announce-format: \'&dBenvenuto {DISPLAYNAME}!\'\n  kit: tools\nteleport-cooldown: 5\n';
    files['plugins/EssentialsX/kits.yml'] = 'kits:\n  tools:\n    delay: 10\n    items:\n      - stone_pickaxe 1\n      - stone_axe 1\n';
    files['bukkit.yml'] = 'settings:\n  allow-end: true\n  warn-on-overload: true\n';
  }
  return files;
}

function statsPoint(s: MockServer, t: number): StatsPoint {
  const prev = s.points.at(-1);
  const wobble = (v: number, d: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v + (Math.random() - 0.5) * d));
  return {
    t,
    cpu: Math.round(wobble(prev?.cpu ?? 30, 14, 4, 95)),
    memMb: Math.round(wobble(prev?.memMb ?? 2200, 120, 900, s.info.memoryMb - 200)),
    players: s.players.online.length,
    latencyMs: s.players.online.length ? Math.round(wobble(prev?.latencyMs ?? 4, 4, 1, 40)) : null,
  };
}

const servers: MockServer[] = [
  {
    info: { name: 'Server degli amici', kind: 'paper', version: '1.21.4', build: 232, memoryMb: 4096, port: 25565, keepRunning: true, autostart: false, created: now() - 86400 * 20, running: true, startedAt: now() - 8040, players: 2, maxPlayers: 20, address: '192.168.1.20:25565', icon: null, publicAddress: null, loaderVersion: null, modpack: null, schedule: { restart_at: null, backup_hours: 6, keep_backups: 10 } },
    props: defaultProps(25565),
    players: {
      online: ['Marco', 'Giulia'],
      ops: [{ name: 'Marco', uuid: '8667ba71-b85a-4004-af54-457a9734eed7' }],
      whitelist: [{ name: 'Marco', uuid: '8667ba71-b85a-4004-af54-457a9734eed7' }, { name: 'Giulia', uuid: '5a1e3f2c-8f0b-4b5e-9c1d-2a7e8b9c0d1e' }, { name: 'Luca', uuid: '1b2c3d4e-5f60-4a7b-8c9d-0e1f2a3b4c5d' }],
      banned: [{ name: 'Griefer99', uuid: '9f8e7d6c-5b4a-4392-8170-6f5e4d3c2b1a', reason: 'Ha distrutto la base di Giulia' }],
    },
    console: [
      '[21:14:01 INFO]: Starting minecraft server version 1.21.4',
      '[21:14:01 INFO]: Loading properties',
      '[21:14:02 INFO]: §6[EssentialsX]§r Enabling EssentialsX v2.21.0',
      '[21:14:02 INFO]: Preparing spawn area: 100%',
      '[21:14:03 INFO]: Done (8.412s)! For help, type "help"',
      '[21:20:41 INFO]: Marco joined the game',
      '[21:22:10 INFO]: <Marco> ciao a tutti!',
      '[21:25:57 INFO]: Giulia joined the game',
      '[21:31:18 WARN]: Can\'t keep up! Is the server overloaded? Running 2140ms behind, skipping 42 tick(s)',
    ],
    points: [],
    files: serverFiles('paper'),
    plugins: [
      { file: 'EssentialsX-2.21.0.jar', enabled: true, sizeBytes: 3_100_000, projectId: 'hXiIvTyT', title: 'EssentialsX', iconUrl: null, versionNumber: '2.21.0' },
      { file: 'LuckPerms-Bukkit-5.4.150.jar', enabled: true, sizeBytes: 2_000_000, projectId: 'Vebnzrzj', title: 'LuckPerms', iconUrl: null, versionNumber: '5.4.150' },
      { file: 'vecchio-plugin.jar.disabled', enabled: false, sizeBytes: 120_000, projectId: null, title: null, iconUrl: null, versionNumber: null },
    ],
  },
  {
    info: { name: 'Survival vanilla', kind: 'vanilla', version: '1.21.4', build: null, memoryMb: 3072, port: 25566, keepRunning: false, autostart: false, created: now() - 86400 * 4, running: false, startedAt: null, players: 0, maxPlayers: 10, address: '192.168.1.20:25566', icon: null, publicAddress: null, loaderVersion: null, modpack: null, schedule: { restart_at: null, backup_hours: 6, keep_backups: 10 } },
    props: { ...defaultProps(25566), maxPlayers: 10, motd: 'A Minecraft Server', onlineMode: true, whitelist: false },
    players: { online: [], ops: [], whitelist: [], banned: [] },
    console: [],
    points: [],
    files: serverFiles('vanilla'),
    plugins: [],
  },
];
// Cinque minuti di storia per il server acceso.
{
  const s = servers[0];
  const t0 = now() - 300;
  for (let i = 0; i < 150; i++) s.points.push(statsPoint(s, t0 + i * 2));
}

function findServer(name: string): MockServer {
  const s = servers.find((x) => x.info.name === name);
  if (!s) throw `Server “${name}” non trovato.`;
  return s;
}

function say(s: MockServer, line: string) {
  s.console.push(line);
}

// Il mondo va avanti: righe nuove e campioni ogni 2 s per i server accesi.
setInterval(() => {
  for (const s of servers) {
    if (!s.info.running) continue;
    s.points.push(statsPoint(s, now()));
    if (s.points.length > 150) s.points.shift();
    if (Math.random() < 0.45) say(s, CHATTER[Math.floor(Math.random() * CHATTER.length)](s));
    s.info.players = s.players.online.length;
    s.info.maxPlayers = s.props.maxPlayers;
  }
}, 2000);

function startServer(s: MockServer) {
  s.info.running = true;
  s.info.startedAt = now();
  s.points = [];
  [
    `Starting minecraft server version ${s.info.version}`,
    'Loading properties',
    `Starting Minecraft server on *:${s.info.port}`,
    'Preparing level "world"',
    'Preparing spawn area: 100%',
    'Done (6.204s)! For help, type "help"',
  ].forEach((l, i) => setTimeout(() => say(s, `[${clock()} INFO]: ${l}`), 250 * i));
}

function stopServer(s: MockServer) {
  say(s, `[${clock()} INFO]: Stopping the server`);
  say(s, `[${clock()} INFO]: Saving worlds`);
  s.info.running = false;
  s.info.startedAt = null;
  s.players.online = [];
  s.info.players = 0;
}

function entry(name: string) {
  return { name, uuid: uuid() };
}

/** Esegue un comando della console sui dati finti, come farebbe il server. */
function runCommand(s: MockServer, line: string) {
  if (!s.info.running) throw 'Il server è spento.';
  const [cmd, ...rest] = line.trim().split(/\s+/);
  const arg = rest.join(' ');
  const p = s.players;
  const out = (text: string) => say(s, `[${clock()} INFO]: ${text}`);
  switch (cmd) {
    case 'op': if (!p.ops.some((o) => o.name === arg)) p.ops.push(entry(arg)); out(`Made ${arg} a server operator`); break;
    case 'deop': p.ops = p.ops.filter((o) => o.name !== arg); out(`Made ${arg} no longer a server operator`); break;
    case 'kick': p.online = p.online.filter((n) => n !== arg); out(`Kicked ${arg}: Kicked by an operator`); break;
    case 'ban':
      p.online = p.online.filter((n) => n !== rest[0]);
      if (!p.banned.some((b) => b.name === rest[0])) p.banned.push({ ...entry(rest[0]), reason: rest.slice(1).join(' ') || 'Banned by an operator.' });
      out(`Banned ${rest[0]}: Banned by an operator.`);
      break;
    case 'pardon': p.banned = p.banned.filter((b) => b.name !== arg); out(`Unbanned ${arg}`); break;
    case 'whitelist':
      if (rest[0] === 'add' && rest[1]) { if (!p.whitelist.some((w) => w.name === rest[1])) p.whitelist.push(entry(rest[1])); out(`Added ${rest[1]} to the whitelist`); }
      else if (rest[0] === 'remove' && rest[1]) { p.whitelist = p.whitelist.filter((w) => w.name !== rest[1]); out(`Removed ${rest[1]} from the whitelist`); }
      else out('§cUnknown or incomplete command, see below for error');
      break;
    case 'say': out(`[Server] ${arg}`); break;
    case 'list': out(`There are ${p.online.length} of a max of ${s.props.maxPlayers} players online: ${p.online.join(', ')}`); break;
    case 'stop': stopServer(s); break;
    default: say(s, `[${clock()} WARN]: §cUnknown or incomplete command, see below for error`);
  }
}

function listDir(s: MockServer, path: string) {
  const prefix = path ? `${path}/` : '';
  const out = new Map<string, { name: string; dir: boolean; size: number; modified: number }>();
  for (const [file, content] of Object.entries(s.files)) {
    if (!file.startsWith(prefix)) continue;
    const rest = file.slice(prefix.length);
    const [head, ...tail] = rest.split('/');
    if (tail.length) {
      const d = out.get(head) ?? { name: head, dir: true, size: 0, modified: now() - 3600 };
      out.set(head, d);
    } else {
      out.set(head, { name: head, dir: false, size: content === null ? 48_000_000 : new Blob([content]).size, modified: now() - 7200 });
    }
  }
  if (path && !out.size) throw 'Cartella non trovata.';
  return [...out.values()];
}

function simulateServerCreate(name: string) {
  const stages: ['download' | 'java' | 'setup', number][] = [['download', 12], ['java', 8], ['setup', 3]];
  return new Promise<void>((resolve) => {
    let i = 0;
    let done = 0;
    const timer = setInterval(() => {
      const [stage, total] = stages[i];
      done += 1;
      emit('server-progress', { name, stage, done, total });
      if (done >= total) {
        i += 1;
        done = 0;
        if (i >= stages.length) { clearInterval(timer); resolve(); }
      }
    }, 140);
  });
}

/** Backup simulati per chiave (`s:<server>` o `w:<istanza>/<mondo>`). */
const backups = new Map<string, BackupInfo[]>();
const bedrockPacks = new Map<string, Record<BedrockKind, BedrockPack[]>>();
function mockPackIcon(color: string): string | null {
  if (typeof document === 'undefined') return null;
  const c = document.createElement('canvas');
  c.width = c.height = 16;
  const g = c.getContext('2d');
  if (!g) return null;
  g.fillStyle = color;
  g.fillRect(0, 0, 16, 16);
  g.fillStyle = 'rgba(255,255,255,.25)';
  for (let i = 0; i < 16; i += 4) g.fillRect(i, (i * 7) % 16, 4, 4);
  return c.toDataURL();
}
function mockBedrockPacks(name: string): Record<BedrockKind, BedrockPack[]> {
  let packs = bedrockPacks.get(name);
  if (!packs) {
    const pack = (kind: BedrockKind, scope: 'shared' | 'user', folder: string, title: string, description: string, version: string | null, color: string, age: number): BedrockPack =>
      ({ kind, scope, folder, name: title, description, version, uuid: folder, icon: mockPackIcon(color), modified: now() - age });
    packs = {
      resource: [pack('resource', 'shared', 'Texture chiare', 'Texture chiare', 'Blocchi più luminosi e puliti', '1.3.0', '#3b9bb0', 86400 * 4)],
      behavior: [
        pack('behavior', 'shared', 'Mob intelligenti', 'Mob più intelligenti', 'Comportamenti nuovi per zombie e scheletri', '1.4.0', '#b0563b', 86400 * 6),
        pack('behavior', 'shared', 'Mobili moderni', 'Mobili moderni', 'Sedie, tavoli e lampade', '2.0.3', '#3b7bb0', 86400 * 3),
      ],
      skin: [],
      world: [
        pack('world', 'user', 'Ab3dEfGhIk0', 'Isola dei pirati', '', null, '#4f8a3b', 86400 * 2),
        pack('world', 'user', 'Zx9yWvUtSr1', 'Creativa', '', null, '#8a7a3b', 86400 * 12),
      ],
      template: [],
    };
    bedrockPacks.set(name, packs);
  }
  return packs;
}

function mockBackups(key: string, count = 3): BackupInfo[] {
  if (!backups.has(key)) {
    backups.set(key, Array.from({ length: count }, (_, i) => ({ file: `${now() - 21600 * (i + 1)}-auto.zip`, created: now() - 21600 * (i + 1), sizeBytes: 140_000_000 - i * 1_000_000, auto: i < 2 })));
  }
  return backups.get(key)!;
}

/** playit.gg collegato (simulato). */
let playitConnected = false;

function serverCommands(): Record<string, (args: Record<string, any>) => unknown> {
  return {
    server_list: () => servers.map((s) => structuredClone(s.info)),
    server_versions: ({ kind }) => (kind === 'paper' ? ['1.21.4', '1.21.3', '1.21.1', '1.20.6', '1.20.4'] : ['1.21.4', '1.21.3', '1.21.2', '1.21.1', '1.21', '1.20.6', '1.20.4']),
    server_create: async ({ input }) => {
      if (servers.some((s) => s.info.name === input.name)) throw 'Esiste già un server con questo nome.';
      if (!input.eula) throw 'Per creare un server bisogna accettare l’EULA di Minecraft.';
      await simulateServerCreate(input.name);
      const s: MockServer = {
        info: { name: input.name, kind: input.kind, version: input.version, build: input.kind === 'paper' ? 230 : null, memoryMb: input.memoryMb, port: input.port, keepRunning: input.keepRunning, autostart: input.autostart, created: now(), running: false, startedAt: null, players: 0, maxPlayers: 20, address: `192.168.1.20:${input.port}`, icon: null, publicAddress: null, loaderVersion: input.loaderVersion ?? null, modpack: null, schedule: { restart_at: null, backup_hours: null, keep_backups: 10 } },
        props: { ...defaultProps(input.port), motd: 'A Minecraft Server' },
        players: { online: [], ops: [], whitelist: [], banned: [] },
        console: [],
        points: [],
        files: serverFiles(input.kind),
        plugins: [],
      };
      servers.push(s);
      return structuredClone(s.info);
    },
    server_crash: ({ name }) => (findServer(name).info.running ? null : { title: 'Porta già in uso', cause: '[21:31:18 WARN]: **** FAILED TO BIND TO PORT!', hint: 'Un altro programma (o un altro server) usa questa porta: chiudilo o cambia la porta nelle impostazioni del server.', file: null }),
    server_play: ({ name }) => ({ instance: 'Sopravvivenza', join: `localhost:${findServer(name).info.port}` }),
    server_pick_modpack: () => ({ path: 'C:\\Users\\utente\\Downloads\\Avventura.mrpack', name: 'Avventura tra amici', game: '1.21.1', loader: 'Fabric 0.16.10' }),
    server_create_modpack: async ({ input }) => {
      if (servers.some((s) => s.info.name === input.name)) throw 'Esiste già un server con questo nome.';
      await simulateServerCreate(input.name);
      const name = input.project ? 'Cobblemon Official Modpack' : 'Avventura tra amici';
      const s: MockServer = {
        info: { name: input.name, kind: 'fabric', version: '1.21.1', build: null, memoryMb: input.memoryMb, port: input.port, keepRunning: input.keepRunning, autostart: input.autostart, created: now(), running: false, startedAt: null, players: 0, maxPlayers: 20, address: `192.168.1.20:${input.port}`, icon: null, publicAddress: null, loaderVersion: '0.16.10', modpack: { name, project: input.project }, schedule: { restart_at: null, backup_hours: null, keep_backups: 10 } },
        props: { ...defaultProps(input.port), motd: 'A Minecraft Server' },
        players: { online: [], ops: [], whitelist: [], banned: [] },
        console: [],
        points: [],
        files: serverFiles('vanilla'),
        plugins: [],
      };
      servers.push(s);
      return structuredClone(s.info);
    },
    server_delete: ({ name }) => {
      const s = findServer(name);
      if (s.info.running) throw 'Ferma il server prima di eliminarlo.';
      servers.splice(servers.indexOf(s), 1);
    },
    server_start: ({ name }) => startServer(findServer(name)),
    server_stop: ({ name }) => stopServer(findServer(name)),
    server_restart: async ({ name }) => {
      const s = findServer(name);
      stopServer(s);
      await new Promise((r) => setTimeout(r, 800));
      startServer(s);
    },
    server_command: ({ name, line }) => runCommand(findServer(name), line),
    server_console: ({ name, since }) => {
      const s = findServer(name);
      return { lines: s.console.slice(since), next: s.console.length };
    },
    server_stats: ({ name }) => {
      const s = findServer(name);
      return { points: structuredClone(s.points), memMaxMb: s.info.memoryMb, online: [...s.players.online] };
    },
    server_update: ({ name, update }) => {
      const s = findServer(name);
      Object.assign(s.info, update);
      return structuredClone(s.info);
    },
    server_set_schedule: ({ name, schedule }) => {
      const s = findServer(name);
      s.info.schedule = schedule;
      return structuredClone(s.info);
    },
    server_set_icon: ({ name, png }) => {
      const s = findServer(name);
      s.info.icon = png;
      return structuredClone(s.info);
    },
    // Backup simulati: un elenco per server e per mondo di istanza.
    server_backups: ({ name }) => structuredClone(mockBackups(`s:${name}`)),
    server_all_backups: () => servers
      .flatMap((s) => mockBackups(`s:${s.info.name}`).map((b) => ({ ...b, server: s.info.name })))
      .sort((x, y) => y.created - x.created),
    server_pick_world: () => 'C:\\Users\\utente\\Downloads\\Mondo di prova.zip',
    server_backup: async ({ name }) => {
      await new Promise((r) => setTimeout(r, 1200));
      const b = { file: `${now()}-manual.zip`, created: now(), sizeBytes: 148_000_000, auto: false };
      mockBackups(`s:${name}`).unshift(b);
      return b;
    },
    server_backup_restore: async ({ name }) => {
      if (findServer(name).info.running) throw 'Ferma il server prima di ripristinare un backup.';
      await new Promise((r) => setTimeout(r, 900));
      return ['world'];
    },
    server_backup_delete: ({ name, file }) => {
      const list = mockBackups(`s:${name}`);
      list.splice(list.findIndex((b) => b.file === file), 1);
    },
    world_backups: ({ name, world }) => structuredClone(mockBackups(`w:${name}/${world}`, 1)),
    world_backup: async ({ name, world }) => {
      await new Promise((r) => setTimeout(r, 700));
      const b = { file: `${now()}-manual.zip`, created: now(), sizeBytes: 12_000_000, auto: false };
      mockBackups(`w:${name}/${world}`, 1).unshift(b);
      return b;
    },
    world_backup_restore: () => new Promise((r) => setTimeout(r, 600)),
    world_backup_delete: ({ name, world, file }) => {
      const list = mockBackups(`w:${name}/${world}`, 1);
      list.splice(list.findIndex((b) => b.file === file), 1);
    },
    // playit.gg: l'approvazione "arriva" dopo qualche secondo.
    playit_status: () => ({ connected: playitConnected }),
    playit_connect: async () => {
      emit('playit-claim', 'WaitingForUserVisit');
      await new Promise((r) => setTimeout(r, 1500));
      emit('playit-claim', 'WaitingForUser');
      await new Promise((r) => setTimeout(r, 1500));
      emit('playit-claim', 'UserAccepted');
      playitConnected = true;
    },
    playit_disconnect: () => {
      playitConnected = false;
      for (const s of servers) s.info.publicAddress = null;
    },
    server_playit_enable: async ({ name }) => {
      const s = findServer(name);
      if (!playitConnected) throw 'playit.gg non collegato';
      for (const stage of ['agent', 'tunnel', 'address']) {
        emit('playit-progress', { name, stage });
        await new Promise((r) => setTimeout(r, 900));
      }
      s.info.publicAddress = 'gentle-sound.tun.ply.gg';
      return structuredClone(s.info);
    },
    server_playit_disable: ({ name }) => {
      const s = findServer(name);
      s.info.publicAddress = null;
      return structuredClone(s.info);
    },
    server_players: ({ name }) => structuredClone(findServer(name).players),
    server_properties: ({ name }) => structuredClone(findServer(name).props),
    server_save_properties: ({ name, properties }) => {
      const s = findServer(name);
      s.props = structuredClone(properties);
      s.info.port = properties.port;
      s.info.address = `192.168.1.20:${properties.port}`;
    },
    server_files: ({ name, path }) => listDir(findServer(name), path),
    server_read_file: ({ name, path }) => {
      const content = findServer(name).files[path];
      if (content === undefined) throw 'File non trovato.';
      if (content === null) throw 'Il file è binario o troppo grande per l’editor.';
      return content;
    },
    server_write_file: ({ name, path, content }) => {
      findServer(name).files[path] = content;
    },
    server_delete_file: ({ name, path }) => {
      const s = findServer(name);
      for (const f of Object.keys(s.files)) if (f === path || f.startsWith(`${path}/`)) delete s.files[f];
    },
    server_plugins: ({ name }) => structuredClone(findServer(name).plugins),
    server_plugin_install: async ({ name, project }) => {
      const s = findServer(name);
      const h = PLUGIN_HITS.find((x) => x.id === project || x.slug === project);
      if (!h) throw 'Plugin non trovato.';
      if (s.plugins.some((p) => p.projectId === h.id)) throw `${h.title} è già installato.`;
      await new Promise((r) => setTimeout(r, 900));
      const file = `${h.slug}.jar`;
      s.plugins.push({ file, enabled: true, sizeBytes: 1_400_000, projectId: h.id, title: h.title, iconUrl: null, versionNumber: '1.0.0' });
      return [file];
    },
    server_plugin_set_enabled: ({ name, file, enabled }) => {
      const p = findServer(name).plugins.find((x) => x.file === file);
      if (!p) throw 'Plugin non trovato.';
      p.enabled = enabled;
      p.file = enabled ? p.file.replace(/\.disabled$/, '') : `${p.file}.disabled`;
      return p.file;
    },
    server_plugin_delete: ({ name, file }) => {
      const s = findServer(name);
      s.plugins = s.plugins.filter((p) => p.file !== file);
    },
  };
}

const commands: Record<string, (args: Record<string, any>) => unknown> = {
  // Negli screenshot delle release (`?version=0.1.22`) si vede la versione vera.
  app_info: () => {
    const version = typeof location === 'undefined' ? null : new URLSearchParams(location.search).get('version');
    return { version: version ?? '0.1.0-dev', dev: !version, dataDir: 'C:\\Users\\utente\\AppData\\Roaming\\HyperLauncher', curseforge: true, windowEffect: false };
  },
  list_instances: () => structuredClone(state.instances),
  create_instance: ({ input }) => {
    if (input.edition === 'bedrock') {
      const inst = { name: input.name || 'Bedrock', version: '1.21.130', loader: null, created: now(), lastPlayed: null, playtimeSecs: 0, memoryMb: null, javaPath: null, jvmArgs: [], syncSettings: true, icon: null, edition: 'bedrock' as const };
      state.instances.push(inst);
      return structuredClone(inst);
    }
    const loader = input.loader === 'fabric' ? { kind: 'fabric' as const, name: 'Fabric', version: '0.20.1' } : null;
    const inst = { name: input.name || `${loader ? 'Fabric' : 'Minecraft'} ${input.version}`, version: input.version, loader, created: now(), lastPlayed: null, playtimeSecs: 0, memoryMb: null, javaPath: null, jvmArgs: [], syncSettings: true, icon: null, edition: 'java' as const };
    state.instances.push(inst);
    return structuredClone(inst);
  },
  // Bedrock simulato: installato, con una installazione attiva e qualche pacchetto.
  bedrock_status: () => ({ installed: true, version: '1.21.130', active: 'Bedrock', profile: true, running: false }),
  bedrock_packs: ({ name, kind }) => structuredClone(mockBedrockPacks(name)[kind as BedrockKind] ?? []),
  bedrock_import: async ({ name }) => {
    await new Promise((r) => setTimeout(r, 700));
    const pack: BedrockPack = { kind: 'behavior', scope: 'shared', folder: 'Piu minerali', name: 'Più minerali', description: 'Rame, argento e piombo nelle grotte', version: '0.9.1', uuid: 'c3', icon: mockPackIcon('#7a3bb0'), modified: now() };
    mockBedrockPacks(name).behavior.push(pack);
    return [{ kind: 'behavior', name: pack.name, replaced: false }];
  },
  bedrock_delete: ({ name, kind, folder }) => {
    const packs = mockBedrockPacks(name);
    packs[kind as BedrockKind] = packs[kind as BedrockKind].filter((p) => p.folder !== folder);
  },
  bedrock_open_folder: () => undefined,
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
  // La skin scelta vale per l'account attivo (nel mock ce n'è una sola).
  list_accounts: () => state.accounts.map((a) => ({ ...a, skin: a.default ? (skins.find((s) => s.id === selectedSkin)?.image ?? null) : null })),
  add_offline_account: ({ name }) => {
    state.accounts.forEach((a) => (a.default = false));
    state.accounts.push({ name, uuid: crypto.randomUUID(), kind: 'offline', default: true, skin: null });
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
  save_settings: ({ settings }) => { state.settings = settings; return settings.appearance.background === 'transparent'; },
  content_search: ({ input }) => {
    const q = String(input.text).toLowerCase();
    const hits = (input.kind === 'plugin' ? PLUGIN_HITS : HITS).filter((h) => !q || h.title.toLowerCase().includes(q) || h.description.toLowerCase().includes(q));
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
    const inst = { name: 'Fabulously Optimized', version: '26.3', loader: { kind: 'fabric' as const, name: 'Fabric', version: '0.20.1' }, created: now(), lastPlayed: null, playtimeSecs: 0, memoryMb: null, javaPath: null, jvmArgs: [], syncSettings: true, icon: null, edition: 'java' as const };
    state.instances.push(inst);
    return structuredClone(inst);
  },
  pack_import: () => null,
  cf_featured: ({ kind }) => {
    const p = (id: number, name: string, summary: string, k: string) => ({ id, name, summary, author: 'Autore', iconUrl: null, downloads: 1_000_000 + id, kind: k, website: `https://www.curseforge.com/minecraft/mc-mods/${id}`, updated: '2026-11-10T00:00:00Z' });
    const all = [p(348521, 'Cloth Config API', 'Configuration library for mods.', 'mod'), p(1782682, 'Villages Beyond', 'New villages and villager expansion.', 'mod'), p(396246, 'Fabulously Optimized', 'Optimization modpack.', 'modpack'), p(1360926, 'Cobblemon SMP', 'Pokémon adventure.', 'modpack')].filter((x) => x.kind === kind);
    return { featured: all.slice(0, 1), popular: all, recent: all.slice(1) };
  },
  cf_project: ({ id }) => ({ id, name: `Progetto ${id}`, summary: 'Progetto trovato per ID.', author: 'Autore', iconUrl: null, downloads: 1234, kind: 'mod', website: `https://www.curseforge.com/projects/${id}`, updated: '2026-11-10T00:00:00Z' }),
  cf_install: async ({ instance }) => { await simulateDownload(instance, 2); return 2; },
  cf_pack_install: async () => {
    await simulateDownload('', 4);
    const inst = { name: 'Pack CurseForge', version: '26.2', loader: { kind: 'fabric' as const, name: 'Fabric', version: '0.20.1' }, created: now(), lastPlayed: null, playtimeSecs: 0, memoryMb: null, javaPath: null, jvmArgs: [], syncSettings: true, icon: null, edition: 'java' as const };
    state.instances.push(inst);
    return { instance: structuredClone(inst), blocked: [{ name: 'Entity Culling', fileName: 'entityculling-fabric-1.11.3.jar', url: 'https://www.curseforge.com/minecraft/mc-mods/entityculling' }] };
  },
  pack_export: ({ instance }) => ({ path: `C:\\Users\\utente\\Documents\\${instance}.mrpack`, linked: 3, bundled: 1 }),
  // Blocchi simulati: texture 16×16 a pixel con il colore del blocco.
  block_icons: () => ([['grass_block_side', '#5d9c3a'], ['dirt', '#8a5b3a'], ['stone', '#7d7d7d'], ['cobblestone', '#6b6b6b'], ['oak_planks', '#b08a52'], ['sand', '#dbcf9a'], ['diamond_ore', '#4fd1c5'], ['gold_block', '#f2c94c'], ['obsidian', '#2a1f3d'], ['netherrack', '#7a2a2a'], ['glowstone', '#e8c66a'], ['tnt_side', '#c0392b']] as [string, string][]).map(([name, color]) => {
    const c = document.createElement('canvas');
    c.width = c.height = 16;
    const x = c.getContext('2d')!;
    for (let i = 0; i < 256; i++) {
      x.fillStyle = color;
      x.globalAlpha = 0.6 + ((i * 7919) % 41) / 100;
      x.fillRect(i % 16, Math.floor(i / 16), 1, 1);
    }
    return { name, image: c.toDataURL('image/png') };
  }),
  set_instance_icon: ({ name, png }) => {
    const inst = state.instances.find((i) => i.name === name)!;
    inst.icon = png;
    return structuredClone(inst);
  },
  list_skins: () => ({ skins: structuredClone(skins), selected: selectedSkin, offline: true }),
  default_skins: () => [
    { id: 'default:steve', name: 'Steve', slim: false, image: fakeSkin('#3fa7a0') },
    { id: 'default:alex', name: 'Alex', slim: true, image: fakeSkin('#5f9e3a') },
    { id: 'default:kai', name: 'Kai', slim: false, image: fakeSkin('#8a3b8f') },
  ],
  add_skin: ({ name, png, slim }) => {
    const id = `skin-${skins.length}`;
    skins.push({ id, name, slim, image: png });
    selectedSkin = id;
    return { skins: structuredClone(skins), selected: selectedSkin, offline: true };
  },
  remove_skin: ({ id }) => {
    skins.splice(skins.findIndex((s) => s.id === id), 1);
    if (selectedSkin === id) selectedSkin = null;
    return { skins: structuredClone(skins), selected: selectedSkin, offline: true };
  },
  select_skin: ({ id }) => {
    selectedSkin = id;
    return { skins: structuredClone(skins), selected: selectedSkin, offline: true };
  },
  list_servers: () => [
    { name: 'Server degli amici', address: 'mc.example.org', icon: null, instances: ['Sopravvivenza'] },
    { name: 'Hypixel', address: 'mc.hypixel.net', icon: null, instances: ['Minecraft 26.3'] },
    { name: 'Vecchio server', address: '10.0.0.5', icon: null, instances: ['Sopravvivenza'] },
  ],
  ping_server: async ({ address }) => {
    await new Promise((r) => setTimeout(r, 600));
    if (address === '10.0.0.5') throw 'Server non raggiungibile.';
    return { online: address.includes('hypixel') ? 31245 : 4, max: address.includes('hypixel') ? 200000 : 20, version: address.includes('hypixel') ? 'Requires MC 1.8 / 1.21' : 'Paper 26.3', motd: '', latencyMs: 42 };
  },
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
  // Nel browser: aggiungi ?update all'indirizzo per vedere la finestra di aggiornamento.
  background: () => backgroundImage,
  set_background: ({ image }) => { backgroundImage = image ?? null; },
  storage_usage: () => ({ instances: 3_400_000_000, libraries: 420_000_000, assets: 610_000_000, versions: 180_000_000, runtimes: 390_000_000, cache: 95_000_000 }),
  clear_download_cache: async () => { await new Promise((r) => setTimeout(r, 400)); },
  check_update: () => (location.search.includes('update') ? { current: '0.1.11', available: '0.1.12', url: 'https://github.com/troppofranco/hyper-launcher/releases/tag/build-12' } : { current: null, available: null, url: null }),
  install_update: () => new Promise(() => {}),
  launch_instance: ({ name }) => { simulateLaunch(name); },
  ...serverCommands(),
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

// ---------- Modalità demo ----------
// `?demo=<pagina>` apre direttamente una pagina con i dati di esempio, a
// onboarding già fatto: la usa lo script che fa gli screenshot delle
// release (`ui/scripts/screenshots.mjs`). Pagine: home, library, instance,
// explore, servers, server, server-players, server-plugins, server-files,
// server-settings, customize, settings, onboarding.

const demo = typeof location === 'undefined' ? null : new URLSearchParams(location.search).get('demo');
// Demo del dialogo dei crash: l'istanza "si chiude" con una mod mancante.
if (demo === 'crash') {
  // Si aspetta che l'interfaccia ascolti gli eventi, poi si "chiude" l'istanza.
  const wait = setInterval(() => {
    if (!handlers.get('instance-state')?.size) return;
    clearInterval(wait);
    setTimeout(() => emit('instance-state', { instance: 'Sopravvivenza', state: 'stopped', error: null, exitCode: 1, crash: { title: 'Mod mancante o incompatibile', cause: "Mod 'Sodium' (sodium) 0.6.0 requires any version of fabric-api, which is missing!", hint: 'Installa la mod richiesta (o la versione giusta) dalla scheda Mod, oppure disattiva quella che la richiede.', file: 'crash-reports/crash.txt' } }), 600);
  }, 100);
}
if (demo && demo !== 'onboarding') state.settings.setupDone = true;

/** Pagina iniziale della modalità demo, se richiesta. */
export function demoRoute(): Route | null {
  if (!demo || demo === 'onboarding') return null;
  const [view, tab] = demo.split('-') as [string, string | undefined];
  switch (view) {
    case 'instance':
      return { view: 'instance', name: state.instances[0].name };
    case 'server':
      return { view: 'server', name: 'Server degli amici', tab };
    case 'crash':
      return { view: 'home' };
    case 'home': case 'library': case 'explore': case 'servers': case 'customize': case 'settings':
      return { view };
    default:
      return null;
  }
}
