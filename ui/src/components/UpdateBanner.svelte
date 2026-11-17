<script lang="ts">
  import { api } from '../lib/api';
  import { app } from '../lib/store.svelte';

  let installing = $state(false);

  async function install() {
    installing = true;
    try {
      await api.installUpdate();
    } catch (e) {
      installing = false;
      app.error(e);
    }
  }
</script>

{#if app.update?.available}
  <div class="banner" role="status">
    <span>È disponibile una nuova versione di Hyper Launcher (build {app.update.available}).</span>
    <button class="btn primary small" disabled={installing} onclick={install}>{installing ? 'Aggiornamento…' : 'Aggiorna e riavvia'}</button>
  </div>
{/if}

<style>
  .banner { display: flex; align-items: center; justify-content: center; gap: 16px; padding: 8px 16px; background: var(--accent-soft); border-bottom: 1px solid var(--accent); font-weight: 500; }
</style>
