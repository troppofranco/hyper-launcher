<script lang="ts">
  import { t } from '../lib/i18n.svelte';
  import { api } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import Avatar from './Avatar.svelte';
  import Icon from './Icon.svelte';

  let { onclose }: { onclose: () => void } = $props();

  async function choose(name: string) {
    try {
      app.accounts = await api.setDefaultAccount(name);
      onclose();
    } catch (e) {
      app.error(e);
    }
  }

  async function remove(name: string) {
    try {
      app.accounts = await api.removeAccount(name);
    } catch (e) {
      app.error(e);
    }
  }

  function add() {
    onclose();
    app.addAccountOpen = true;
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />
<button class="backdrop" aria-label={t('Chiudi menu account')} onclick={onclose}></button>
<div class="menu" role="menu">
  {#each app.accounts as a (a.uuid)}
    <div class="row" class:active={a.default}>
      <button class="pick" role="menuitem" onclick={() => choose(a.name)}>
        <Avatar account={a} size={40} />
        <span class="who">
          <span class="name">{a.name}</span>
          <span class="kind">{a.kind === 'microsoft' ? t('Account Microsoft') : t('Account offline')}{a.default ? ' · ' + t('attivo') : ''}</span>
        </span>
      </button>
      <button class="remove" aria-label={t('Rimuovi {nome}', { nome: a.name })} title={t('Rimuovi')} onclick={() => remove(a.name)}><Icon name="logout" size={16} /></button>
    </div>
  {:else}
    <p class="empty">{t('Nessun account. Aggiungine uno per giocare.')}</p>
  {/each}
  <button class="btn add" onclick={add}><Icon name="plus" size={16} /> {t('Aggiungi account')}</button>
</div>

<style>
  .backdrop { position: fixed; inset: 0; background: transparent; border: 0; z-index: 20; cursor: default; }
  /* Si apre accanto al pulsante, verso il contenuto (a sinistra se la barra è a destra). */
  .menu { position: absolute; left: calc(100% + 12px); top: 0; z-index: 21; width: 310px; background: var(--c-surface); border: 1px solid var(--line-strong); border-radius: 14px; padding: 10px; box-shadow: 0 18px 40px rgba(0,0,0,.45); display: flex; flex-direction: column; gap: 6px; }
  :global(.body.right) .menu { left: auto; right: calc(100% + 12px); }
  .row { display: flex; align-items: center; border-radius: 10px; }
  .row.active { background: var(--surface-2); }
  .pick { flex: 1; display: flex; align-items: center; gap: 12px; padding: 8px; background: transparent; border: 0; text-align: left; border-radius: 10px; }
  .pick:hover { background: var(--surface-2); }
  .who { display: flex; flex-direction: column; min-width: 0; }
  .name { font-weight: 600; }
  .kind { font-size: 12px; color: var(--muted); }
  .remove { width: 36px; height: 36px; border: 0; background: transparent; color: var(--muted); border-radius: 8px; margin-right: 4px; }
  .remove:hover { color: var(--danger-text); background: var(--surface-2); }
  .empty { margin: 6px; color: var(--muted); }
  .add { margin-top: 4px; }
</style>
