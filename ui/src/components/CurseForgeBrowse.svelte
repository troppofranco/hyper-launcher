<script lang="ts">
  // Catalogo CurseForge in Esplora: progetti in evidenza, popolari e
  // aggiornati di recente, più l'aggiunta per Project ID. La ricerca
  // arriverà quando CurseForge abiliterà la chiave del launcher.
  import { t } from '../lib/i18n.svelte';
  import { api, type CfFeatured, type CfProject, type ContentKind, type Instance } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { compact, relativeIso } from '../lib/format';
  import { incompatibility, kindLabel } from '../lib/content';
  import Icon from './Icon.svelte';
  import ProjectIcon from './ProjectIcon.svelte';
  import ProgressBar from './ProgressBar.svelte';

  let { kind, target }: { kind: ContentKind; target: Instance | null } = $props();

  let lists = $state<CfFeatured | null>(null);
  let failed = $state<string | null>(null);
  let installing = $state<number | null>(null);
  let done = $state<Set<number>>(new Set());
  let idText = $state('');
  let byId = $state<CfProject | null>(null);
  let lookingUp = $state(false);

  const isPack = $derived(kind === 'modpack');
  const blocked = $derived(isPack ? null : incompatibility(kind, target));
  const progress = $derived(installing ? app.contentProgress[isPack ? '' : (target?.name ?? '')] : undefined);
  const targetKey = $derived(target ? `${target.name}|${target.version}|${target.loader?.kind ?? ''}` : '');

  // Le tre liste, senza ripetere un progetto già mostrato in una precedente.
  const sections = $derived.by(() => {
    if (!lists) return [];
    const seen = new Set<number>();
    const unique = (items: CfProject[]) => items.filter((p) => !seen.has(p.id) && seen.add(p.id));
    return [
      [t('In evidenza'), unique(lists.featured)],
      [t('Popolari'), unique(lists.popular)],
      [t('Aggiornati di recente'), unique(lists.recent)],
    ].filter(([, items]) => items.length) as [string, CfProject[]][];
  });

  $effect(() => {
    void targetKey;
    const instance = isPack ? null : (target?.name ?? null);
    lists = null;
    failed = null;
    let live = true;
    api.cfFeatured(kind, instance)
      .then((l) => live && (lists = l))
      .catch((e) => live && (failed = String(e)));
    return () => (live = false);
  });

  async function lookup(e: SubmitEvent) {
    e.preventDefault();
    const id = Number(idText.trim());
    if (!Number.isInteger(id) || id <= 0) return;
    lookingUp = true;
    try {
      byId = await api.cfProject(id);
    } catch (err) {
      byId = null;
      app.error(err);
    } finally {
      lookingUp = false;
    }
  }

  async function install(p: CfProject) {
    installing = p.id;
    const ok = p.kind === 'modpack' ? await app.installCfPack(p.id) : await app.installCf(target!.name, p.id, p.name);
    if (ok && p.kind !== 'modpack') done = new Set([...done, p.id]);
    installing = null;
  }

  /** Si installa da qui: modpack sempre, il resto con un'istanza compatibile. */
  function installable(p: CfProject): boolean {
    if (p.kind === 'modpack') return true;
    return !!target && !!p.kind && p.kind !== 'datapack' && !incompatibility(p.kind, target);
  }
</script>

{#snippet card(p: CfProject, i: number)}
  <article class="card res enter" style:--i={i % 20}>
    <div class="top">
      <ProjectIcon url={p.iconUrl} name={p.name} size={52} />
      <span class="meta">
        <span class="title-row"><span class="title">{p.name}</span><span class="dl">↓ {compact(p.downloads)}</span></span>
        <span class="desc">{p.summary}</span>
      </span>
    </div>
    <div class="foot">
      <span class="by">{t('di {autore}', { autore: p.author })}{p.updated ? ` · ${relativeIso(p.updated).toLowerCase()}` : ''}</span>
      <span class="action">
        <button class="btn small ghost" aria-label={t('Apri {titolo} su CurseForge', { titolo: p.name })} title={t('Apri su CurseForge')} onclick={() => api.openUrl(p.website)}><Icon name="external" size={15} /></button>
        {#if installing === p.id}
          <ProgressBar {progress} label={false} />
        {:else if done.has(p.id)}
          <span class="done"><Icon name="check" size={16} /> {t('Installato')}</span>
        {:else if installable(p)}
          <button class="btn primary small" disabled={!!installing} onclick={() => install(p)}><Icon name="download" size={15} /> {t('Installa')}</button>
        {/if}
      </span>
    </div>
  </article>
{/snippet}

<div class="cf">
  <p class="note">{t('Su CurseForge per ora trovi i progetti in evidenza e puoi aggiungere qualsiasi progetto con il suo Project ID (il numero nella colonna “About Project” della pagina). La ricerca arriverà presto.')}</p>

  <form class="by-id" onsubmit={lookup}>
    <label class="field grow">{t('Aggiungi per Project ID')}
      <input class="input" inputmode="numeric" bind:value={idText} placeholder="394468" autocomplete="off" />
    </label>
    <button class="btn" type="submit" disabled={lookingUp || !/^\d+$/.test(idText.trim())}><Icon name="search" size={16} /> {lookingUp ? t('Ricerca…') : t('Trova')}</button>
  </form>
  {#if byId}
    <div class="results single">{@render card(byId, 0)}</div>
  {/if}

  {#if blocked}<p class="note warn">{blocked}</p>{/if}
  {#if failed}<p class="error-box" role="alert">{t('CurseForge non risponde: {errore}', { errore: failed })}</p>{/if}

  {#if !lists && !failed}
    <div class="results"><div class="card skeleton"></div><div class="card skeleton"></div><div class="card skeleton"></div></div>
  {:else}
    {#each sections as [title, items] (title)}
      <section class="list">
        <h2 class="count">{title}</h2>
        <div class="results">{#each items as p, i (p.id)}{@render card(p, i)}{/each}</div>
      </section>
    {:else}
      {#if lists}<p class="muted">{t('Nessun {tipo} in evidenza per questa istanza.', { tipo: kindLabel(kind).toLowerCase() })}</p>{/if}
    {/each}
  {/if}
</div>

<style>
  .cf { display: flex; flex-direction: column; gap: 18px; }
  .note { margin: 0; padding: 12px 14px; border-radius: 12px; background: var(--surface); border: 1px solid var(--line); color: var(--text-2); line-height: 1.5; }
  .note.warn { background: color-mix(in srgb, #e5a13a 14%, transparent); border-color: transparent; color: var(--warn-text); }
  .by-id { display: flex; gap: 10px; align-items: flex-end; flex-wrap: wrap; }
  .grow { flex: 1 1 260px; }
  .list { display: flex; flex-direction: column; gap: 12px; }
  .count { font-size: 16px; font-weight: 600; margin: 0; }
  .results { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 14px; }
  .skeleton { height: 150px; }
  .res { display: flex; flex-direction: column; gap: 12px; padding: 14px; }
  .res:hover { border-color: var(--line-strong); }
  .top { display: flex; gap: 14px; align-items: flex-start; min-width: 0; }
  .meta { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
  .title-row { display: flex; justify-content: space-between; gap: 10px; align-items: baseline; }
  .title { font-weight: 600; font-size: 15px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dl { flex: none; font-size: 12px; color: var(--faint); font-variant-numeric: tabular-nums; }
  .desc { color: var(--muted); font-size: 13px; line-height: 1.45; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .foot { margin-top: auto; display: flex; justify-content: space-between; align-items: center; gap: 10px; }
  .by { font-size: 12px; color: var(--faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .action { flex: none; display: flex; gap: 6px; align-items: center; justify-content: flex-end; min-width: 110px; }
  .ghost { width: 36px; padding: 0; color: var(--muted); }
  .done { display: inline-flex; align-items: center; gap: 6px; font-size: 13px; font-weight: 600; color: var(--success-text); }
</style>
