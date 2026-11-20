<script lang="ts">
  // Scelta dell'immagine di un'istanza: un blocco di Minecraft (le texture
  // vengono dal gioco già scaricato sul PC) oppure un'immagine dal computer.
  import { api } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { t } from '../lib/i18n.svelte';
  import { pickFile, pixelPng, squarePng } from '../lib/image';
  import Icon from './Icon.svelte';
  import Modal from './Modal.svelte';

  let { onpick, onclose, removable = false }: {
    /** PNG scelto come `data:` URL, oppure `null` per togliere l'immagine. */
    onpick: (png: string | null) => void;
    onclose: () => void;
    removable?: boolean;
  } = $props();

  let blocks = $state<{ name: string; image: string }[] | null>(null);
  let busy = $state(false);

  $effect(() => {
    api.blockIcons().then((b) => (blocks = b)).catch(() => (blocks = []));
  });

  /** "diamond_ore" → "Diamond ore". */
  const label = (name: string) => {
    const words = name.replace(/_(side|front)$/, '').replaceAll('_', ' ');
    return words.charAt(0).toUpperCase() + words.slice(1);
  };

  async function choose(make: () => Promise<string | null>) {
    busy = true;
    try {
      const png = await make();
      if (png) {
        onpick(png);
        onclose();
      }
    } catch (e) {
      app.error(e);
    } finally {
      busy = false;
    }
  }

  const fromBlock = (image: string) => choose(() => pixelPng(image));
  const fromComputer = () =>
    choose(async () => {
      const file = await pickFile('image/png,image/jpeg,image/webp,image/gif');
      return file ? squarePng(file) : null;
    });
</script>

<Modal title={t('Immagine dell’istanza')} {onclose} width={540}>
  <div class="wrap">
    {#if blocks === null}
      <p class="muted small">{t('Caricamento…')}</p>
    {:else if blocks.length}
      <div class="grid">
        {#each blocks as b (b.name)}
          <button type="button" class="block" disabled={busy} title={label(b.name)} aria-label={label(b.name)} onclick={() => fromBlock(b.image)}>
            <img src={b.image} alt="" width="48" height="48" />
          </button>
        {/each}
      </div>
    {:else}
      <p class="muted small">{t('I blocchi arrivano dai file del gioco: compaiono dopo aver scaricato una versione di Minecraft (basta avviare un’istanza una volta).')}</p>
    {/if}
    <div class="actions">
      <button type="button" class="btn" disabled={busy} onclick={fromComputer}><Icon name="upload" size={15} /> {t('Carica dal PC')}</button>
      {#if removable}
        <button type="button" class="btn" disabled={busy} onclick={() => { onpick(null); onclose(); }}>{t('Togli immagine')}</button>
      {/if}
      <span class="spacer"></span>
      <button type="button" class="btn" onclick={onclose}>{t('Annulla')}</button>
    </div>
  </div>
</Modal>

<style>
  .wrap { display: flex; flex-direction: column; gap: 16px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(60px, 1fr)); gap: 8px; }
  .block { aspect-ratio: 1; padding: 6px; border-radius: var(--radius-sm); border: 1px solid var(--line); background: var(--panel); display: flex; align-items: center; justify-content: center; transition: border-color .15s ease, transform .12s ease; }
  .block:hover:not(:disabled) { border-color: var(--accent-line); transform: translateY(-1px); }
  .block img { width: 100%; height: 100%; image-rendering: pixelated; border-radius: 4px; }
  .actions { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
  .spacer { flex: 1; }
</style>
