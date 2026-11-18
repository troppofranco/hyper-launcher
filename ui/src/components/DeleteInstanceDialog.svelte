<script lang="ts">
  // Conferma di eliminazione di un'istanza (o modpack): si scrive il nome
  // per non cancellare mondi per sbaglio.
  import { api } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { t } from '../lib/i18n.svelte';
  import Modal from './Modal.svelte';

  let { name, onclose, ondeleted }: { name: string; onclose: () => void; ondeleted?: () => void } = $props();

  let confirm = $state('');
  let busy = $state(false);
  const parts = $derived(t('Verranno cancellati per sempre i mondi, le mod e le impostazioni di {name}. Non si può annullare.').split('{name}'));

  async function remove(e: SubmitEvent) {
    e.preventDefault();
    if (confirm !== name) return;
    // Il nome si fissa subito: chiudendo, chi apre il dialogo può azzerarlo.
    const target = name;
    busy = true;
    try {
      await api.deleteInstance(target);
      app.info_(t('“{nome}” eliminata.', { nome: target }));
      onclose();
      ondeleted?.();
      await app.refreshInstances();
    } catch (err) {
      app.error(err);
    } finally {
      busy = false;
    }
  }
</script>

<Modal title={t('Eliminare l’istanza?')} {onclose} width={480}>
  <form class="form" onsubmit={remove}>
    <p>{parts[0]}<strong>{name}</strong>{parts[1]}</p>
    <label class="field">{t('Scrivi il nome dell’istanza per confermare')}
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
