<script lang="ts">
  import { t } from '../../lib/i18n.svelte';
  import { api } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import { ACCENTS, THEMES } from '../../lib/look';
  import { fitImage, pickFile } from '../../lib/image';
  import { settings } from './draft.svelte';
  import Icon from '../../components/Icon.svelte';
  import Range from '../../components/controls/Range.svelte';
  import Segmented from '../../components/controls/Segmented.svelte';
  import { slide } from 'svelte/transition';
  import { ms } from '../../lib/motion.svelte';

  const d = $derived(settings.draft!);
  const a = $derived(d.appearance);
  const custom = $derived(!ACCENTS.some(([c]) => c === d.accent));
  let uploading = $state(false);

  const BACKGROUNDS: ['solid' | 'transparent' | 'image' | 'gradient', string, string][] = [
    ['solid', 'Tinta unita', 'I colori del tema.'],
    ['gradient', 'Sfumato', 'Due colori che si fondono.'],
    ['transparent', 'Trasparente', 'Il desktop dietro, sfocato.'],
    ['image', 'Immagine', 'Una tua foto, sfocata a piacere.'],
  ];

  async function chooseImage() {
    const file = await pickFile('image/png,image/jpeg,image/webp,image/gif');
    if (!file) return;
    uploading = true;
    try {
      await api.setBackground(await fitImage(file));
      await app.loadBackground(true, true);
      a.background = 'image';
    } catch (e) {
      app.error(e);
    } finally {
      uploading = false;
    }
  }

  async function removeImage() {
    try {
      await api.setBackground(null);
      app.backgroundImage = null;
      a.background = 'solid';
    } catch (e) {
      app.error(e);
    }
  }
</script>

<section class="group enter" style:--i={0}>
  <span class="strong">{t('Tema')}</span>
  <div class="choices two">
    {#each THEMES as [id, label] (id)}
      <button class="choice" class:on={d.theme === id} aria-pressed={d.theme === id} onclick={() => (d.theme = id)}>
        <span class="mini {id}"></span>{t(label)}
      </button>
    {/each}
  </div>
</section>

<section class="group enter" style:--i={1}>
  <span class="strong">{t('Colore accento')}</span>
  <div class="swatches">
    {#each ACCENTS as [color, label] (color)}
      <button class="swatch" class:on={d.accent === color} style:background={color} aria-label={t(label)} title={t(label)} aria-pressed={d.accent === color} onclick={() => (d.accent = color)}></button>
    {/each}
    <label class="custom" class:on={custom} title={t('Colore personalizzato')}>
      <input type="color" bind:value={d.accent} aria-label={t('Colore personalizzato')} />
      <span class="dot" style:background={custom ? d.accent : 'conic-gradient(#e5484d, #f3c98a, #3fb97f, #2f6fe4, #6e5ae6, #e5484d)'}></span>
      {t('Personalizzato')}
    </label>
  </div>
</section>

<section class="group enter" style:--i={2}>
  <span class="strong">{t('Sfondo')}</span>
  <div class="choices four">
    {#each BACKGROUNDS as [id, label, hint] (id)}
      <button class="choice tall" class:on={a.background === id} aria-pressed={a.background === id} onclick={() => (id === 'image' && !app.backgroundImage ? chooseImage() : (a.background = id))}>
        <span class="thumb {id}"></span>
        <span class="lbl">{t(label)}</span>
        <span class="muted small">{t(hint)}</span>
      </button>
    {/each}
  </div>
</section>

{#if a.background === 'transparent'}
  <section class="group sub" transition:slide={{ duration: ms(240) }}>
    <Segmented bind:value={a.effect} label="Intensità della sfocatura" options={[['blur', 'Leggera'], ['acrylic', 'Media'], ['mica', 'Forte']]} />
    <p class="muted small">{t('Sono gli effetti di Windows: Leggera lascia intravedere le finestre dietro, Media le sfoca e le tinge, Forte mostra solo i colori dello sfondo del desktop (Windows 11; su Windows 10 diventa Media).')}</p>
    <Range bind:value={a.panelOpacity} label="Opacità dei pannelli" min={20} max={100} step={5} unit="%" />
  </section>
{:else if a.background === 'gradient'}
  <section class="group sub" transition:slide={{ duration: ms(240) }}>
    <div class="grad-row">
      <span class="grad-preview" style:background={`linear-gradient(${{ down: 'to bottom', up: 'to top', right: 'to right', left: 'to left' }[a.gradientDirection]}, ${a.gradientFrom}, ${a.gradientTo})`}></span>
      <label class="color-pick">
        <input type="color" bind:value={a.gradientFrom} aria-label={t('Primo colore')} />
        <span class="muted small">{t('Da')}</span>
      </label>
      <label class="color-pick">
        <input type="color" bind:value={a.gradientTo} aria-label={t('Secondo colore')} />
        <span class="muted small">{t('A')}</span>
      </label>
      <button class="btn small" onclick={() => ([a.gradientFrom, a.gradientTo] = [a.gradientTo, a.gradientFrom])}>{t('Inverti')}</button>
    </div>
    <Segmented bind:value={a.gradientDirection} label="Verso" options={[['down', '↓ In basso'], ['up', '↑ In alto'], ['right', '→ A destra'], ['left', '← A sinistra']]} />
    <Range bind:value={a.panelOpacity} label="Opacità dei pannelli" min={20} max={100} step={5} unit="%" />
  </section>
{:else if a.background === 'image'}
  <section class="group sub" transition:slide={{ duration: ms(240) }}>
    <div class="image-row">
      <span class="preview" style:background-image={app.backgroundImage ? `url(${app.backgroundImage})` : 'none'}></span>
      <span class="muted small grow">{t('L’immagine viene ridotta e salvata nella cartella del launcher.')}</span>
      <button class="btn small" disabled={uploading} onclick={chooseImage}><Icon name="image" size={15} /> {uploading ? t('Caricamento…') : t('Cambia immagine')}</button>
      <button class="btn small" aria-label={t('Togli immagine')} title={t('Togli immagine')} onclick={removeImage}><Icon name="trash" size={15} /></button>
    </div>
    <Range bind:value={a.blur} label="Sfocatura" min={0} max={40} unit=" px" />
    <Range bind:value={a.dim} label="Oscuramento" min={0} max={80} step={5} unit="%" />
    <Range bind:value={a.panelOpacity} label="Opacità dei pannelli" min={20} max={100} step={5} unit="%" />
  </section>
{/if}

<section class="group enter" style:--i={3}>
  <Segmented bind:value={a.buttons} label="Stile dei pulsanti" options={[['standard', 'Standard'], ['liquid', 'Liquid glass']]} />
</section>

<section class="group enter" style:--i={4}>
  <Segmented bind:value={a.animations} label="Animazioni" options={[['full', 'Complete'], ['reduced', 'Ridotte'], ['off', 'Spente']]} />
</section>

<style>
  .group { display: flex; flex-direction: column; gap: 10px; }
  .sub { padding: 14px; border-radius: var(--radius-sm); background: var(--panel); gap: 14px; }
  .strong { font-weight: 600; }
  p { margin: 0; line-height: 1.5; }
  .choices { display: grid; gap: 10px; }
  .two { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .three { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  .four { grid-template-columns: repeat(4, minmax(0, 1fr)); }
  @media (max-width: 760px) { .four { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
  .choice { display: flex; align-items: center; gap: 10px; height: 52px; padding: 0 14px 0 10px; border-radius: var(--radius-sm); border: 1px solid var(--line-strong); background: var(--panel); font-weight: 600; transition: border-color .18s ease, background .18s ease, transform .15s ease; }
  .choice:hover { border-color: var(--accent-line); }
  .choice:active { transform: scale(.98); }
  .choice.on { border-color: var(--accent-line); background: var(--accent-soft); box-shadow: 0 0 0 1px var(--accent-line) inset; }
  .tall { height: auto; padding: 12px; flex-direction: column; align-items: flex-start; gap: 6px; text-align: left; }
  .lbl { font-weight: 600; }
  .thumb { width: 100%; height: 54px; border-radius: 8px; border: 1px solid var(--line-strong); }
  .thumb.solid { background: var(--c-bg); }
  .thumb.transparent { background: linear-gradient(135deg, rgba(255,255,255,.28), rgba(255,255,255,.04)), repeating-linear-gradient(45deg, #3b4252 0 8px, #2b313d 8px 16px); }
  .thumb.image { background: linear-gradient(160deg, #3d7a4a, #6f9ccf 55%, #8a5b2e); filter: blur(.5px); }
  .thumb.gradient { background: linear-gradient(to bottom, var(--accent), var(--c-bg)); }
  .grad-row { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .grad-preview { width: 96px; height: 54px; border-radius: 8px; flex: none; border: 1px solid var(--line-strong); }
  .color-pick { display: flex; align-items: center; gap: 6px; }
  .color-pick input { width: 40px; height: 32px; padding: 0; border: 1px solid var(--line-strong); border-radius: 8px; background: none; cursor: pointer; }
  .mini { width: 30px; height: 30px; border-radius: 8px; border: 1px solid var(--line-strong); }
  .mini.dark { background: linear-gradient(135deg, #0f1115 50%, #1e222b 50%); }
  .mini.light { background: linear-gradient(135deg, #ffffff 50%, #e8eaef 50%); }
  .swatches { display: flex; gap: 10px; flex-wrap: wrap; align-items: center; }
  .swatch { width: 38px; height: 38px; border-radius: 19px; border: 3px solid transparent; transition: transform .15s ease; }
  .swatch:hover { transform: scale(1.08); }
  .swatch.on { border-color: var(--text); }
  .custom { position: relative; display: inline-flex; align-items: center; gap: 8px; height: 38px; padding: 0 12px 0 6px; border-radius: 19px; border: 1px dashed var(--line-strong); font-weight: 600; font-size: 13px; cursor: pointer; }
  .custom.on { border-style: solid; border-color: var(--text); }
  .custom input { position: absolute; inset: 0; opacity: 0; cursor: pointer; }
  .dot { width: 26px; height: 26px; border-radius: 13px; }
  .image-row { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .preview { width: 96px; height: 54px; border-radius: 8px; background: var(--surface-2) center / cover; flex: none; border: 1px solid var(--line-strong); }
  .grow { flex: 1; min-width: 160px; }
</style>
