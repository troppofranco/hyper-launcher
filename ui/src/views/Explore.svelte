<script lang="ts">
  import { t } from '../lib/i18n.svelte';
  import { api, type SearchHit, type VersionInfo } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { compact, loaderLabel, relativeIso } from '../lib/format';
  import { KINDS, LOADERS, modCategories, dirOf, incompatibility, kindLabel } from '../lib/content';
  import Icon from '../components/Icon.svelte';
  import ProjectIcon from '../components/ProjectIcon.svelte';
  import ProgressBar from '../components/ProgressBar.svelte';
  import CurseForgeBrowse from '../components/CurseForgeBrowse.svelte';

  let hits = $state<SearchHit[]>([]);
  let total = $state(0);
  let loading = $state(false);
  let failed = $state<string | null>(null);
  let installing = $state<string | null>(null);
  let installed = $state<Set<string>>(new Set());
  let releases = $state<VersionInfo[]>([]);
  let searchSeq = 0;

  const ex = app.explore;
  const isPack = $derived(ex.kind === 'modpack');
  // CurseForge solo se la build ha la chiave; altrimenti sempre Modrinth.
  const onCf = $derived(ex.source === 'curseforge' && !!app.info?.curseforge);
  // Con un'istanza scelta versione e loader sono i suoi; i filtri del
  // pannello valgono solo senza istanza (o per i modpack).
  const target = $derived(isPack ? null : (app.instances.find((i) => i.name === ex.instance) ?? null));
  const blocked = $derived(incompatibility(ex.kind, target));
  const targetKey = $derived(target ? `${target.name}|${target.version}|${target.loader?.kind ?? ''}` : '');
  const loaderFilter = $derived(ex.kind === 'mod' || ex.kind === 'modpack');
  const progress = $derived(installing ? app.contentProgress[isPack ? '' : (ex.instance ?? '')] : undefined);

  $effect(() => {
    app.loadVersions().then((v) => (releases = v.versions.filter((x) => x.kind === 'release'))).catch(() => {});
  });

  async function load(offset: number) {
    const seq = ++searchSeq;
    loading = true;
    failed = null;
    try {
      const res = await api.search({
        text: ex.text.trim(),
        kind: ex.kind,
        instance: target?.name ?? null,
        gameVersion: target ? null : ex.gameVersion,
        loaders: target || !loaderFilter ? [] : ex.loaders,
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
    // Dell'istanza contano solo nome, versione e loader: l'oggetto cambia a
    // ogni aggiornamento dell'elenco istanze e ripartirebbe la ricerca.
    void [ex.text.trim(), ex.kind, targetKey, ex.category, ex.sort, ex.gameVersion, ex.loaders.join()];
    if (onCf) return;
    const timer = setTimeout(() => load(0), 300);
    return () => clearTimeout(timer);
  });

  // Progetti già presenti nell'istanza scelta, per mostrare "Installato".
  $effect(() => {
    const dir = dirOf(ex.kind);
    const inst = targetKey ? ex.instance : null;
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

  function toggleLoader(id: string) {
    ex.loaders = ex.loaders.includes(id) ? ex.loaders.filter((l) => l !== id) : [...ex.loaders, id];
  }

  /** Colore di riserva per la copertina (dal colore dell'icona). */
  function cover(hit: SearchHit): string {
    const hex = `#${(hit.color ?? 0x2a2f3a).toString(16).padStart(6, '0')}`;
    return `linear-gradient(135deg, color-mix(in srgb, ${hex} 55%, var(--panel)), var(--panel))`;
  }
</script>

<div class="explore">
  <div class="main">
    <div class="page-head">
      <div>
        <h1 class="page-title">{t('Esplora')}</h1>
        <p class="muted">{onCf ? t('Modpack, mod e risorse da CurseForge, installati con le loro dipendenze.') : t('Modpack, mod e risorse da Modrinth, installati con le loro dipendenze.')}</p>
      </div>
      <span class="head-actions">
        {#if app.info?.curseforge}
          <div class="seg source" role="group" aria-label={t('Catalogo')}>
            <button class:on={!onCf} aria-pressed={!onCf} onclick={() => (ex.source = 'modrinth')}>Modrinth</button>
            <button class:on={onCf} aria-pressed={onCf} onclick={() => (ex.source = 'curseforge')}>CurseForge</button>
          </div>
        {/if}
        <button class="btn" onclick={() => app.importPack()} disabled={'' in app.contentProgress}><Icon name="upload" size={16} /> {t('Importa modpack')}</button>
      </span>
    </div>

    {#if !isPack}
      <div class="context">
        <label for="explore-target" class="muted">{t('Installa in')}</label>
        <select id="explore-target" class="input" bind:value={ex.instance}>
          <option value={null}>{t('Nessuna istanza (mostra tutto)')}</option>
          {#each app.recent.filter((i) => i.edition !== 'bedrock') as inst (inst.name)}
            <option value={inst.name}>{inst.name} · {loaderLabel(inst)} {inst.version}</option>
          {/each}
        </select>
        {#if target}
          <span class="pill">{target.version}</span>
          <span class="pill">{loaderLabel(target)}</span>
          <span class="muted small">{t('Vedi solo ciò che funziona in questa istanza.')}</span>
        {:else}
          <span class="muted small">{t('Scegli un’istanza per vedere solo i contenuti compatibili e installarli con un clic.')}</span>
        {/if}
      </div>
    {/if}

    {#if onCf}
      <CurseForgeBrowse kind={ex.kind} {target} />
    {:else}
    <label class="search">
      <Icon name="search" size={18} />
      <span class="visually-hidden">{t('Cerca su Modrinth')}</span>
      <input id="explore-search" type="search" placeholder={t('Cerca {tipo}…', { tipo: kindLabel(ex.kind).toLowerCase() })} bind:value={ex.text} />
    </label>

    <div class="bar-row">
      <h2 class="count">{kindLabel(ex.kind)} <span class="muted small">{loading && !hits.length ? t('ricerca…') : t('{n} risultati', { n: compact(total) })}</span></h2>
      <div class="tools">
        <label><span class="visually-hidden">{t('Ordina')}</span>
          <select id="explore-sort" class="input sort" bind:value={ex.sort}>
            <option value="relevance">{t('Rilevanza')}</option>
            <option value="downloads">{t('Download')}</option>
            <option value="updated">{t('Aggiornati di recente')}</option>
          </select>
        </label>
        <div class="seg" role="group" aria-label={t('Vista')}>
          <button class:on={ex.layout === 'grid'} aria-pressed={ex.layout === 'grid'} aria-label={t('Griglia')} onclick={() => (ex.layout = 'grid')}><Icon name="library" size={16} /></button>
          <button class:on={ex.layout === 'list'} aria-pressed={ex.layout === 'list'} aria-label={t('Elenco')} onclick={() => (ex.layout = 'list')}><Icon name="list" size={16} /></button>
        </div>
      </div>
    </div>

    {#if blocked}<p class="note warn">{blocked}</p>{/if}
    {#if failed}<p class="error-box" role="alert">{t('Modrinth non risponde: {errore}', { errore: failed })}</p>{/if}

    <div class="results" class:grid={ex.layout === 'grid'}>
      {#each hits as hit, i (hit.id)}
        <article class="card res enter" style:--i={i % 20}>
          {#if ex.layout === 'grid'}
            <button class="cover" style:background={cover(hit)} aria-label={t('Apri {titolo}', { titolo: hit.title })} onclick={() => app.openProject(hit.id, ex.kind)}>
              {#if hit.image}<img src={hit.image} alt="" loading="lazy" />{/if}
              <span class="cover-icon"><ProjectIcon url={hit.iconUrl} name={hit.title} size={52} /></span>
            </button>
          {/if}
          <div class="body">
            <button class="open" onclick={() => app.openProject(hit.id, ex.kind)}>
              {#if ex.layout === 'list'}<ProjectIcon url={hit.iconUrl} name={hit.title} size={56} />{/if}
              <span class="meta">
                <span class="title-row"><span class="title">{hit.title}</span><span class="dl">↓ {compact(hit.downloads)}</span></span>
                <span class="desc">{hit.description}</span>
              </span>
            </button>
            <div class="foot">
              <span class="by">{t('di {autore}', { autore: hit.author })} · {relativeIso(hit.updated).toLowerCase()}</span>
              <span class="action">
                {#if installing === hit.id}
                  <ProgressBar {progress} label={false} />
                {:else if installed.has(hit.id)}
                  <span class="done"><Icon name="check" size={16} /> {t('Installato')}</span>
                {:else if isPack}
                  <button class="btn primary small" disabled={!!installing} onclick={() => install(hit)}><Icon name="download" size={15} /> {t('Installa')}</button>
                {:else if ex.kind === 'datapack' || !target}
                  <button class="btn small" onclick={() => app.openProject(hit.id, ex.kind)}>{t('Dettagli')}</button>
                {:else}
                  <button class="btn primary small" disabled={!!installing || !!blocked} onclick={() => install(hit)}><Icon name="download" size={15} /> {t('Installa')}</button>
                {/if}
              </span>
            </div>
          </div>
        </article>
      {:else}
        {#if !loading && !failed}<p class="muted">{ex.text.trim() ? t('Nessun risultato per “{testo}”.', { testo: ex.text.trim() }) : t('Nessun risultato.')}</p>{/if}
      {/each}
    </div>

    {#if loading && hits.length}
      <p class="muted center">{t('Ricerca su Modrinth…')}</p>
    {:else if hits.length < total}
      <button class="btn more" onclick={() => load(hits.length)}>{t('Mostra altri')}</button>
    {/if}
    {/if}
  </div>

  <aside class="filters" aria-label={t('Filtri')}>
    <h2 class="filters-title"><Icon name="filter" size={16} /> {t('Filtri')}</h2>

    <section>
      <h3 class="label">{t('Tipo di contenuto')}</h3>
      <div class="chips">
        {#each KINDS as k (k.kind)}
          <button class="chip" class:on={ex.kind === k.kind} aria-pressed={ex.kind === k.kind} onclick={() => pickKind(k.kind)}>{k.label}</button>
        {/each}
      </div>
    </section>

    {#if !onCf}
    <section>
      <h3 class="label">{t('Versione di gioco')}</h3>
      {#if target}
        <p class="small muted">{t('{versione} · dall’istanza “{nome}”', { versione: target.version, nome: target.name })}</p>
      {:else}
        <select id="explore-version" class="input" bind:value={ex.gameVersion}>
          <option value={null}>{t('Qualsiasi versione')}</option>
          {#each releases as v (v.id)}<option value={v.id}>{v.id}</option>{/each}
        </select>
      {/if}
    </section>

    {#if loaderFilter}
      <section>
        <h3 class="label">Mod loader</h3>
        {#if target}
          <p class="small muted">{t('{loader} · dall’istanza', { loader: loaderLabel(target) })}</p>
        {:else}
          <div class="chips">
            {#each LOADERS as [id, label] (id)}
              <button class="chip" class:on={ex.loaders.includes(id)} aria-pressed={ex.loaders.includes(id)} onclick={() => toggleLoader(id)}>{label}</button>
            {/each}
          </div>
        {/if}
      </section>
    {/if}

    {#if ex.kind === 'mod'}
      <section>
        <h3 class="label">{t('Categorie')}</h3>
        <div class="chips">
          {#each modCategories() as [id, label] (id)}
            <button class="chip" class:on={ex.category === id} aria-pressed={ex.category === id} onclick={() => (ex.category = ex.category === id ? null : id)}>{label}</button>
          {/each}
        </div>
      </section>
    {/if}
    {/if}
  </aside>
</div>

<style>
  .explore { display: flex; align-items: flex-start; min-height: 100%; }
  .main { flex: 1; min-width: 0; padding: 28px 32px 40px; display: flex; flex-direction: column; gap: 18px; }
  .context { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; padding: 10px 14px; border-radius: 12px; background: var(--surface); border: 1px solid var(--line); }
  .context select { height: 36px; min-width: 240px; }
  .pill { font-family: var(--mono); font-size: 11px; padding: 3px 8px; border-radius: 6px; background: var(--surface-2); color: var(--text-2); }
  .note { margin: 0; padding: 12px 14px; border-radius: 12px; }
  .note.warn { background: color-mix(in srgb, #e5a13a 14%, transparent); color: var(--warn-text); }
  .search { display: flex; align-items: center; gap: 10px; height: 50px; padding: 0 16px; border-radius: 12px; border: 1px solid var(--line-strong); background: var(--panel); color: var(--muted); }
  .search:focus-within { border-color: var(--accent-line); }
  .search input { flex: 1; min-width: 0; height: 100%; border: 0; background: transparent; outline: none; font-size: 15px; }
  .bar-row { display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap; }
  .count { font-size: 16px; font-weight: 600; }
  .tools { display: flex; gap: 8px; align-items: center; }
  .sort { height: 38px; }
  .seg { display: flex; border: 1px solid var(--line-strong); border-radius: 10px; overflow: hidden; }
  .head-actions { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
  .seg.source button { width: auto; padding: 0 14px; height: 42px; font-weight: 600; }
  .seg button { width: 40px; height: 36px; border: 0; background: transparent; color: var(--muted); display: flex; align-items: center; justify-content: center; }
  .seg button.on { background: var(--accent-soft); color: var(--text); }
  .results { display: flex; flex-direction: column; gap: 10px; }
  .results.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)); gap: 14px; }
  .res { display: flex; flex-direction: column; overflow: hidden; }
  .res:hover { border-color: var(--line-strong); }
  .cover { position: relative; height: 128px; border: 0; padding: 0; display: block; width: 100%; }
  .cover img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .cover-icon { position: absolute; left: 14px; bottom: -22px; border-radius: 14px; padding: 3px; background: var(--surface); display: flex; }
  .body { padding: 14px; display: flex; flex-direction: column; gap: 10px; flex: 1; }
  .grid .body { padding-top: 30px; }
  .open { display: flex; align-items: center; gap: 14px; background: none; border: 0; padding: 0; text-align: left; min-width: 0; }
  .meta { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
  .title-row { display: flex; justify-content: space-between; gap: 10px; align-items: baseline; }
  .title { font-weight: 600; font-size: 15px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dl { flex: none; font-size: 12px; color: var(--faint); font-variant-numeric: tabular-nums; }
  .desc { color: var(--muted); font-size: 13px; line-height: 1.45; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .foot { margin-top: auto; display: flex; justify-content: space-between; align-items: center; gap: 10px; }
  .by { font-size: 12px; color: var(--faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .action { flex: none; display: flex; justify-content: flex-end; min-width: 110px; }
  .done { display: inline-flex; align-items: center; gap: 6px; font-size: 13px; font-weight: 600; color: var(--success-text); }
  .more { align-self: center; }
  .center { text-align: center; }
  .filters { flex: 0 0 280px; align-self: stretch; margin: 12px 12px 12px 0; border: 1px solid var(--line); border-radius: var(--radius); background: var(--panel); padding: 22px 18px; display: flex; flex-direction: column; gap: 22px; }
  .filters-title { display: flex; align-items: center; gap: 8px; font-size: 15px; font-weight: 600; }
  .filters section { display: flex; flex-direction: column; gap: 10px; }
  .filters h3, .filters p { margin: 0; }
  .filters select { height: 40px; width: 100%; }
  .chips { display: flex; gap: 8px; flex-wrap: wrap; }
  .chip { height: 34px; padding: 0 13px; border-radius: 17px; border: 1px solid var(--line-strong); background: transparent; color: var(--text-2); font-weight: 500; font-size: 13px; }
  .chip:hover { background: var(--surface-2); }
  .chip.on { background: var(--accent-soft); border-color: var(--accent); color: var(--text); }
  @media (max-width: 1100px) {
    .explore { flex-direction: column-reverse; }
    .filters { align-self: auto; margin: 12px 12px 0; }
  }
</style>
