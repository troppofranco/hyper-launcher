<script lang="ts">
  import { t } from '../../lib/i18n.svelte';
  import { app } from '../../lib/store.svelte';
  import { playtime } from '../../lib/format';

  const total = $derived(app.instances.reduce((sum, i) => sum + i.playtimeSecs, 0));
  const top = $derived([...app.instances].sort((a, b) => b.playtimeSecs - a.playtimeSecs)[0] ?? null);
</script>

<div class="card stats">
  <div class="nums">
    <div><span class="num">{app.instances.length}</span><span class="muted small">{t('istanze')}</span></div>
    <div><span class="num">{playtime(total)}</span><span class="muted small">{t('giocate')}</span></div>
  </div>
  {#if top?.playtimeSecs}
    <button class="top" onclick={() => top && app.go({ view: 'instance', name: top.name })}>
      <span class="muted small">{t('La più giocata')}</span>
      <span class="name">{top.name}</span>
    </button>
  {/if}
</div>

<style>
  .stats { padding: 16px; display: flex; flex-direction: column; gap: 14px; }
  .nums { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
  .nums > div { display: flex; flex-direction: column; gap: 2px; }
  .num { font-family: var(--display); font-size: 26px; font-weight: 600; font-variant-numeric: tabular-nums; }
  .top { display: flex; flex-direction: column; gap: 2px; padding: 10px 12px; border-radius: var(--radius-sm); border: 0; background: var(--panel); text-align: left; color: var(--text); }
  .top:hover { background: var(--surface-2); }
  .name { font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
