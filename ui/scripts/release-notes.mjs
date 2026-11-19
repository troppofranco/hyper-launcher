// Note di una release: changelog dai commit dall'ultima release e
// screenshot delle pagine toccate da quei commit.
//
//   node ui/scripts/release-notes.mjs --since v0.1.20 --tag v0.1.21 \
//     --repo owner/repo --out dist/release
//
// Scrive `<out>/notes.md` e `<out>/screen-N-<pagina>.png`. Gli screenshot
// si fanno sull'interfaccia già compilata (`ui/dist`) aperta in modalità
// demo (`?demo=<pagina>`, dati di esempio) con Chrome o Edge headless:
// nessuna dipendenza in più. Senza browser le note escono senza immagini.

import { execFileSync, spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const UI = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const ROOT = path.resolve(UI, '..');
const MAX_SCREENS = 3;
const SIZE = [1320, 820];

const args = Object.fromEntries(
  process.argv.slice(2).reduce((acc, a, i, all) => (a.startsWith('--') ? [...acc, [a.slice(2), all[i + 1]]] : acc), []),
);
const out = path.resolve(args.out ?? 'dist/release');
fs.mkdirSync(out, { recursive: true });

const git = (...a) => execFileSync('git', a, { cwd: ROOT, encoding: 'utf8' }).trim();

// ---------- Commit ----------

/** Commit (dal più vecchio) dopo `since`; senza tag precedente gli ultimi 20. */
function commits() {
  let range = ['-n', '20', 'HEAD'];
  if (args.since) {
    try {
      git('rev-parse', '--verify', '--quiet', `${args.since}^{commit}`);
      range = [`${args.since}..HEAD`];
    } catch {
      console.warn(`tag ${args.since} non trovato: uso gli ultimi 20 commit`);
    }
  }
  const SEP = '\u001e';
  const raw = git('log', '--reverse', '--no-merges', `--format=%H%x1f%s%x1f%b${SEP}`, ...range);
  return raw
    .split(SEP)
    .map((c) => c.trim())
    .filter(Boolean)
    .map((c) => {
      const [hash, subject, body = ''] = c.split('\u001f');
      return { hash, subject: subject.trim(), points: points(body) };
    });
}

/** Punti dal corpo del commit: righe "- …" (anche su più righe) o paragrafi. */
function points(body) {
  const lines = body
    .split('\n')
    .filter((l) => !/^(co-authored-by|signed-off-by):/i.test(l.trim()));
  const result = [];
  let current = null;
  for (const line of lines) {
    const t = line.trim();
    if (!t) {
      current = null;
      continue;
    }
    if (t.startsWith('- ')) {
      current = t.slice(2);
      result.push(current);
    } else if (current !== null) {
      result[result.length - 1] = `${result[result.length - 1]} ${t}`;
    } else {
      current = t;
      result.push(current);
    }
  }
  return result;
}

// ---------- Pagine toccate ----------

// Dal file cambiato alla pagina demo che lo mostra (la prima regola vince).
const PAGES = [
  [/server\/Plugins\.svelte|crates\/server\/src\/plugins\.rs/, 'server-plugins', 'Plugin del server'],
  [/server\/FileBrowser\.svelte|crates\/server\/src\/files\.rs/, 'server-files', 'File del server'],
  [/server\/Players\.svelte|crates\/server\/src\/players\.rs/, 'server-players', 'Giocatori del server'],
  [/server\/ServerSettings\.svelte|crates\/server\/src\/properties\.rs/, 'server-settings', 'Impostazioni del server'],
  [/views\/Servers\.svelte|server\/NewServerDialog\.svelte/, 'servers', 'Server'],
  [/views\/ServerPage\.svelte|components\/server\/|crates\/server\/|crates\/app\/src\/servers\.rs/, 'server', 'Pagina del server'],
  [/views\/Home\.svelte|components\/widgets\/|lib\/widgets\.ts/, 'home', 'Home'],
  [/views\/Library\.svelte/, 'library', 'Libreria'],
  [/views\/InstancePage\.svelte|components\/ContentList\.svelte/, 'instance', 'Istanza'],
  [/views\/(Explore|ProjectPage)\.svelte|CurseForgeBrowse\.svelte|crates\/content\//, 'explore', 'Esplora'],
  [/views\/Customize\.svelte|SkinViewer\.svelte/, 'customize', 'Personalizzazione'],
  [/views\/Settings\.svelte|views\/settings\/|lib\/look\.ts/, 'settings', 'Impostazioni'],
  [/components\/Onboarding\.svelte/, 'onboarding', 'Primo avvio'],
];

/** Pagine da fotografare: quelle toccate più spesso, al massimo tre. */
function pages(list) {
  const score = new Map();
  for (const c of list) {
    const files = git('show', '--name-only', '--format=', c.hash).split('\n').filter(Boolean);
    const seen = new Set();
    for (const f of files) {
      const page = PAGES.find(([re]) => re.test(f));
      if (page && !seen.has(page[1])) {
        seen.add(page[1]);
        score.set(page[1], (score.get(page[1]) ?? 0) + 1);
      }
    }
  }
  // Se è cambiata una scheda precisa del server, la pagina generica (che
  // apre la Console) sarebbe un doppione meno utile.
  if ([...score.keys()].some((p) => p.startsWith('server-'))) score.delete('server');
  return [...score.entries()]
    .sort((a, b) => b[1] - a[1])
    .slice(0, MAX_SCREENS)
    .map(([p]) => PAGES.find((x) => x[1] === p));
}

// ---------- Screenshot ----------

function findBrowser() {
  const candidates = [
    process.env.CHROME,
    'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
    'C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe',
    'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
    'C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe',
    '/usr/bin/google-chrome',
    '/usr/bin/chromium',
  ];
  return candidates.find((c) => c && fs.existsSync(c)) ?? null;
}

async function serve() {
  const vite = path.join(UI, 'node_modules', 'vite', 'bin', 'vite.js');
  const port = 4317;
  const child = spawn(process.execPath, [vite, 'preview', '--port', String(port), '--strictPort'], { cwd: UI, stdio: 'ignore' });
  for (let i = 0; i < 50; i++) {
    try {
      await fetch(`http://localhost:${port}/`);
      return { url: `http://localhost:${port}/`, stop: () => child.kill() };
    } catch {
      await new Promise((r) => setTimeout(r, 200));
    }
  }
  child.kill();
  throw new Error('anteprima dell’interfaccia non avviata');
}

function shoot(browser, url, file) {
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'hyper-shot-'));
  try {
    execFileSync(browser, [
      '--headless=new', '--disable-gpu', '--hide-scrollbars', '--no-first-run', '--no-default-browser-check',
      '--disable-extensions', `--user-data-dir=${profile}`, `--window-size=${SIZE.join(',')}`,
      // Tempo "virtuale" per dati simulati e animazioni d'entrata.
      '--virtual-time-budget=8000', `--screenshot=${file}`, url,
    ], { stdio: 'ignore', timeout: 60_000 });
  } finally {
    fs.rmSync(profile, { recursive: true, force: true });
  }
  return fs.existsSync(file);
}

async function screenshots(list) {
  const browser = findBrowser();
  if (!browser || !fs.existsSync(path.join(UI, 'dist', 'index.html'))) {
    console.warn('niente screenshot: serve un Chrome/Edge e ui/dist compilato');
    return [];
  }
  const server = await serve();
  const done = [];
  try {
    for (const [i, [, page, title]] of list.entries()) {
      const name = `screen-${i + 1}-${page}.png`;
      // La barra del titolo mostra la versione della release.
      const version = args.tag ? `&version=${encodeURIComponent(args.tag.replace(/^v/, ''))}` : '';
      if (shoot(browser, `${server.url}?demo=${page}${version}`, path.join(out, name))) done.push({ name, title });
    }
  } finally {
    server.stop();
  }
  return done;
}

// ---------- Note ----------

const list = commits();
const shots = await screenshots(pages(list));
const asset = (name) => (args.repo && args.tag ? `https://github.com/${args.repo}/releases/download/${args.tag}/${name}` : name);

let md = '## Novità\n\n';
md += list.length
  ? list.map((c) => [`- **${c.subject}**`, ...c.points.map((p) => `  - ${p}`)].join('\n')).join('\n')
  : '- Nessuna modifica rispetto alla release precedente.';
md += '\n';
if (shots.length) {
  md += '\n## Screenshot\n\n';
  md += shots.map((s) => `**${s.title}**\n\n![${s.title}](${asset(s.name)})`).join('\n\n');
  md += '\n';
}
if (args.since && args.repo) md += `\nTutte le modifiche: https://github.com/${args.repo}/compare/${args.since}...${args.tag ?? 'HEAD'}\n`;

fs.writeFileSync(path.join(out, 'notes.md'), md);
console.log(`note: ${list.length} commit, ${shots.length} screenshot in ${out}`);
