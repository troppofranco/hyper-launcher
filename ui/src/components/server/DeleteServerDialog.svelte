<script lang="ts">
  // Conferma di eliminazione di un server: si scrive il nome per non
  // cancellare il mondo per sbaglio.
  import { api } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import { t } from '../../lib/i18n.svelte';
  import Modal from '../Modal.svelte';

  let { name, onclose, ondeleted }: { name: string; onclose: () => void; ondeleted?: () => void } = $props();

  let confirm = $state('');
  let busy = $state(false);
  const parts = $derived(t('Verranno cancellati per sempre il mondo, i plugin e le impostazioni di {name}. Non si può annullare.').split('{name}'));

  async function remove(e: SubmitEvent) {
    e.preventDefault();
    if (confirm !== name) return;
    const target = name;
    busy = true;
    try {
      await api.serverDelete(target);
      app.info_(t('Server “{nome}” eliminato.', { nome: target }));
      onclose();
      ondeleted?.();
    } catch (err) {
      app.error(err);
    } finally {
      busy = false;
    }
  }
</script>

<Modal title={t('Eliminare il server?')} {onclose} width={480}>
  <form class="form" onsubmit={remove}>
    <p>{parts[0]}<strong>{name}</strong>{parts[1]}</p>
    <label class="field">{t('Scrivi il nome del server per confermare')}
      <input class="input" bind:value={confirm} placeholder={name} autocomplete="off" />
    </label>
    <div class="actions">
      <button type="button" class="btn" onclick={onclose}>{t('Annulla')}</button>
      <button type="submit" class="btn danger" disabled={busy || confirm !== name}>{t('Elimina definitivamente')}</button>
    </div>
  </form>
</Modal>

<style>
  .form { display: flex; flex-direction: column; gap: 14px; }
  p { margin: 0; line-height: 1.5; }
  .actions { display: flex; justify-content: flex-end; gap: 10px; }
</style>
