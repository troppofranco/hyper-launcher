<script lang="ts">
  // Plugin di un server Paper: quelli installati (riconosciuti su Modrinth
  // dal loro hash) e la ricerca nel catalogo Modrinth per aggiungerne altri.
  // Ogni modifica vale dal prossimo avvio del server.
  import { api, type SearchHit, type ServerPlugin } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import { t } from '../../lib/i18n.svelte';
  import { compact, megabytes } from '../../lib/format';
  import Icon from '../Icon.svelte';
  import ProjectIcon from '../ProjectIcon.svelte';

  let { name, version, running, onrestart }: {
    name: string;
    version: string;
    running: boolean;
    onrestart: () => void;
  } = $props();

  let installed = $state<ServerPlugin[]>([]);
  let loading = $state(true);
  let hits = $state<SearchHit[]>([]);
  let searching = $state(false);
  let query = $state('');
  let installing = $state<string | null>(null);
  /** Qualcosa è cambiato mentre il server è acceso: serve un riavvio. */
  let pending = $state(false);

  const installedIds = $derived(new Set(installed.map((p) => p.projectId).filter(Boolean)));

  async function load() {
    try {
      installed = await api.serverPlugins(name);
    } catch (e) {
      app.error(e);
    } finally {
      loading = false;
    }
  }

  let seq = 0;
  async function search(text: string) {
    const mine = ++seq;
    searching = true;
    try {
      const res = await api.search({
        text,
        kind: 'plugin',
        instance: null,
        gameVersion: version,
        loaders: ['paper', 'spigot', 'bukkit'],
        categories: [],
        sort: text.trim() ? 'relevance' : 'downloads',
        offset: 0,
      });
      if (mine === seq) hits = res.hits;
    } catch (e) {
      if (mine === seq) app.error(e);
    } finally {
      if (mine === seq) searching = false;
    }
  }

  $effect(() => {
    void name;
    loading = true;
    load();
  });

  // Ricerca mentre si scrive, con una breve pausa.
  $effect(() => {
    const text = query;
    const timer = setTimeout(() => search(text), text ? 300 : 0);
    return () => clearTimeout(timer);
  });

  function changed() {
    if (running) pending = true;
  }

  async function install(hit: SearchHit) {
    installing = hit.id;
    try {
      const files = await api.serverPluginInstall(name, hit.id);
      app.info_(files.length > 1
        ? t('{name} installato con {n} dipendenze.', { name: hit.title, n: files.length - 1 })
        : t('{name} installato.', { name: hit.title }));
      changed();
      await load();
    } catch (e) {
      app.error(e);
    } finally {
      installing = null;
    }
  }

  async function act(op: () => Promise<unknown>) {
    try {
      await op();
      changed();
    } catch (e) {
      app.error(e);
    }
    await load();
  }

  const label = (p: ServerPlugin) => p.title ?? p.file.replace(/\.jar(\.disabled)?$/i, '');
</script>

<div class="wrap">
  {#if pending}
    <div class="banner enter">
      <Icon name="refresh" size={16} />
      <span class="grow">{t('Le modifiche ai plugin valgono dal prossimo riavvio del server.')}</span>
      <button class="btn small primary" onclick={() => { pending = false; onrestart(); }}>{t('Riavvia ora')}</button>
    </div>
  {/if}

  <section class="card">
    <header class="head">
      <h2>{t('Installati')}{#if installed.length}<span class="count">{installed.length}</span>{/if}</h2>
    </header>
    {#if running && installed.length}
      <p class="note">{t('Il server è acceso: fermalo per attivare, disattivare o eliminare i plugin. Puoi comunque installarne di nuovi.')}</p>
    {/if}
    {#if loading && !installed.length}
      <p class="muted pad">{t('Lettura dei plugin e controllo su Modrinth…')}</p>
    {/if}
    {#each installed as p, i (p.file)}
      <div class="row enter" style:--i={i} class:off={!p.enabled}>
        <span class="who">
          <ProjectIcon url={p.iconUrl} name={label(p)} size={40} />
          <span class="meta">
            <span class="name">{label(p)}</span>
            <span class="muted small">{p.file} · {megabytes(p.sizeBytes)}{p.projectId ? '' : t(' · non riconosciuto su Modrinth')}</span>
          </span>
        </span>
        {#if p.versionNumber}<span class="ver">{p.versionNumber}</span>{/if}
        <button class="switch" role="switch" aria-checked={p.enabled} disabled={running}
          aria-label={p.enabled ? t('Disattiva {name}', { name: label(p) }) : t('Attiva {name}', { name: label(p) })}
          onclick={() => act(() => api.serverPluginSetEnabled(name, p.file, !p.enabled))}></button>
        <button class="icon-btn" aria-label={t('Elimina {name}', { name: label(p) })} title={t('Elimina')} disabled={running}
          onclick={() => act(() => api.serverPluginDelete(name, p.file))}><Icon name="trash" size={18} /></button>
      </div>
    {:else}
      {#if !loading}<p class="muted pad">{t('Nessun plugin: cercane uno qui sotto.')}</p>{/if}
    {/each}
  </section>

  <section class="card">
    <header class="head">
      <h2>{t('Aggiungi da Modrinth')}</h2>
      <span class="muted small">{t('Compatibili con Paper {version}', { version })}</span>
    </header>
    <div class="search">
      <input class="input" type="search" placeholder={t('Cerca plugin (es. EssentialsX, LuckPerms)…')} aria-label={t('Cerca plugin')} bind:value={query} />
    </div>
    {#each hits as h, i (h.id)}
      <div class="row enter" style:--i={i}>
        <span class="who">
          <ProjectIcon url={h.iconUrl} name={h.title} size={40} />
          <span class="meta">
            <span class="name">{h.title} <span class="muted small by">{t('di {author}', { author: h.author })}</span></span>
            <span class="muted small desc">{h.description}</span>
          </span>
        </span>
        <span class="dl muted small"><Icon name="download" size={14} /> {compact(h.downloads)}</span>
        {#if installedIds.has(h.id)}
          <span class="done"><Icon name="check" size={14} /> {t('Installato')}</span>
        {:else}
          <button class="btn small primary" disabled={!!installing} onclick={() => install(h)}>
            {installing === h.id ? t('Installazione…') : t('Installa')}
          </button>
        {/if}
      </div>
    {:else}
      <p class="muted pad">{searching ? t('Ricerca su Modrinth…') : t('Nessun plugin trovato per questa versione.')}</p>
    {/each}
  </section>
</div>

<style>
  .wrap { display: flex; flex-direction: column; gap: 14px; }
  .banner { display: flex; align-items: center; gap: 12px; padding: 10px 14px; border-radius: 12px; background: var(--accent-soft); border: 1px solid color-mix(in srgb, var(--accent) 50%, transparent); font-size: 13px; }
  .grow { flex: 1; min-width: 0; }
  .card { overflow: hidden; }
  .head { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; padding: 14px 16px 10px; }
  h2 { margin: 0; font-size: 15px; display: flex; align-items: center; gap: 8px; }
  .count { font-size: 11px; font-weight: 600; padding: 2px 7px; border-radius: 10px; background: var(--surface-2); color: var(--text-2); }
  .search { padding: 0 16px 10px; }
  .search .input { width: 100%; }
  .row { display: flex; align-items: center; gap: 14px; padding: 10px 16px; border-top: 1px solid var(--line); }
  .row.off .who { opacity: .55; }
  .who { flex: 1; min-width: 0; display: flex; align-items: center; gap: 14px; }
  .meta { min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .name { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .by { font-weight: 400; }
  .meta .small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ver { flex: 0 0 auto; max-width: 160px; font-family: var(--mono); font-size: 12px; color: var(--text-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dl { flex: none; display: inline-flex; align-items: center; gap: 4px; }
  .done { flex: none; display: inline-flex; align-items: center; gap: 4px; font-size: 12px; font-weight: 600; color: var(--success-text); padding: 0 6px; }
  .note { margin: 0 16px 12px; padding: 8px 12px; border-radius: 10px; background: var(--surface-2); color: var(--text-2); font-size: 12.5px; }
  .pad { padding: 4px 16px 16px; margin: 0; }
  .icon-btn { width: 36px; height: 36px; border-radius: 8px; border: 0; background: transparent; color: var(--muted); display: flex; align-items: center; justify-content: center; flex: none; }
  .icon-btn:hover:not(:disabled) { color: var(--danger-text); background: var(--surface-2); }
  @media (max-width: 640px) { .dl, .ver { display: none; } }
</style>
