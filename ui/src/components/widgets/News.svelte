<script lang="ts">
  import { t } from '../../lib/i18n.svelte';
  import { api } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import { isoDate } from '../../lib/format';
  import Icon from '../Icon.svelte';

  let { count = 1 }: { count?: number } = $props();
  const news = $derived(app.news.slice(0, count));
</script>

{#if news.length}
  <div class="list" class:many={news.length > 1}>
    {#each news as item, i (item.url)}
      <button class="card news enter" style:--i={i} onclick={() => api.openUrl(item.url)}>
        <span class="news-img">{#if item.image}<img src={item.image} alt="" />{/if}</span>
        <span class="news-body">
          <span class="news-meta"><span class="tag">{t('NOTIZIA')}</span><span class="muted small">{isoDate(item.date)}</span></span>
          <span class="news-title">{item.title}</span>
          {#if item.text && news.length === 1}<span class="muted">{item.text}</span>{/if}
          <span class="read">{t('Leggi di più')} <Icon name="external" size={14} /></span>
        </span>
      </button>
    {/each}
  </div>
{:else}
  <p class="muted card empty">{t('Notizie non disponibili offline.')}</p>
{/if}

<style>
  .list { display: grid; gap: 14px; }
  .list.many { grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); }
  .news { padding: 0; display: grid; grid-template-columns: minmax(0, 320px) 1fr; overflow: hidden; text-align: left; color: var(--text); width: 100%; }
  .many .news { grid-template-columns: 1fr; }
  .news-img { background: var(--bg-deep); min-height: 150px; display: block; overflow: hidden; }
  .many .news-img { min-height: 110px; height: 110px; }
  .news-img img { width: 100%; height: 100%; object-fit: cover; display: block; transition: transform .5s var(--ease); }
  .news:hover .news-img img { transform: scale(1.05); }
  .news-body { padding: 18px 20px; display: flex; flex-direction: column; gap: 8px; justify-content: center; }
  .news-meta { display: flex; align-items: center; gap: 10px; }
  .tag { font-size: 10px; font-weight: 700; letter-spacing: .06em; padding: 4px 9px; border-radius: 12px; background: var(--accent-soft); color: var(--accent-line); }
  .news-title { font-family: var(--display); font-size: 20px; font-weight: 600; }
  .many .news-title { font-size: 15px; }
  .read { display: inline-flex; align-items: center; gap: 6px; font-weight: 600; margin-top: 4px; }
  .empty { margin: 0; padding: 18px; }
  @container widget (max-width: 560px) {
    .news { grid-template-columns: 1fr; }
  }
</style>
