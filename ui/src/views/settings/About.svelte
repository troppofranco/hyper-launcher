<script lang="ts">
  import { t } from '../../lib/i18n.svelte';
  import { api } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import Icon from '../../components/Icon.svelte';

  let checking = $state(false);

  async function checkUpdate() {
    checking = true;
    try {
      app.update = await api.checkUpdate();
      app.updateDismissed = false;
      if (!app.update.current) app.info_(t('Questa è una versione di sviluppo: gli aggiornamenti automatici sono disattivati.'));
      else if (!app.update.available) app.info_(t('Hyper Launcher è aggiornato.'));
    } catch (e) {
      app.error(e);
    } finally {
      checking = false;
    }
  }
</script>

<div class="hero enter" style:--i={0}>
  <span class="logo" aria-hidden="true"><Icon name="play" size={26} /></span>
  <div><div class="name">Hyper Launcher</div><div class="muted">{t('Versione {v}', { v: app.info?.version ?? '' })}{app.info?.dev ? ` · ${t('versione di sviluppo')}` : ''}</div></div>
</div>
<p class="muted enter" style:--i={1}>{t('Gli aggiornamenti si installano da soli all’avvio.')}</p>
<div class="actions enter" style:--i={2}>
  <button class="btn primary" disabled={checking} onclick={checkUpdate}><Icon name="refresh" size={16} /> {checking ? t('Controllo…') : t('Controlla aggiornamenti')}</button>
  <button class="btn" onclick={() => api.openUrl('https://github.com/troppofranco/hyper-launcher')}><Icon name="external" size={16} /> GitHub</button>
</div>

<style>
  p { margin: 0; }
  .hero { display: flex; align-items: center; gap: 14px; }
  .logo { width: 52px; height: 52px; border-radius: 14px; background: var(--accent); color: #fff; display: flex; align-items: center; justify-content: center; }
  .name { font-family: var(--display); font-size: 20px; font-weight: 600; }
  .actions { display: flex; gap: 10px; flex-wrap: wrap; }
</style>
