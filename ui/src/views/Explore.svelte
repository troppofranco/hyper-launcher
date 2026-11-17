<script lang="ts">
  import { api, type SearchHit } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { compact, loaderLabel, relativeIso } from '../lib/format';
  import { KINDS, MOD_CATEGORIES, dirOf, incompatibility, kindLabel } from '../lib/content';
  import Icon from '../components/Icon.svelte';
  import ProjectIcon from '../components/ProjectIcon.svelte';
  import ProgressBar from '../components/ProgressBar.svelte';

  const PAGE = 20;
  let hits = $state<SearchHit[]>([]);
  let total = $state(0);
  let loading = $state(false);
  let failed = $state<string | null>(null);
  let installing = $state<string | null>(null);
  let installed = $state<Set<string>>(new Set());
  let searchSeq = 0;

  const ex = app.explore;
  const target = $derived(app.instances.find((i) => i.name === ex.instance) ?? null);
  const blocked = $derived(incompatibility(ex.kind, target));
  const isPack = $derived(ex.kind === 'modpack');
  const progress = $derived(installing ? app.contentProgress[isPack ? '' : (ex.instance ?? '')] : undefined);

  async function load(offset: number) {
    const seq = ++searchSeq;
    loading = true;
    failed = null;
    try {
      const res = await api.search({
        text: ex.text.trim(),
        kind: ex.kind,
        instance: isPack ? null : ex.instance,
        categories: ex.kind === 'mod' && ex.category ? [ex.category] : [],
        sort: ex.sort,
        offset,
      });
      if (seq !== searchSeq) return; // è arrivata nel frattempo una ricerca più nuova
      hits = offset ? [...hits, ...res.hits] : res.hits;
      total = res.total;
    } catch (e) {
      if (seq === searchSeq) failed = String(e);
    } finally {
      if (seq === searchSeq) loading = false;
    }
  }

  // Nuova ricerca quando cambiano i filtri; il testo aspetta che si smetta di scrivere.
  $effect(() => {
    void [ex.text, ex.kind, ex.instance, ex.category, ex.sort];
    const timer = setTimeout(() => load(0), 300);
    return () => clearTimeout(timer);
  });

  // Progetti già presenti nell'istanza scelta, per mostrare "Installato".
  $effect(() => {
    const dir = dirOf(ex.kind);
    const inst = ex.instance;
    installed = new Set();
    if (!inst || !dir) return;
    api.installed(inst, dir, false)
      .then((items) => (installed = new Set(items.flatMap((i) => (i.projectId ? [i.projectId] : [])))))
      .catch(() => {});
  });

  async function install(hit: SearchHit) {
    installing = hit.id;
    if (isPack) await app.installPack(hit.id);
    else if (await app.installContent(ex.instance!, hit.id, hit.title, ex.kind, null)) installed = new Set([...installed, hit.id]);
    installing = null;
  }

  function pickKind(kind: typeof ex.kind) {
    ex.kind = kind;
    ex.category = null;
  }
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1 class="page-title">Esplora</h1>
      <p class="muted">Mod, resource pack, shader, data pack e modpack da Modrinth.</p>
    </div>
    <button class="btn" onclick={() => app.importPack()} disabled={'' in app.contentProgress}><Icon name="upload" size={16} /> Importa .mrpack</button>
  </div>

  {#if !isPack}
    <div class="context">
      <label for="explore-target" class="muted">Installa in</label>
      <select id="explore-target" class="input" bind:value={ex.instance}>
        <option value={null}>Nessuna istanza (mostra tutto)</option>
        {#each app.recent as inst (inst.name)}
          <option value={inst.name}>{inst.name} · {inst.loader ? inst.loader.name : 'Vanilla'} {inst.version}</option>
        {/each}
      </select>
      {#if target}
        <span class="pill">{target.version}</span>
        <span class="pill">{loaderLabel(target)}</span>
      {:else}
        <span class="muted small">Scegli un’istanza per vedere solo i contenuti compatibili e installarli con un clic.</span>
      {/if}
    </div>
  {:else}
    <p class="note">Un modpack diventa una nuova istanza con la sua versione di Minecraft, Fabric e tutte le mod.</p>
  {/if}

  <div class="searchrow">
    <label class="search">
      <Icon name="search" size={18} />
      <span class="visually-hidden">Cerca su Modrinth</span>
      <input id="explore-search" type="search" placeholder="Cerca {kindLabel(ex.kind).toLowerCase()}…" bind:value={ex.text} />
    </label>
    <label class="sort"><span class="muted">Ordina</span>
      <select id="explore-sort" class="input" bind:value={ex.sort}>
        <option value="relevance">Rilevanza</option>
        <option value="downloads">Download</option>
        <option value="updated">Aggiornati di recente</option>
      </select>
    </label>
  </div>

  <div class="chips" role="tablist" aria-label="Tipo di contenuto">
    {#each KINDS as k (k.kind)}
      <button role="tab" class="chip" class:on={ex.kind === k.kind} aria-selected={ex.kind === k.kind} onclick={() => pickKind(k.kind)}>{k.label}</button>
    {/each}
  </div>

  {#if ex.kind === 'mod'}
    <div class="chips cats" aria-label="Categoria">
      <button class="chip small-chip" class:on={!ex.category} onclick={() => (ex.category = null)}>Tutte</button>
      {#each MOD_CATEGORIES as [id, label] (id)}
        <button class="chip small-chip" class:on={ex.category === id} onclick={() => (ex.category = ex.category === id ? null : id)}>{label}</button>
      {/each}
    </div>
  {/if}

  {#if blocked}<p class="note warn">{blocked}</p>{/if}
  {#if failed}<p class="error-box" role="alert">Modrinth non risponde: {failed}</p>{/if}

  <div class="results">
    {#each hits as hit (hit.id)}
      <div class="card res">
        <button class="open" onclick={() => app.openProject(hit.id, ex.kind)}>
          <ProjectIcon url={hit.iconUrl} name={hit.title} size={56} />
          <span class="meta">
            <span class="title">{hit.title} <span class="by">di {hit.author}</span></span>
            <span class="desc">{hit.description}</span>
            <span class="facts"><span>↓ {compact(hit.downloads)}</span><span>aggiornato {relativeIso(hit.updated).toLowerCase()}</span></span>
          </span>
        </button>
        <span class="action">
          {#if installing === hit.id}
            <span class="progress"><ProgressBar {progress} /></span>
          {:else if installed.has(hit.id)}
            <span class="done"><Icon name="check" size={16} /> Installato</span>
          {:else if isPack}
            <button class="btn primary small" disabled={!!installing} onclick={() => install(hit)}>Installa</button>
          {:else if ex.kind === 'datapack' || !ex.instance}
            <button class="btn small" onclick={() => app.openProject(hit.id, ex.kind)}>Dettagli</button>
          {:else}
            <button class="btn primary small" disabled={!!installing || !!blocked} onclick={() => install(hit)}>Installa</button>
          {/if}
        </span>
      </div>
    {:else}
      {#if !loading && !failed}<p class="muted">Nessun risultato{ex.text.trim() ? ` per “${ex.text.trim()}”` : ''}.</p>{/if}
    {/each}
  </div>

  {#if loading}
    <p class="muted">Ricerca su Modrinth…</p>
  {:else if hits.length < total}
    <button class="btn more" onclick={() => load(hits.length)}>Mostra altri ({total - hits.length})</button>
  {/if}
</div>

<style>
  .context { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; padding: 10px 14px; border-radius: 12px; background: var(--surface); border: 1px solid var(--line); }
  .context select { height: 36px; min-width: 240px; }
  .pill { font-family: var(--mono); font-size: 11px; padding: 3px 8px; border-radius: 6px; background: var(--surface-2); color: var(--text-2); }
  .note { margin: 0; padding: 12px 14px; border-radius: 12px; background: var(--surface); border: 1px solid var(--line); color: var(--text-2); }
  .note.warn { background: color-mix(in srgb, #e5a13a 14%, transparent); border-color: transparent; color: #f3c98a; }
  .searchrow { display: flex; gap: 10px; flex-wrap: wrap; }
  .search { flex: 1 1 320px; display: flex; align-items: center; gap: 10px; height: 46px; padding: 0 14px; border-radius: 10px; border: 1px solid var(--line-strong); background: var(--panel); color: var(--muted); }
  .search:focus-within { border-color: var(--accent-line); }
  .search input { flex: 1; min-width: 0; height: 100%; border: 0; background: transparent; outline: none; }
  .sort { display: flex; align-items: center; gap: 8px; }
  .sort .input { height: 46px; }
  .chips { display: flex; gap: 8px; flex-wrap: wrap; }
  .chip { height: 34px; padding: 0 14px; border-radius: 17px; border: 1px solid var(--line-strong); background: transparent; color: var(--text-2); font-weight: 500; }
  .chip:hover { background: var(--surface-2); }
  .chip.on { background: var(--accent-soft); border-color: var(--accent); color: var(--text); }
  .small-chip { height: 30px; padding: 0 12px; font-size: 12px; }
  .results { display: flex; flex-direction: column; gap: 10px; }
  .res { display: flex; align-items: center; gap: 14px; padding: 14px; }
  .res:hover { border-color: var(--line-strong); }
  .open { flex: 1; min-width: 0; display: flex; align-items: center; gap: 14px; background: none; border: 0; padding: 0; text-align: left; }
  .meta { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
  .title { font-weight: 600; font-size: 15px; }
  .by { font-weight: 400; font-size: 12px; color: var(--faint); }
  .desc { color: var(--muted); font-size: 13px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .facts { display: flex; gap: 14px; font-size: 12px; color: var(--faint); font-variant-numeric: tabular-nums; }
  .action { flex: none; display: flex; justify-content: flex-end; min-width: 120px; }
  .done { display: inline-flex; align-items: center; gap: 6px; font-size: 13px; font-weight: 600; color: #7ee0ad; }
  .progress { width: 140px; }
  .more { align-self: center; }
</style>
