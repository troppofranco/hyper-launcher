// Piccoli aiuti condivisi dalle schermate dei server ospitati.

import type { ServerInfo } from '../../lib/api';
import { playtime } from '../../lib/format';
import { t } from '../../lib/i18n.svelte';
import { app } from '../../lib/store.svelte';

/** "Paper 1.21.4 · build 232" oppure "Vanilla 1.21.4". */
export function kindLabel(s: Pick<ServerInfo, 'kind' | 'version' | 'build'>): string {
  const base = `${s.kind === 'paper' ? 'Paper' : 'Vanilla'} ${s.version}`;
  return s.build !== null ? `${base} · ${t('build {n}', { n: s.build })}` : base;
}

/** Da quanto è acceso il server ("2h 14m"). */
export function uptime(startedAt: number | null): string {
  return startedAt ? playtime(Math.max(0, Math.floor(Date.now() / 1000) - startedAt)) : '';
}

export function gb(mb: number): string {
  const v = mb / 1024;
  return `${Number.isInteger(v) ? v : v.toFixed(1)} GB`;
}

export async function copyText(text: string, done = t('Indirizzo copiato.')) {
  try {
    await navigator.clipboard.writeText(text);
    app.info_(done);
  } catch (e) {
    app.error(e);
  }
}

/** Prima porta libera da 25565 in su, tra quelle dei server esistenti. */
export function freePort(servers: ServerInfo[]): number {
  const used = new Set(servers.map((s) => s.port));
  let port = 25565;
  while (used.has(port)) port++;
  return port;
}

/**
 * Chiama `fn` subito e poi ogni `every` ms, senza sovrapporre le chiamate;
 * restituisce la funzione che ferma tutto (da usare come pulizia di `$effect`).
 */
export function poll(fn: () => Promise<unknown>, every: number): () => void {
  let stopped = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const tick = async () => {
    try {
      await fn();
    } catch {
      // Chi chiama gestisce i suoi errori; qui conta solo continuare a chiedere.
    } finally {
      if (!stopped) timer = setTimeout(tick, every);
    }
  };
  void tick();
  return () => {
    stopped = true;
    clearTimeout(timer);
  };
}

/**
 * Nome valido per un server: lettere senza accenti, numeri, spazi, - e _
 * (massimo 32), come controlla il backend (`validate_name`).
 */
export const VALID_SERVER_NAME = /^[A-Za-z0-9 _-]{1,32}$/;
