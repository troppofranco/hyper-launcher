<script lang="ts">
  import { t } from '../../lib/i18n.svelte';
  import { app } from '../../lib/store.svelte';
  import { loaderLabel, playtime, relativeDay } from '../../lib/format';
  import InstanceIcon from '../InstanceIcon.svelte';
  import PlayButton from '../PlayButton.svelte';

  // L'ultima istanza avviata (o la più recente creata).
  const last = $derived(app.recent[0] ?? null);
</script>

{#if last}
  <div class="card last">
    <button class="who" onclick={() => app.go({ view: 'instance', name: last.name })}>
      <InstanceIcon name={last.name} icon={last.icon} size={52} />
      <span class="meta">
        <span class="name">{last.name}</span>
        <span class="muted small">{loaderLabel(last)} · {last.version}</span>
        <span class="muted small">{relativeDay(last.lastPlayed)}{last.playtimeSecs ? ` · ${playtime(last.playtimeSecs)}` : ''}</span>
      </span>
    </button>
    <PlayButton name={last.name} />
  </div>
{:else}
  <div class="card last empty">
    <p class="muted">{t('Ancora nessuna istanza.')}</p>
    <button class="btn primary small" onclick={() => (app.newInstanceOpen = true)}>{t('Crea la prima istanza')}</button>
  </div>
{/if}

<style>
  /* Nella tessera più alta: il pulsante Gioca resta in fondo. */
  .last { padding: 16px; display: flex; flex-direction: column; justify-content: space-between; gap: 14px; }
  .who { display: flex; align-items: center; gap: 14px; background: none; border: 0; padding: 0; text-align: left; color: var(--text); min-width: 0; }
  .meta { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .name { font-weight: 600; font-size: 16px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .empty p { margin: 0; }
</style>
