<script lang="ts">
  import { t, LANGUAGES } from '../../lib/i18n.svelte';
  import { app } from '../../lib/store.svelte';
  import { settings } from './draft.svelte';
  import Toggle from '../../components/controls/Toggle.svelte';
  import Range from '../../components/controls/Range.svelte';

  const d = $derived(settings.draft!);
</script>

<label class="field enter" style:--i={0}>{t('Lingua')}
  <select class="input" bind:value={d.language}>
    {#each LANGUAGES as [id, label] (id)}<option value={id}>{label}</option>{/each}
  </select>
</label>
<div class="enter" style:--i={1}><Toggle bind:checked={d.minimizeOnLaunch} label="Riduci a icona quando il gioco parte" hint="Il launcher torna visibile quando chiudi Minecraft." /></div>
<div class="enter" style:--i={2}><Toggle bind:checked={d.showConsole} label="Mostra la console di Java" hint="Apre la finestra nera con i messaggi di Java accanto al gioco. Il registro resta comunque nella scheda Console dell’istanza." /></div>
<div class="enter" style:--i={3}><Range bind:value={d.downloadConcurrency} label="Download in parallelo" min={1} max={64} /></div>
<p class="muted small enter" style:--i={4}>{t('Più download insieme vanno più veloci con una buona connessione; abbassali se la rete è lenta o condivisa.')}</p>
<div class="box enter" style:--i={5}>
  <div><div class="strong">{t('Configurazione iniziale')}</div><div class="muted small">{t('Rivedi lingua, account e stile come al primo avvio.')}</div></div>
  <button class="btn" onclick={() => app.settings && app.applySettings({ ...$state.snapshot(app.settings), setupDone: false })}>{t('Ripeti')}</button>
</div>

<style>
  .strong { font-weight: 600; }
  p { margin: 0; }
  .box { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 14px; border-radius: var(--radius-sm); background: var(--panel); flex-wrap: wrap; }
</style>
