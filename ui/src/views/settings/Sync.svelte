<script lang="ts">
  import { t } from '../../lib/i18n.svelte';
  import { settings } from './draft.svelte';
  import Toggle from '../../components/controls/Toggle.svelte';

  const d = $derived(settings.draft!);
  const GROUPS: ['keybinds' | 'video' | 'audio' | 'controls', string, string][] = [
    ['keybinds', 'Tasti', 'Tutti i comandi assegnati, comprese le mod che usano i tasti di Minecraft.'],
    ['video', 'Video', 'Distanza di rendering, FOV, luminosità, scala dell’interfaccia, FPS, nuvole e particelle.'],
    ['audio', 'Audio', 'Volumi per categoria, audio direzionale e sottotitoli.'],
    ['controls', 'Controlli', 'Sensibilità del mouse, assi invertiti, salto automatico, accovacciarsi e scattare con un tasto.'],
  ];
</script>

<p class="muted enter" style:--i={0}>{t('Cambi un tasto o la distanza di rendering in un’istanza e lo ritrovi in tutte le altre. Le impostazioni scelte qui vengono copiate in ogni istanza prima di avviarla e salvate quando chiudi il gioco. Lingua, server recenti e resource pack restano di ogni istanza.')}</p>
<div class="enter" style:--i={1}><Toggle bind:checked={d.sync.enabled} label="Condividi le impostazioni tra le istanze" hint="Si può escludere una singola istanza dalle sue impostazioni." /></div>
<div class="groups" class:off={!d.sync.enabled}>
  {#each GROUPS as [key, label, hint], i (key)}
    <label class="group enter" style:--i={i + 2}>
      <input type="checkbox" bind:checked={d.sync[key]} disabled={!d.sync.enabled} />
      <span><span class="strong">{t(label)}</span><span class="muted small">{t(hint)}</span></span>
    </label>
  {/each}
</div>

<style>
  p { margin: 0; line-height: 1.5; }
  .strong { font-weight: 600; }
  .groups { display: flex; flex-direction: column; gap: 6px; transition: opacity .2s ease; }
  .groups.off { opacity: .5; }
  .group { display: flex; gap: 12px; align-items: flex-start; padding: 10px 12px; border-radius: var(--radius-sm); background: var(--panel); }
  .group input { margin-top: 3px; accent-color: var(--accent); width: 16px; height: 16px; }
  .group > span { display: flex; flex-direction: column; gap: 2px; }
</style>
