<script lang="ts">
  import { api } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { isoDate } from '../lib/format';
</script>

<aside class="news scroll" aria-label="Notizie Minecraft">
  <h2 class="label">Notizie Minecraft</h2>
  {#each app.news as n (n.url + n.title)}
    <button class="item" onclick={() => api.openUrl(n.url)}>
      {#if n.image}
        <img src={n.image} alt="" loading="lazy" />
      {:else}
        <div class="ph" aria-hidden="true"></div>
      {/if}
      <span class="text">
        <span class="title">{n.title}</span>
        <span class="date">{isoDate(n.date)}</span>
      </span>
    </button>
  {:else}
    <p class="muted">Notizie non disponibili offline.</p>
  {/each}
</aside>

<style>
  .news { flex: 0 0 300px; background: var(--panel); border-left: 1px solid #1f232c; padding: 20px 16px; display: flex; flex-direction: column; gap: 6px; }
  h2 { margin: 0 4px 8px; font-weight: 600; }
  .item { display: flex; gap: 12px; align-items: center; text-align: left; padding: 8px; border-radius: 10px; border: 0; background: transparent; }
  .item:hover { background: var(--surface-2); }
  img, .ph { flex: 0 0 76px; width: 76px; height: 54px; border-radius: 8px; object-fit: cover; background: var(--surface-2); }
  .text { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .title { font-weight: 600; font-size: 13px; line-height: 1.3; }
  .date { font-size: 11px; color: var(--muted); }
  p { margin: 4px; }
</style>
