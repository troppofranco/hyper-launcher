<script lang="ts">
  import { api, type ContentDirName, type ContentKind, type InstalledContent } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { megabytes } from '../lib/format';
  import { dirOf, kindLabel } from '../lib/content';
  import Icon from './Icon.svelte';
  import ProjectIcon from './ProjectIcon.svelte';
  import ProgressBar from './ProgressBar.svelte';

  // Mod, resource pack o shader di un'istanza, riconosciuti su Modrinth dal
  // loro hash, con aggiornamenti e collegamento al catalogo.
  let { instance, kind, oncount }: {
    instance: string;
    kind: ContentKind;
    oncount?: (n: number) => void;
  } = $props();
  const dir = $derived(dirOf(kind) as ContentDirName);
  const label = $derived(kindLabel(kind));

  let items = $state<InstalledContent[]>([]);
  let loading = $state(true);
  let query = $state('');
  let updating = $state<string[] | null>(null);

  const running = $derived(!!app.launches[instance]);
  const updates = $derived(items.filter((i) => i.update));
  const shown = $derived(items.filter((i) => {
    const q = query.trim().toLowerCase();
    return !q || (i.title ?? i.name).toLowerCase().includes(q) || i.file.toLowerCase().includes(q);
  }));
  const progress = $derived(updating ? app.contentProgress[instance] : undefined);

  async function load() {
    try {
      items = await api.installed(instance, dir, true);
      oncount?.(items.length);
    } catch (e) {
      app.error(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void [instance, dir];
    loading = true;
    load();
  });

  async function act(op: () => Promise<unknown>) {
    try {
      await op();
    } catch (e) {
      app.error(e);
    }
    await load();
  }

  async function runUpdate(files: string[]) {
    updating = files;
    try {
      const n = await app.withContentProgress(instance, () => api.updateContent(instance, dir, files));
      app.info_(n === 1 ? 'Aggiornato 1 file.' : `Aggiornati ${n} file.`);
    } catch (e) {
      app.error(e);
    } finally {
      updating = null;
      await load();
    }
  }
</script>

<div class="tools">
  <input class="input grow" type="search" aria-label="Cerca tra {label.toLowerCase()}" placeholder="Cerca tra {label.toLowerCase()}…" bind:value={query} />
  <button class="btn" onclick={() => api.openFolder(instance, dir)}><Icon name="folder" size={16} /> Apri cartella</button>
  <button class="btn primary" onclick={() => app.browse(instance, kind)}><Icon name="plus" size={16} /> Aggiungi contenuti</button>
</div>

{#if running}<p class="note">Il gioco è in esecuzione: chiudilo per attivare, eliminare o aggiornare i file.</p>{/if}

{#if updates.length}
  <div class="banner">
    <span class="grow"><strong>{updates.length === 1 ? '1 aggiornamento disponibile' : `${updates.length} aggiornamenti disponibili`}</strong>
      <span class="muted"> · {updates.slice(0, 3).map((u) => u.title ?? u.name).join(', ')}{updates.length > 3 ? '…' : ''}</span></span>
    {#if updating}
      <span class="progress"><ProgressBar {progress} label={false} /></span>
    {:else}
      <button class="btn primary small" disabled={running} onclick={() => runUpdate([])}>Aggiorna tutto</button>
    {/if}
  </div>
{/if}

<div class="card list">
  {#if loading && !items.length}
    <p class="muted pad">Lettura dei file e controllo su Modrinth…</p>
  {/if}
  {#each shown as item, i (item.file)}
    <div class="row enter" style:--i={i} class:off={!item.enabled}>
      <button class="who" disabled={!item.projectId} onclick={() => item.projectId && app.openProject(item.projectId, kind, instance)}>
        <ProjectIcon url={item.iconUrl} name={item.title ?? item.name} size={40} />
        <span class="meta">
          <span class="name">{item.title ?? item.name}</span>
          <span class="muted small">{item.file} · {megabytes(item.sizeBytes)}{item.projectId ? '' : ' · non trovato su Modrinth'}</span>
        </span>
      </button>
      <span class="ver">
        {#if item.versionNumber}{item.versionNumber}{/if}
        {#if item.update}<span class="arrow">→ {item.update}</span>{/if}
      </span>
      <span class="act">
        {#if item.update}
          <button class="btn small upd" disabled={running || !!updating} onclick={() => runUpdate([item.file])}>Aggiorna</button>
        {/if}
      </span>
      <button class="switch" role="switch" aria-checked={item.enabled} aria-label="{item.enabled ? 'Disattiva' : 'Attiva'} {item.title ?? item.name}" disabled={running} onclick={() => act(() => api.setContentEnabled(instance, dir, item.file, !item.enabled))}></button>
      <button class="icon-btn" aria-label="Elimina {item.title ?? item.name}" disabled={running} onclick={() => act(() => api.deleteContent(instance, dir, item.file))}><Icon name="trash" size={18} /></button>
    </div>
  {:else}
    {#if !loading}
      <div class="empty pad">
        <p class="muted">{items.length ? 'Nessun file corrisponde alla ricerca.' : `Nessun contenuto in ${label.toLowerCase()}.`}</p>
        {#if !items.length}<button class="btn" onclick={() => app.browse(instance, kind)}>Cerca su Modrinth</button>{/if}
      </div>
    {/if}
  {/each}
</div>

<style>
  .tools { display: flex; gap: 10px; flex-wrap: wrap; }
  .grow { flex: 1 1 240px; min-width: 0; }
  .note { margin: 0; padding: 10px 14px; border-radius: 10px; background: var(--surface); border: 1px solid var(--line); color: var(--text-2); font-size: 13px; }
  .banner { display: flex; align-items: center; gap: 12px; padding: 12px 16px; border-radius: 12px; background: var(--accent-soft); border: 1px solid color-mix(in srgb, var(--accent) 50%, transparent); }
  .progress { width: 140px; }
  .list { overflow: hidden; }
  .row { display: flex; align-items: center; gap: 14px; padding: 10px 16px; border-bottom: 1px solid #1f232c; }
  .row:last-child { border-bottom: 0; }
  .row.off .who { opacity: .55; }
  .who { flex: 1; min-width: 0; display: flex; align-items: center; gap: 14px; background: none; border: 0; padding: 0; text-align: left; }
  .who:disabled { cursor: default; opacity: 1; }
  .meta { min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .name { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta .small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ver { flex: 0 0 auto; max-width: 220px; font-family: var(--mono); font-size: 12px; color: var(--text-2); display: flex; flex-direction: column; align-items: flex-end; overflow: hidden; }
  .arrow { color: var(--accent-line); }
  .act { flex: 0 0 86px; display: flex; justify-content: flex-end; }
  .upd { background: color-mix(in srgb, #3fb97f 18%, transparent); border-color: transparent; color: #7ee0ad; }
  .pad { padding: 16px; margin: 0; }
  .empty { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .empty p { margin: 0; }
  .icon-btn { width: 36px; height: 36px; border-radius: 8px; border: 0; background: transparent; color: var(--muted); display: flex; align-items: center; justify-content: center; flex: none; }
  .icon-btn:hover:not(:disabled) { color: #ff8a8e; background: var(--surface-2); }
</style>
