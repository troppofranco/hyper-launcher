<script lang="ts">
  // Il gioco si è chiuso con un errore: causa probabile dal crash report,
  // un consiglio e i collegamenti al report e alle mod dell'istanza.
  import { api } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { t } from '../lib/i18n.svelte';
  import Modal from './Modal.svelte';

  const crash = $derived(app.crash);
  const inst = $derived(app.instances.find((i) => i.name === crash?.instance));
  /** "Fabric 0.20.1 · Minecraft 26.3 · codice di uscita 1". */
  const subtitle = $derived([
    inst?.loader ? `${inst.loader.name} ${inst.loader.version}` : null,
    inst ? `Minecraft ${inst.version}` : null,
    crash?.exitCode != null ? t('codice di uscita {n}', { n: crash.exitCode }) : null,
  ].filter(Boolean).join(' · '));
  const isMods = $derived(!!crash?.info.title.toLowerCase().includes('mod'));

  function close() {
    app.crash = null;
  }

  function openReport() {
    if (!crash) return;
    const folder = crash.info.file?.includes('crash-reports') ? 'crash-reports' : 'logs';
    void api.openFolder(crash.instance, folder).catch((e) => app.error(e));
  }

  function toMods() {
    if (!crash) return;
    const name = crash.instance;
    close();
    app.go({ view: 'instance', name });
  }
</script>

{#if crash}
  <Modal title={t('{nome} si è chiusa con un errore', { nome: crash.instance })} onclose={close} width={560}>
    <div class="wrap">
      <p class="muted small sub">{subtitle}</p>
      <div class="box">
        <span class="label">{t('Causa probabile')}</span>
        <strong>{t(crash.info.title)}</strong>
        {#if crash.info.cause}<code>{crash.info.cause}</code>{/if}
        <span class="hint">{t(crash.info.hint)}</span>
      </div>
      <div class="actions">
        <button class="btn" onclick={openReport}>{t('Apri il report completo')}</button>
        <span class="spacer"></span>
        <button class="btn" onclick={close}>{t('Chiudi')}</button>
        {#if isMods}<button class="btn primary" onclick={toMods}>{t('Vai alle mod')}</button>{/if}
      </div>
    </div>
  </Modal>
{/if}

<style>
  .wrap { display: flex; flex-direction: column; gap: 14px; }
  .sub { margin: 0; }
  .box { display: flex; flex-direction: column; gap: 8px; padding: 14px; border-radius: var(--radius-sm); background: var(--panel); border: 1px solid var(--line); }
  .label { font-size: 11px; font-weight: 700; letter-spacing: .04em; text-transform: uppercase; color: var(--muted); }
  code { font-family: var(--mono); font-size: 12px; color: var(--text-2); overflow-wrap: anywhere; user-select: text; }
  .hint { color: var(--text-2); line-height: 1.5; font-size: 13px; }
  .actions { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
  .spacer { flex: 1; }
</style>
