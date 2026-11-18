<script lang="ts">
  import { t } from '../../lib/i18n.svelte';
  import { api, type Skin } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import SkinViewer from '../SkinViewer.svelte';
  import Icon from '../Icon.svelte';

  // La skin scelta per l'account attivo (si ricarica se cambia account o skin).
  let skin = $state<Skin | null>(null);
  $effect(() => {
    void app.activeAccount?.skin;
    api.listSkins()
      .then((s) => (skin = s.skins.find((x) => x.id === s.selected) ?? null))
      .catch(() => (skin = null));
  });
</script>

<button class="card skin" onclick={() => app.go({ view: 'customize' })} aria-label={t('Apri Personalizzazione')}>
  {#if skin}
    <span class="stage"><SkinViewer skin={skin.image} slim={skin.slim} width={180} height={220} /></span>
    <span class="name">{skin.name}</span>
    <span class="muted small">{t('Cambia skin')}</span>
  {:else}
    <span class="stage empty"><Icon name="shirt" size={40} /></span>
    <span class="name">{t('Nessuna skin scelta')}</span>
    <span class="muted small">{t('Scegline una in Personalizzazione')}</span>
  {/if}
</button>

<style>
  .skin { width: 100%; padding: 14px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 4px; color: var(--text); }
  .stage { display: flex; justify-content: center; border-radius: var(--radius-sm); background: radial-gradient(circle at 50% 60%, color-mix(in srgb, var(--accent) 25%, transparent), transparent 70%); width: 100%; }
  .empty { height: 160px; align-items: center; color: var(--muted); }
  .name { font-weight: 600; }
</style>
