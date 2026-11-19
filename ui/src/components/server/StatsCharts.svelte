<script lang="ts">
  // CPU, RAM e giocatori degli ultimi 5 minuti, con grafici SVG semplici
  // (nessuna libreria). Campioni nuovi ogni 2 s mentre il server è acceso.
  import { api, type ServerStats, type StatsPoint } from '../../lib/api';
  import { t } from '../../lib/i18n.svelte';
  import { gb, poll } from './util';

  let { name, running, maxPlayers }: { name: string; running: boolean; maxPlayers: number } = $props();

  const W = 300;
  const H = 70;
  const WINDOW = 300;

  let stats = $state<ServerStats | null>(null);

  async function load() {
    stats = await api.serverStats(name);
  }

  $effect(() => {
    if (running) return poll(load, 2000);
  });

  const points = $derived(running ? (stats?.points ?? []) : []);
  const last = $derived(points.at(-1) ?? null);
  const end = $derived(last?.t ?? 0);
  // Appena acceso ci sono pochi campioni: il grafico si allarga su quelli
  // che ci sono, fino a coprire gli ultimi 5 minuti.
  const start = $derived(Math.max(points[0]?.t ?? 0, end - WINDOW));
  const span = $derived(Math.max(1, end - start));

  const x = (p: StatsPoint) => (Math.max(0, p.t - start) / span) * W;
  const y = (v: number, max: number) => H - 3 - (Math.min(v, max) / max) * (H - 8);

  /** Linea dei valori; con `step` sale a gradini (giocatori). I null spezzano la linea. */
  function line(get: (p: StatsPoint) => number | null, max: number, step = false): string {
    let d = '';
    let prev: number | null = null;
    for (const p of points) {
      const v = get(p);
      if (v === null) { prev = null; continue; }
      const px = x(p).toFixed(1);
      const py = y(v, max).toFixed(1);
      if (prev === null) d += `M${px} ${py}`;
      else d += step ? `H${px}V${py}` : `L${px} ${py}`;
      prev = v;
    }
    return d;
  }

  /** Area sotto la linea, chiusa sul fondo del grafico. */
  function area(d: string): string {
    if (!d || points.length < 2) return '';
    return `${d}L${x(points.at(-1)!).toFixed(1)} ${H}L${x(points[0]).toFixed(1)} ${H}Z`;
  }

  const memMax = $derived(stats?.memMaxMb || 1024);
  const playersMax = $derived(Math.max(1, maxPlayers, ...points.map((p) => p.players)));
  const latencyMax = $derived(Math.max(50, ...points.map((p) => p.latencyMs ?? 0)));

  const cpuLine = $derived(line((p) => p.cpu, 100));
  const memLine = $derived(line((p) => p.memMb, memMax));
  const playerLine = $derived(line((p) => p.players, playersMax, true));
  const latencyLine = $derived(line((p) => p.latencyMs, latencyMax));
</script>

<div class="stats">
  <section class="card chart cpu" aria-label={t('CPU')}>
    <div class="top"><span class="name">{t('CPU')}</span><span><span class="big">{last ? `${Math.round(last.cpu)}%` : '—'}</span></span></div>
    {#if running}
      <svg viewBox="0 0 {W} {H}" width="100%" height={H} preserveAspectRatio="none" aria-hidden="true">
        <path class="area" d={area(cpuLine)} /><path class="stroke" d={cpuLine} />
      </svg>
    {:else}
      <span class="empty">{t('Server spento')}</span>
    {/if}
  </section>
  <section class="card chart mem" aria-label={t('RAM')}>
    <div class="top"><span class="name">{t('RAM')}</span><span><span class="big">{last ? (last.memMb / 1024).toFixed(1) : '—'}</span>{#if last}<span class="muted"> / {gb(memMax)}</span>{/if}</span></div>
    {#if running}
      <svg viewBox="0 0 {W} {H}" width="100%" height={H} preserveAspectRatio="none" aria-hidden="true">
        <path class="area" d={area(memLine)} /><path class="stroke" d={memLine} />
      </svg>
    {:else}
      <span class="empty">{t('Server spento')}</span>
    {/if}
  </section>
  <section class="card chart players" aria-label={t('Giocatori · latenza')}>
    <div class="top">
      <span class="name">{t('Giocatori · latenza')}</span>
      <span><span class="big">{last ? last.players : '—'}</span><span class="muted"> / {maxPlayers}{#if last?.latencyMs != null} · {last.latencyMs} ms{/if}</span></span>
    </div>
    {#if running}
      <svg viewBox="0 0 {W} {H}" width="100%" height={H} preserveAspectRatio="none" aria-hidden="true">
        <path class="area" d={area(playerLine)} /><path class="stroke" d={playerLine} /><path class="latency" d={latencyLine} />
      </svg>
    {:else}
      <span class="empty">{t('Server spento')}</span>
    {/if}
  </section>
</div>

<style>
  .stats { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 14px; }
  .chart { padding: 14px 16px 10px; display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  .top { display: flex; justify-content: space-between; align-items: baseline; gap: 8px; }
  .name { font-weight: 600; }
  .big + .muted { margin-left: 4px; }
  .big { font-family: var(--display); font-size: 22px; font-weight: 600; font-variant-numeric: tabular-nums; }
  svg { display: block; overflow: visible; }
  path { vector-effect: non-scaling-stroke; }
  .area { fill: var(--c); fill-opacity: .18; stroke: none; }
  .stroke { fill: none; stroke: var(--c); stroke-width: 2; stroke-linejoin: round; }
  .latency { fill: none; stroke: var(--warn-text); stroke-width: 1.5; stroke-dasharray: 4 3; }
  .cpu { --c: var(--accent); }
  .mem { --c: var(--text-2); }
  .players { --c: var(--success); }
  .empty { height: 70px; display: flex; align-items: center; justify-content: center; border-radius: var(--radius-sm); background: var(--panel); color: var(--faint); font-size: 12px; }
  @media (max-width: 860px) { .stats { grid-template-columns: 1fr; } }
</style>
