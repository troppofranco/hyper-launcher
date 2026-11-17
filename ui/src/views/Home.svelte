<script lang="ts">
  import { app } from '../lib/store.svelte';
  import { loaderLabel, playtime, relativeDay } from '../lib/format';
  import InstanceIcon from '../components/InstanceIcon.svelte';
  import PlayButton from '../components/PlayButton.svelte';

  const quick = $derived(app.recent.slice(0, 4));
  const total = $derived(app.instances.reduce((sum, i) => sum + i.playtimeSecs, 0));
  const byPlaytime = $derived(
    [...app.instances].filter((i) => i.playtimeSecs > 0).sort((a, b) => b.playtimeSecs - a.playtimeSecs).slice(0, 5),
  );
  const maxPlaytime = $derived(byPlaytime[0]?.playtimeSecs ?? 1);
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1 class="page-title">{app.activeAccount ? `Bentornato, ${app.activeAccount.name}` : 'Benvenuto in Hyper Launcher'}</h1>
      <p class="muted">{app.instances.length ? 'Riprendi da dove avevi lasciato.' : 'Crea la tua prima istanza per iniziare a giocare.'}</p>
    </div>
    <button class="btn" onclick={() => app.go({ view: 'library' })}>Apri la libreria</button>
  </div>

  <section aria-labelledby="quick">
    <h2 id="quick" class="section-title">Gioca subito</h2>
    {#if quick.length}
      <div class="quick">
        {#each quick as inst (inst.name)}
          <div class="card quick-card">
            <button class="open" onclick={() => app.go({ view: 'instance', name: inst.name })}>
              <InstanceIcon name={inst.name} size={52} />
              <span class="meta">
                <span class="name">{inst.name}</span>
                <span class="muted small">{loaderLabel(inst)} · {inst.version} · {relativeDay(inst.lastPlayed)}</span>
              </span>
            </button>
            <PlayButton name={inst.name} />
          </div>
        {/each}
      </div>
    {:else}
      <div class="card empty">
        <p>Nessuna istanza ancora.</p>
        <button class="btn primary" onclick={() => (app.newInstanceOpen = true)}>+ Nuova istanza</button>
      </div>
    {/if}
  </section>

  <section class="card stats" aria-labelledby="playtime">
    <div class="stats-head">
      <h2 id="playtime" class="section-title">Tempo di gioco</h2>
      <span class="total">{playtime(total)}</span>
    </div>
    {#if byPlaytime.length}
      <ul class="bars">
        {#each byPlaytime as inst (inst.name)}
          <li>
            <span class="bar-name">{inst.name}</span>
            <span class="bar"><span style:width="{Math.max(3, (inst.playtimeSecs / maxPlaytime) * 100)}%"></span></span>
            <span class="bar-value">{playtime(inst.playtimeSecs)}</span>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted">Il tempo di gioco comparirà dopo la prima partita.</p>
    {/if}
  </section>
</div>

<style>
  .section-title { font-size: 15px; font-weight: 600; margin-bottom: 12px; }
  .quick { display: grid; grid-template-columns: repeat(auto-fill, minmax(340px, 1fr)); gap: 14px; }
  .quick-card { padding: 16px; display: flex; align-items: center; gap: 12px; }
  .open { flex: 1; display: flex; align-items: center; gap: 14px; background: transparent; border: 0; text-align: left; min-width: 0; padding: 0; }
  .meta { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .name { font-weight: 600; font-size: 16px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .empty { padding: 24px; display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .empty p { margin: 0; }
  .stats { padding: 20px; }
  .stats-head { display: flex; justify-content: space-between; align-items: baseline; }
  .total { font-family: var(--display); font-size: 26px; font-weight: 600; }
  .bars { list-style: none; margin: 4px 0 0; padding: 0; display: flex; flex-direction: column; gap: 10px; }
  .bars li { display: grid; grid-template-columns: minmax(0, 180px) 1fr 70px; gap: 12px; align-items: center; }
  .bar-name { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--text-2); }
  .bar { height: 10px; border-radius: 5px; background: #262a34; overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--accent); border-radius: 5px; }
  .bar-value { text-align: right; color: var(--muted); font-size: 12px; }
  .stats p { margin: 0; }
</style>
