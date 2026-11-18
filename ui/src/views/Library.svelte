<script lang="ts">
  import { app } from '../lib/store.svelte';
  import { loaderLabel, playtime, relativeDay } from '../lib/format';
  import InstanceIcon from '../components/InstanceIcon.svelte';

  let query = $state('');
  let sort = $state<'recent' | 'name' | 'playtime'>('recent');

  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = app.recent.filter((i) => !q || i.name.toLowerCase().includes(q) || i.version.includes(q));
    if (sort === 'name') return [...list].sort((a, b) => a.name.localeCompare(b.name, 'it'));
    if (sort === 'playtime') return [...list].sort((a, b) => b.playtimeSecs - a.playtimeSecs);
    return list;
  });
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1 class="page-title">Libreria</h1>
      <p class="muted">Le tue istanze di Minecraft, ognuna con mondi, mod e impostazioni separati.</p>
    </div>
    <span class="head-actions">
      <button class="btn" onclick={() => app.importPack()} disabled={'' in app.contentProgress}>{'' in app.contentProgress ? 'Importazione…' : 'Importa .mrpack'}</button>
      <button class="btn" onclick={() => app.browse(null, 'modpack')}>Sfoglia modpack</button>
      <button class="btn primary" onclick={() => (app.newInstanceOpen = true)}>+ Nuova istanza</button>
    </span>
  </div>

  <div class="tools">
    <label class="search">
      <span class="visually-hidden">Cerca istanze</span>
      <input class="input" type="search" placeholder="Cerca istanze…" bind:value={query} />
    </label>
    <label class="sort">Ordina
      <select class="input" bind:value={sort}>
        <option value="recent">Ultima partita</option>
        <option value="name">Nome</option>
        <option value="playtime">Tempo di gioco</option>
      </select>
    </label>
  </div>

  {#if shown.length}
    <div class="grid">
      {#each shown as inst, i (inst.name)}
        <button class="card tile enter" style:--i={i} onclick={() => app.go({ view: 'instance', name: inst.name })}>
          <span class="tile-top">
            <InstanceIcon name={inst.name} icon={inst.icon} size={48} />
            <span class="muted small">{app.launches[inst.name] ? 'In corso' : relativeDay(inst.lastPlayed)}</span>
          </span>
          <span class="tile-name">{inst.name}</span>
          <span class="muted small">{inst.playtimeSecs ? `${playtime(inst.playtimeSecs)} giocate` : 'Mai giocata'}</span>
          <span class="tile-foot"><span>{loaderLabel(inst)}</span><span>{inst.version}</span></span>
        </button>
      {/each}
    </div>
  {:else if app.instances.length}
    <p class="muted">Nessuna istanza corrisponde a “{query}”.</p>
  {:else}
    <div class="card empty">
      <p>La libreria è vuota.</p>
      <button class="btn primary" onclick={() => (app.newInstanceOpen = true)}>Crea la prima istanza</button>
    </div>
  {/if}
</div>

<style>
  .head-actions { display: flex; gap: 10px; flex-wrap: wrap; }
  .tools { display: flex; gap: 12px; flex-wrap: wrap; }
  .search { flex: 1 1 320px; display: flex; }
  .search .input { flex: 1; height: 44px; }
  .sort { display: flex; align-items: center; gap: 8px; color: var(--muted); }
  .sort .input { height: 44px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 16px; }
  .tile { padding: 18px; display: flex; flex-direction: column; gap: 6px; text-align: left; }
  .tile:hover { border-color: var(--line-strong); background: #1a1d25; }
  .tile-top { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 8px; }
  .tile-name { font-weight: 600; font-size: 16px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tile-foot { display: flex; justify-content: space-between; font-size: 12px; color: var(--muted); margin-top: 8px; }
  .empty { padding: 24px; display: flex; justify-content: space-between; align-items: center; }
  .empty p { margin: 0; }
</style>
