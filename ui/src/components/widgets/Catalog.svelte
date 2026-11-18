<script lang="ts">
  import { t } from '../../lib/i18n.svelte';
  import { api, type SearchHit } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import { compact } from '../../lib/format';
  import Icon from '../Icon.svelte';
  import ProjectIcon from '../ProjectIcon.svelte';

  let { count = 3 }: { count?: number } = $props();

  // I modpack più scaricati (fino a 6): una volta per sessione, poi dalla memoria.
  $effect(() => {
    if (app.featuredPacks) return;
    api.search({ text: '', kind: 'modpack', instance: null, gameVersion: null, loaders: [], categories: [], sort: 'downloads', offset: 0 })
      .then((r) => (app.featuredPacks = r.hits.slice(0, 6)))
      .catch(() => (app.featuredPacks = []));
  });
  const shown = $derived(app.featuredPacks?.slice(0, count) ?? null);

  function cover(hit: SearchHit): string {
    const hex = `#${(hit.color ?? 0x2a2f3a).toString(16).padStart(6, '0')}`;
    return `linear-gradient(135deg, color-mix(in srgb, ${hex} 55%, var(--panel)), var(--panel))`;
  }

  function openPack(hit: SearchHit) {
    app.explore.kind = 'modpack';
    app.openProject(hit.id, 'modpack');
  }
</script>

<div class="packs">
  {#if shown === null}
    {#each Array.from({ length: count }, (_, n) => n) as n (n)}<div class="card pack skeleton"></div>{/each}
  {:else}
    {#each shown as hit, i (hit.id)}
      <button class="card pack enter" style:--i={i} onclick={() => openPack(hit)}>
        <span class="pack-cover" style:background={cover(hit)}>
          {#if hit.image}<img src={hit.image} alt="" loading="lazy" />{/if}
          <span class="pack-icon"><ProjectIcon url={hit.iconUrl} name={hit.title} size={46} /></span>
        </span>
        <span class="pack-body">
          <span class="pack-title">{hit.title}</span>
          <span class="pack-desc">{hit.description}</span>
          <span class="pack-foot"><span>{t('di {autore}', { autore: hit.author })}</span><span><Icon name="download" size={12} /> {compact(hit.downloads)}</span></span>
        </span>
      </button>
    {/each}
  {/if}
</div>
<button class="link" onclick={() => app.browse(null, 'modpack')}>{t('Vedi tutti')} <Icon name="back" size={14} /></button>

<style>
  .packs { display: grid; grid-template-columns: repeat(auto-fill, minmax(210px, 1fr)); gap: 14px; }
  .pack { padding: 0; overflow: hidden; display: flex; flex-direction: column; text-align: left; color: var(--text); }
  .pack.skeleton { height: 230px; }
  .pack-cover { position: relative; height: 120px; display: block; }
  .pack-cover img { width: 100%; height: 100%; object-fit: cover; display: block; transition: transform .5s var(--ease); }
  .pack:hover .pack-cover img { transform: scale(1.06); }
  .pack-icon { position: absolute; left: 12px; bottom: -20px; border-radius: 13px; padding: 3px; background: var(--c-surface); display: flex; }
  .pack-body { padding: 26px 14px 14px; display: flex; flex-direction: column; gap: 6px; }
  .pack-title { font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .pack-desc { color: var(--muted); font-size: 13px; line-height: 1.45; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .pack-foot { display: flex; justify-content: space-between; font-size: 12px; color: var(--faint); margin-top: 4px; }
  .pack-foot span { display: inline-flex; align-items: center; gap: 4px; }
  .link { align-self: flex-end; display: inline-flex; align-items: center; gap: 4px; background: none; border: 0; color: var(--accent-line); font-size: 13px; padding: 0; margin-top: 10px; }
  .link :global(svg) { transform: rotate(180deg); }
</style>
