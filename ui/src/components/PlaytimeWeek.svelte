<script lang="ts">
  import { api, type PlaySession } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { playtime } from '../lib/format';

  // Tempo di gioco degli ultimi 7 giorni, giorno per giorno (fuso orario
  // locale), sommando tutte le istanze. Una partita a cavallo della
  // mezzanotte viene divisa tra i due giorni.
  const DAYS = 7;
  let sessions = $state<PlaySession[]>([]);
  // Ricalcola i giorni a mezzanotte anche se il launcher resta aperto.
  let today = $state(startOfToday());

  function startOfToday(): number {
    const d = new Date();
    d.setHours(0, 0, 0, 0);
    return d.getTime();
  }

  $effect(() => {
    const timer = setInterval(() => {
      const t = startOfToday();
      if (t !== today) today = t;
    }, 60_000);
    return () => clearInterval(timer);
  });

  $effect(() => {
    void app.sessionsRev; // si aggiorna quando finisce una partita
    api.recentSessions(DAYS + 1).then((s) => (sessions = s)).catch(() => (sessions = []));
  });

  const days = $derived.by(() => {
    const base = new Date(today);
    return Array.from({ length: DAYS }, (_, i) => {
      const start = new Date(base);
      start.setDate(base.getDate() - (DAYS - 1 - i));
      const end = new Date(start);
      end.setDate(start.getDate() + 1);
      const from = start.getTime() / 1000;
      const to = end.getTime() / 1000;
      const secs = sessions.reduce((sum, s) => sum + Math.max(0, Math.min(s.end, to) - Math.max(s.start, from)), 0);
      const weekday = start.toLocaleDateString('it-IT', { weekday: 'short' }).replace('.', '');
      return { label: i === DAYS - 1 ? 'Oggi' : weekday[0].toUpperCase() + weekday.slice(1), secs };
    });
  });

  // Stessa finestra del grafico: dalla mezzanotte di 6 giorni fa.
  const windowStart = $derived.by(() => {
    const d = new Date(today);
    d.setDate(d.getDate() - (DAYS - 1));
    return d.getTime() / 1000;
  });
  const inWindow = $derived(sessions.filter((s) => s.end > windowStart));
  const total = $derived(days.reduce((sum, d) => sum + d.secs, 0));
  const max = $derived(Math.max(...days.map((d) => d.secs), 1));
  const average = $derived(inWindow.length ? total / inWindow.length : 0);
  const longest = $derived(inWindow.reduce((m, s) => Math.max(m, s.end - Math.max(s.start, windowStart)), 0));
  const played = $derived(days.filter((d) => d.secs > 0).length);
  const peak = $derived(days.findIndex((d) => d.secs === max && d.secs > 0));
</script>

<section class="card week" aria-labelledby="week-title">
  <div class="head">
    <h2 id="week-title" class="section-title">Il tuo tempo di gioco</h2>
    <div class="total">{playtime(total)}</div>
    <div class="muted small">Ultimi 7 giorni</div>
  </div>

  <div class="chart" role="img" aria-label="Tempo di gioco per giorno: {days.map((d) => `${d.label} ${playtime(d.secs)}`).join(', ')}">
    {#each days as day, i (i)}
      <div class="col">
        <div class="track">
          {#if i === peak}<span class="peak">{playtime(day.secs)}</span>{/if}
          <div class="bar" class:empty={!day.secs} style:height="{day.secs ? Math.max(4, (day.secs / max) * 100) : 0}%"></div>
        </div>
        <span class="day" class:today={i === days.length - 1}>{day.label}</span>
      </div>
    {/each}
  </div>

  <div class="stats">
    <div><span class="value">{playtime(Math.round(average))}</span><span class="muted small">Partita media</span></div>
    <div><span class="value">{playtime(longest)}</span><span class="muted small">Partita più lunga</span></div>
    <div><span class="value">{played} su 7</span><span class="muted small">Giorni giocati</span></div>
  </div>
</section>

<style>
  .week { padding: 20px; display: flex; flex-direction: column; gap: 18px; }
  .section-title { font-size: 15px; font-weight: 600; margin: 0 0 10px; }
  .total { font-family: var(--display); font-size: 34px; font-weight: 600; line-height: 1.1; font-variant-numeric: tabular-nums; }
  .chart { display: grid; grid-template-columns: repeat(7, 1fr); gap: 10px; height: 170px; }
  .col { display: flex; flex-direction: column; align-items: center; gap: 8px; min-width: 0; }
  .track { flex: 1; width: 100%; display: flex; flex-direction: column; justify-content: flex-end; align-items: center; position: relative; }
  .bar { width: min(44px, 100%); border-radius: 8px 8px 4px 4px; background: var(--accent); transition: height .3s; }
  .bar.empty { height: 3px !important; background: var(--line-strong); border-radius: 2px; }
  .peak { font-size: 11px; font-weight: 600; color: var(--accent-line); margin-bottom: 4px; white-space: nowrap; }
  .day { font-size: 12px; color: var(--muted); }
  .day.today { color: var(--text); font-weight: 600; }
  .stats { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; border-top: 1px solid var(--line); padding-top: 14px; }
  .stats div { display: flex; flex-direction: column; gap: 2px; }
  .value { font-weight: 600; font-size: 16px; font-variant-numeric: tabular-nums; }
</style>
