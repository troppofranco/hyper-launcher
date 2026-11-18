<script lang="ts">
  import { api, type SavedServer, type ServerStatus } from '../lib/api';
  import { initial } from '../lib/format';
  import { t } from '../lib/i18n.svelte';

  // Server salvati nelle istanze (elenco "Multigiocatore" del gioco), con
  // giocatori online e versione chiesti direttamente ai server.
  let servers = $state<SavedServer[] | null>(null);
  let status = $state<Record<string, ServerStatus | 'offline' | undefined>>({});

  let { limit = 6, showTitle = true }: { limit?: number; showTitle?: boolean } = $props();

  $effect(() => {
    api.listServers().then((list) => (servers = list)).catch(() => (servers = []));
  });

  const shown = $derived(servers?.slice(0, limit) ?? []);
  // Si interrogano solo i server mostrati, una volta ciascuno.
  const asked = new Set<string>();
  $effect(() => {
    for (const s of shown) {
      if (asked.has(s.address)) continue;
      asked.add(s.address);
      api.pingServer(s.address)
        .then((st) => (status[s.address] = st))
        .catch(() => (status[s.address] = 'offline'));
    }
  });
</script>

<section class="servers" aria-label={t('I tuoi server')}>
  {#if showTitle}<h2 id="servers-title" class="section-title">{t('I tuoi server')}</h2>{/if}
  {#if servers === null}
    <div class="card skeleton"></div>
  {:else if !servers.length}
    <div class="card empty">
      <p>{t('Nessun server salvato.')}</p>
      <p class="muted small">{t('Aggiungili dal menu Multigiocatore del gioco: compariranno qui con giocatori online e versione.')}</p>
    </div>
  {:else}
    <ul class="list">
      {#each shown as s, i (s.address)}
        {@const st = status[s.address]}
        <li class="card row enter" style:--i={i} title={s.instances.length ? t('In: {istanze}', { istanze: s.instances.join(', ') }) : undefined}>
          {#if s.icon}<img class="ico" src={s.icon} alt="" />{:else}<span class="ico letter">{initial(s.name)}</span>{/if}
          <span class="meta">
            <span class="name">{s.name}</span>
            <span class="sub">
              {#if st === undefined}
                <span class="dot wait"></span><span class="muted">{t('Controllo…')}</span>
              {:else if st === 'offline'}
                <span class="dot off"></span><span class="muted">{t('Non raggiungibile')}</span>
              {:else}
                <span class="dot on"></span><span>{st.online}/{st.max}</span>
                {#if st.version}<span class="pill">{st.version}</span>{/if}
              {/if}
            </span>
          </span>
          <span class="rank">{st && st !== 'offline' ? `${st.latencyMs} ms` : `#${i + 1}`}</span>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .servers { display: flex; flex-direction: column; min-width: 0; }
  .section-title { font-size: 16px; font-weight: 600; margin-bottom: 12px; }
  .list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
  .row { display: flex; align-items: center; gap: 12px; padding: 10px 14px; transition: transform .18s ease, border-color .18s ease, background .18s ease; }
  .row:hover { transform: translateX(3px); border-color: var(--line-strong); }
  .ico { width: 40px; height: 40px; border-radius: 9px; flex: none; image-rendering: pixelated; }
  .letter { display: flex; align-items: center; justify-content: center; background: var(--surface-2); color: var(--muted); font-weight: 700; }
  .meta { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
  .name { font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sub { display: flex; align-items: center; gap: 7px; font-size: 12px; color: var(--text-2); min-width: 0; }
  .dot { width: 7px; height: 7px; border-radius: 4px; flex: none; }
  .dot.on { background: var(--success); box-shadow: 0 0 0 3px color-mix(in srgb, var(--success) 25%, transparent); }
  .dot.off { background: var(--faint); }
  .dot.wait { background: var(--accent); animation: pulse 1s ease-in-out infinite alternate; }
  .pill { font-family: var(--mono); font-size: 10px; padding: 2px 6px; border-radius: 5px; background: var(--surface-2); color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 160px; }
  .rank { font-size: 12px; color: var(--faint); font-variant-numeric: tabular-nums; flex: none; }
  .empty { padding: 18px; display: flex; flex-direction: column; gap: 6px; }
  .empty p { margin: 0; }
  .skeleton { height: 180px; }
  @keyframes pulse { from { opacity: .35; } to { opacity: 1; } }
</style>
