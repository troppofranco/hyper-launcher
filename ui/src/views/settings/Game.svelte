<script lang="ts">
  import { t } from '../../lib/i18n.svelte';
  import { settings } from './draft.svelte';
  import Toggle from '../../components/controls/Toggle.svelte';
  import Range from '../../components/controls/Range.svelte';
  import { slide } from 'svelte/transition';
  import { ms } from '../../lib/motion.svelte';

  const d = $derived(settings.draft!);
  // La memoria si sceglie in GB e si salva in MB, solo quando la si cambia:
  // aprire la sezione non arrotonda un valore scritto a mano (es. 3000 MB).
  const memoryGb = {
    get: () => Math.max(1, Math.round(d.defaultMemoryMb / 1024)),
    set: (gb: number) => (d.defaultMemoryMb = gb * 1024),
  };

  const customSize = $derived(d.gameWidth !== null && d.gameHeight !== null);
  function toggleSize() {
    if (customSize) {
      d.gameWidth = null;
      d.gameHeight = null;
    } else {
      d.gameWidth = 1600;
      d.gameHeight = 900;
    }
  }
  // I campi accettano solo numeri ragionevoli (il backend controlla 320–7680).
  const clamp = (v: number) => Math.min(7680, Math.max(320, Math.round(v) || 320));
</script>

<div class="enter" style:--i={0}><Range bind:value={memoryGb.get, memoryGb.set} label="Memoria predefinita" min={1} max={32} unit=" GB" /></div>
<p class="muted small enter" style:--i={1}>{t('Vale per le istanze che non hanno una memoria propria. 4–6 GB bastano per la maggior parte dei modpack.')}</p>
<label class="field enter" style:--i={2}>{t('Java personalizzato (facoltativo)')}
  <input class="input" bind:value={d.javaPath} placeholder={t('Vuoto = runtime ufficiale Mojang scaricato in automatico')} />
</label>
<label class="field enter" style:--i={3}>{t('Argomenti JVM predefiniti')}
  <input class="input mono" bind:value={d.defaultJvmArgs} placeholder="-XX:+UseG1GC" spellcheck="false" />
  <span class="muted small">{t('Usati dalle istanze che non ne hanno di propri.')}</span>
</label>
<div class="enter" style:--i={4}>
  <div class="row">
    <div><div class="strong">{t('Dimensione della finestra di gioco')}</div><div class="muted small">{t('Larghezza e altezza di Minecraft all’avvio.')}</div></div>
    <button class="switch" role="switch" aria-checked={customSize} aria-label={t('Dimensione della finestra di gioco')} onclick={toggleSize}></button>
  </div>
  {#if customSize}
    <div class="size" transition:slide={{ duration: ms(220) }}>
      <label class="field">{t('Larghezza')}<input class="input" type="number" min="320" max="7680" value={d.gameWidth} onchange={(e) => (d.gameWidth = clamp(e.currentTarget.valueAsNumber))} /></label>
      <span class="x">×</span>
      <label class="field">{t('Altezza')}<input class="input" type="number" min="320" max="7680" value={d.gameHeight} onchange={(e) => (d.gameHeight = clamp(e.currentTarget.valueAsNumber))} /></label>
    </div>
  {/if}
</div>
<div class="enter" style:--i={5}><Toggle bind:checked={d.gameFullscreen} label="Schermo intero all’avvio" hint="Minecraft parte già a schermo intero." /></div>

<style>
  .strong { font-weight: 600; }
  p { margin: 0; line-height: 1.5; }
  .mono { font-family: var(--mono); }
  .row { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 4px 0; }
  .size { display: flex; align-items: flex-end; gap: 10px; padding-top: 10px; }
  .size .field { flex: 1; }
  .x { padding-bottom: 12px; color: var(--muted); }
</style>
