<script lang="ts">
  // Finestrella al centro quando esce una nuova build: versione, link alla
  // release su GitHub e "Aggiorna e riavvia". Lo sfondo resta sfocato.
  import { t } from '../lib/i18n.svelte';
  import { api } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import Icon from './Icon.svelte';
  import { fade, scale } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { prefersReducedMotion } from 'svelte/motion';

  const d = (ms: number) => (prefersReducedMotion.current ? 0 : ms);
  let installing = $state(false);

  // Solo dopo la configurazione iniziale e se non è stato rimandato.
  const update = $derived(app.settings?.setupDone && !app.updateDismissed && app.update?.available ? app.update : null);

  async function install() {
    installing = true;
    try {
      await api.installUpdate();
    } catch (e) {
      installing = false;
      app.error(e);
    }
  }

  function later() {
    if (!installing) app.updateDismissed = true;
  }
</script>

<svelte:window onkeydown={(e) => update && e.key === 'Escape' && later()} />

{#if update}
  <div class="overlay" transition:fade={{ duration: d(200) }}>
    <div class="box" role="alertdialog" aria-modal="true" aria-labelledby="update-title" aria-describedby="update-text" transition:scale={{ start: 0.92, duration: d(260), easing: cubicOut }}>
      <div class="badge" aria-hidden="true"><Icon name="download" size={24} /></div>
      <h2 id="update-title">{t('È disponibile un aggiornamento')}</h2>
      <p id="update-text" class="version">
        <span class="pill new">{t('Build {n}', { n: update.available ?? '' })}</span>
        {#if update.current}<span class="muted small">{t('Ora hai la build {n}', { n: update.current })}</span>{/if}
      </p>
      {#if update.url}
        <button class="link" onclick={() => update.url && api.openUrl(update.url)}>
          <Icon name="external" size={14} /> {t('Novità su GitHub')}
        </button>
      {/if}
      <div class="actions">
        <button class="btn" disabled={installing} onclick={later}>{t('Più tardi')}</button>
        <button class="btn primary" disabled={installing} onclick={install}>{installing ? t('Aggiornamento…') : t('Aggiorna e riavvia')}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed; inset: 36px 0 0; z-index: 40; display: flex; align-items: center; justify-content: center; padding: 24px;
    background: color-mix(in srgb, var(--bg) 45%, transparent);
    backdrop-filter: blur(10px) saturate(1.1);
    -webkit-backdrop-filter: blur(10px) saturate(1.1);
  }
  .box {
    width: 380px; max-width: 100%; padding: 26px 24px 20px; border-radius: 18px; text-align: center;
    background: var(--surface); border: 1px solid var(--line-strong); box-shadow: 0 24px 60px var(--shadow);
    display: flex; flex-direction: column; align-items: center; gap: 12px;
  }
  .badge { width: 52px; height: 52px; border-radius: 16px; background: var(--accent); color: #fff; display: flex; align-items: center; justify-content: center; box-shadow: 0 8px 22px color-mix(in srgb, var(--accent) 40%, transparent); }
  h2 { font-size: 18px; font-weight: 600; margin: 4px 0 0; }
  .version { margin: 0; display: flex; flex-direction: column; align-items: center; gap: 6px; }
  .pill { font-family: var(--mono); font-size: 12px; padding: 4px 10px; border-radius: 999px; }
  .pill.new { background: var(--accent-soft); color: var(--text); border: 1px solid var(--accent-line); }
  .link { display: inline-flex; align-items: center; gap: 6px; border: 0; background: none; padding: 2px 4px; color: var(--accent-line); font-weight: 600; font-size: 13px; text-decoration: underline; text-underline-offset: 3px; }
  .actions { display: flex; gap: 10px; width: 100%; margin-top: 8px; }
  .actions .btn { flex: 1; white-space: nowrap; }
</style>
