<script lang="ts">
  // Impostazioni: una sezione per file in ./settings. Ogni modifica si vede
  // subito e si salva da sola (vedi settings/draft.svelte.ts).
  import { t } from '../lib/i18n.svelte';
  import Icon from '../components/Icon.svelte';
  import { settings, startEditing, changed } from './settings/draft.svelte';
  import General from './settings/General.svelte';
  import Appearance from './settings/Appearance.svelte';
  import Layout from './settings/Layout.svelte';
  import Game from './settings/Game.svelte';
  import Sync from './settings/Sync.svelte';
  import Microsoft from './settings/Microsoft.svelte';
  import Storage from './settings/Storage.svelte';
  import About from './settings/About.svelte';
  import { fly, fade } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { ms } from '../lib/motion.svelte';
  import type { Component, ComponentProps } from 'svelte';

  type Section = 'general' | 'appearance' | 'layout' | 'game' | 'sync' | 'accounts' | 'storage' | 'about';
  const SECTIONS: [Section, string, ComponentProps<typeof Icon>['name'], string, Component][] = [
    ['general', 'Generale', 'settings', 'Lingua, avvio del gioco e download.', General],
    ['appearance', 'Aspetto', 'image', 'Tema, colori, sfondo e animazioni.', Appearance],
    ['layout', 'Layout', 'library', 'Barra laterale, pannelli, scala e Home.', Layout],
    ['game', 'Gioco', 'play', 'Memoria, Java e finestra di Minecraft.', Game],
    ['sync', 'Sincronizzazione', 'refresh', 'Tasti e impostazioni condivisi tra le istanze.', Sync],
    ['accounts', 'Account Microsoft', 'logout', 'Client ID per il login Microsoft.', Microsoft],
    ['storage', 'Archiviazione', 'folder', 'Cartella dati, spazio e cache.', Storage],
    ['about', 'Informazioni', 'list', 'Versione e aggiornamenti.', About],
  ];

  let section = $state<Section>('general');
  const current = $derived(SECTIONS.find(([id]) => id === section)!);

  startEditing();
  // Ogni modifica alla bozza (anche dentro oggetti annidati) parte da qui.
  $effect(() => {
    JSON.stringify(settings.draft);
    changed();
  });
</script>

<div class="page settings">
  <nav class="nav" aria-label={t('Sezioni delle impostazioni')}>
    <h1 class="page-title">{t('Impostazioni')}</h1>
    {#each SECTIONS as [id, label, icon], i (id)}
      <button class="enter" style:--i={i} class:on={section === id} aria-current={section === id ? 'page' : undefined} onclick={() => (section = id)}>
        <Icon name={icon} size={17} /> {t(label)}
      </button>
    {/each}
    <p class="status muted small" aria-live="polite">
      {#key settings.status}
        <span in:fade={{ duration: ms(160) }}>
          {#if settings.status === 'saving'}{t('Salvataggio…')}{:else if settings.status === 'saved'}<Icon name="check" size={13} /> {t('Salvato')}{:else if settings.status === 'error'}{t('Non salvato')}{:else}{t('Le modifiche si salvano da sole.')}{/if}
        </span>
      {/key}
    </p>
  </nav>

  {#if settings.draft}
    {#key section}
      {@const SectionView = current[4]}
      <section class="card panel" in:fly={{ y: 12, duration: ms(260), easing: cubicOut }}>
        <header>
          <h2>{t(current[1])}</h2>
          <p class="muted small">{t(current[3])}</p>
        </header>
        <SectionView />
      </section>
    {/key}
  {/if}
</div>

<style>
  .settings { flex-direction: row; align-items: flex-start; flex-wrap: wrap; }
  .nav { flex: 0 0 240px; display: flex; flex-direction: column; gap: 4px; position: sticky; top: 0; }
  .nav h1 { margin-bottom: 12px; font-size: 24px; }
  .nav button { height: 42px; display: flex; align-items: center; gap: 10px; text-align: left; padding: 0 12px; border-radius: var(--radius-sm); border: 1px solid transparent; background: transparent; color: var(--text-2); font-weight: 600; transition: background .18s ease, border-color .18s ease, color .18s ease, transform .12s ease; }
  .nav button:hover { background: var(--surface-2); color: var(--text); }
  .nav button:active { transform: scale(.98); }
  .nav button.on { border-color: var(--accent-line); background: var(--accent-soft); color: var(--text); }
  .status { margin: 14px 0 0; min-height: 18px; display: flex; align-items: center; }
  .status span { display: inline-flex; align-items: center; gap: 5px; }
  .panel { flex: 1 1 440px; min-width: 0; padding: 24px; display: flex; flex-direction: column; gap: 18px; }
  header { display: flex; flex-direction: column; gap: 4px; padding-bottom: 4px; border-bottom: 1px solid var(--line); }
  h2 { font-size: 18px; font-weight: 600; }
  header p { margin: 0 0 10px; }
</style>
