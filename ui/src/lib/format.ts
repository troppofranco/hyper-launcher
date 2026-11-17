// Formattazione in italiano di tempi, date e dimensioni.

export function playtime(secs: number): string {
  if (secs < 60) return secs > 0 ? 'meno di 1m' : '0m';
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

/** "Oggi", "Ieri", "3 giorni fa" oppure la data. */
export function relativeDay(unixSecs: number | null): string {
  if (!unixSecs) return 'Mai';
  const date = new Date(unixSecs * 1000);
  const today = new Date();
  const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((startOfDay(today) - startOfDay(date)) / 86_400_000);
  if (days <= 0) return 'Oggi';
  if (days === 1) return 'Ieri';
  if (days < 7) return `${days} giorni fa`;
  return date.toLocaleDateString('it-IT', { day: 'numeric', month: 'short', year: 'numeric' });
}

export function isoDate(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleDateString('it-IT', { day: 'numeric', month: 'short', year: 'numeric' });
}

export function megabytes(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

const PALETTE: [string, string][] = [
  ['#1d3b33', '#7ee2c0'],
  ['#2b3320', '#b9e27e'],
  ['#33291d', '#e2b97e'],
  ['#2a2340', '#b4a9ff'],
  ['#3a2230', '#f0a3c8'],
  ['#1f2c3d', '#8ec5ff'],
];

/** Colori stabili per l'icona di un'istanza, ricavati dal nome. */
export function instanceColors(name: string): { bg: string; fg: string } {
  let hash = 0;
  for (const ch of name) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  const [bg, fg] = PALETTE[hash % PALETTE.length];
  return { bg, fg };
}

export function initial(name: string): string {
  return (name.trim()[0] ?? '?').toUpperCase();
}

export function loaderLabel(i: { version: string; loader: { name: string; version: string } | null }): string {
  return i.loader ? `${i.loader.name} ${i.loader.version}` : 'Vanilla';
}

const compactFormat = new Intl.NumberFormat('it-IT', { notation: 'compact', maximumFractionDigits: 1 });

/** "98 Mln", "12.000" → numeri brevi per i download. */
export function compact(n: number): string {
  return compactFormat.format(n);
}

/** Come relativeDay, ma da una data ISO (es. quelle di Modrinth). */
export function relativeIso(iso: string): string {
  const t = Date.parse(iso);
  return Number.isNaN(t) ? '' : relativeDay(Math.floor(t / 1000));
}
