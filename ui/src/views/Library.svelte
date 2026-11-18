<script lang="ts">
  import { app } from '../lib/store.svelte';
  import { loaderLabel, playtime, relativeDay } from '../lib/format';
  import InstanceIcon from '../components/InstanceIcon.svelte';
  import Icon from '../components/Icon.svelte';
  import DeleteInstanceDialog from '../components/DeleteInstanceDialog.svelte';
  import { t, locale } from '../lib/i18n.svelte';

  let query = $state('');
  let sort = $state<'recent' | 'name' | 'playtime'>('recent');
  /** Istanza da eliminare (apre la conferma). */
  let deleting = $state<string | null>(null);

  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = app.recent.filter((i) => !q || i.name.toLowerCase().includes(q) || i.version.includes(q));
    if (sort === 'name') return [...list].sort((a, b) => a.name.localeCompare(b.name, locale()));
    if (sort === 'playtime') return [...list].sort((a, b) => b.playtimeSecs - a.playtimeSecs);
    return list;
  });
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1 class="page-title">{t('Libreria')}</h1>
      <p class="muted">{t('Le tue istanze di Minecraft, ognuna con mondi, mod e impostazioni separati.')}</p>
    </div>
    <span class="head-actions">
      <button class="btn" onclick={() => app.importPack()} disabled={'' in app.contentProgress}>{'' in app.contentProgress ? t('Importazione…') : t('Importa modpack')}</button>
      <button class="btn" onclick={() => app.browse(null, 'modpack')}>{t('Sfoglia modpack')}</button>
      <button class="btn primary" onclick={() => (app.newInstanceOpen = true)}>{t('+ Nuova istanza')}</button>
    </span>
  </div>

  <div class="tools">
    <label class="search">
      <span class="visually-hidden">{t('Cerca istanze')}</span>
      <input class="input" type="search" placeholder={t('Cerca istanze…')} bind:value={query} />
    </label>
    <label class="sort">{t('Ordina')}
      <select class="input" bind:value={sort}>
        <option value="recent">{t('Ultima partita')}</option>
        <option value="name">{t('Nome')}</option>
        <option value="playtime">{t('Tempo di gioco')}</option>
      </select>
    </label>
  </div>

  {#if shown.length}
    <div class="grid">
      {#each shown as inst, i (inst.name)}
        <div class="tile-wrap enter" style:--i={i}>
        <button class="card tile" onclick={() => app.go({ view: 'instance', name: inst.name })}>
          <span class="tile-top">
            <InstanceIcon name={inst.name} icon={inst.icon} size={48} />
            <span class="muted small tile-date">{app.launches[inst.name] ? t('In corso') : relativeDay(inst.lastPlayed)}</span>
          </span>
          <span class="tile-name">{inst.name}</span>
          <span class="muted small">{inst.playtimeSecs ? t('{time} giocate', { time: playtime(inst.playtimeSecs) }) : t('Mai giocata')}</span>
          <span class="tile-foot"><span>{loaderLabel(inst)}</span><span>{inst.version}</span></span>
        </button>
          <button class="tile-del" disabled={!!app.launches[inst.name]} aria-label={t('Elimina {nome}', { nome: inst.name })} title={t('Elimina')} onclick={() => (deleting = inst.name)}><Icon name="trash" size={15} /></button>
        </div>
      {/each}
    </div>
  {:else if app.instances.length}
    <p class="muted">{t('Nessuna istanza corrisponde a “{query}”.', { query })}</p>
  {:else}
    <div class="card empty">
      <p>{t('La libreria è vuota.')}</p>
      <button class="btn primary" onclick={() => (app.newInstanceOpen = true)}>{t('Crea la prima istanza')}</button>
    </div>
  {/if}
</div>

{#if deleting}<DeleteInstanceDialog name={deleting} onclose={() => (deleting = null)} />{/if}

<style>
  .head-actions { display: flex; gap: 10px; flex-wrap: wrap; }
  .tools { display: flex; gap: 12px; flex-wrap: wrap; }
  .search { flex: 1 1 320px; display: flex; }
  .search .input { flex: 1; height: 44px; }
  .sort { display: flex; align-items: center; gap: 8px; color: var(--muted); }
  .sort .input { height: 44px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 16px; }
  .tile-wrap { position: relative; display: flex; }
  .tile { flex: 1; min-width: 0; padding: 18px; display: flex; flex-direction: column; gap: 6px; text-align: left; }
  .tile-del { position: absolute; top: 12px; right: 12px; width: 32px; height: 32px; border-radius: 9px; border: 1px solid var(--line-strong); background: var(--surface); color: var(--muted); display: flex; align-items: center; justify-content: center; padding: 0; opacity: 0; transform: scale(.85); transition: opacity .18s ease, transform .18s ease, color .18s ease; }
  .tile-wrap:hover .tile-del, .tile-del:focus-visible { opacity: 1; transform: translateY(-3px); }
  /* La data in alto a destra lascia il posto al cestino. */
  .tile-date { transition: opacity .18s ease; }
  .tile-wrap:hover .tile-date { opacity: 0; }
  .tile-del:hover:not(:disabled) { color: var(--danger-text); border-color: color-mix(in srgb, var(--danger) 60%, transparent); }
  .tile:hover { border-color: var(--line-strong); background: var(--surface-2); }
  .tile-top { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 8px; }
  .tile-name { font-weight: 600; font-size: 16px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tile-foot { display: flex; justify-content: space-between; font-size: 12px; color: var(--muted); margin-top: 8px; }
  .empty { padding: 24px; display: flex; justify-content: space-between; align-items: center; }
  .empty p { margin: 0; }
</style>
