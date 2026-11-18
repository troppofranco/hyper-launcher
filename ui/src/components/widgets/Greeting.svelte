<script lang="ts">
  import { t } from '../../lib/i18n.svelte';
  import { app } from '../../lib/store.svelte';
  import Icon from '../Icon.svelte';

  const GREETINGS = ['Pronto a costruire qualcosa di grande?', 'Il mondo ti aspetta.', 'Si gioca?', 'Un altro blocco, un’altra avventura.'];
  const greetingIndex = new Date().getDate() % GREETINGS.length;
</script>

<header class="head">
  <div>
    <h1 class="page-title">{app.activeAccount ? t('Bentornato, {nome}', { nome: app.activeAccount.name }) : t('Benvenuto in Hyper Launcher')}</h1>
    <p class="muted">{app.instances.length ? t(GREETINGS[greetingIndex]) : t('Crea la tua prima istanza per iniziare a giocare.')}</p>
  </div>
  <button class="card lib-btn" onclick={() => app.go({ view: 'library' })}>
    <Icon name="library" size={20} />
    <span><span class="strong">{t('Libreria')}</span><span class="muted small">{t('Tutte le tue istanze')}</span></span>
  </button>
</header>

<style>
  .head { flex: 1; display: flex; justify-content: space-between; align-items: center; gap: 16px; flex-wrap: wrap; }
  .head p { margin: 6px 0 0; }
  .lib-btn { display: flex; align-items: center; gap: 12px; padding: 12px 18px; text-align: left; color: var(--text); }
  .lib-btn > span { display: flex; flex-direction: column; }
  .strong { font-weight: 600; }
</style>
