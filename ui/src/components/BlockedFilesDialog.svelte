<script lang="ts">
  // Dopo l'import di un modpack CurseForge: file che l'autore non permette
  // di scaricare dai launcher, da prendere a mano dalle loro pagine.
  import { t } from '../lib/i18n.svelte';
  import { api } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import Modal from './Modal.svelte';
  import Icon from './Icon.svelte';

  const files = $derived(app.blockedFiles ?? []);
  const instance = $derived(app.route.view === 'instance' ? app.route.name : null);
  const close = () => (app.blockedFiles = null);
</script>

<Modal title={t('Alcune mod vanno scaricate a mano')} onclose={close} width={560}>
  <p>{t('Gli autori di queste mod non permettono il download dai launcher. Scaricale dalla loro pagina CurseForge e mettile nella cartella mods dell’istanza.')}</p>
  <ul class="list">
    {#each files as f (f.fileName)}
      <li class="row">
        <span class="who"><span class="name">{f.name}</span><span class="muted small">{f.fileName}</span></span>
        <button class="btn small" onclick={() => api.openUrl(f.url)}><Icon name="external" size={15} /> {t('Apri pagina')}</button>
      </li>
    {/each}
  </ul>
  <div class="actions">
    {#if instance}<button class="btn" onclick={() => api.openFolder(instance, 'mods')}><Icon name="folder" size={16} /> {t('Apri cartella mods')}</button>{/if}
    <button class="btn primary" onclick={close}>{t('Ho capito')}</button>
  </div>
</Modal>

<style>
  p { margin: 0; line-height: 1.5; color: var(--text-2); }
  .list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; max-height: 320px; overflow-y: auto; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 12px; border-radius: 10px; background: var(--panel); }
  .who { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .name { font-weight: 600; }
  .who .small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .actions { display: flex; justify-content: flex-end; gap: 10px; }
</style>
