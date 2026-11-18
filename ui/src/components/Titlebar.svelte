<script lang="ts">
  import { t } from '../lib/i18n.svelte';
  import { windowAction } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import Icon from './Icon.svelte';
  import logo from '../assets/logo.svg';
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="brand" data-tauri-drag-region>
    <img src={logo} alt="" width="18" height="18" />
    <span data-tauri-drag-region>Hyper Launcher</span>
    {#if app.info?.build}<span class="build" data-tauri-drag-region>build {app.info.build}</span>{/if}
  </div>
  <div class="controls">
    <button aria-label={t('Riduci a icona')} onclick={() => windowAction('minimize')}><Icon name="minimize" size={16} /></button>
    <button aria-label={t('Ingrandisci')} onclick={() => windowAction('maximize')}><Icon name="maximize" size={14} /></button>
    <button class="close" aria-label={t('Chiudi')} onclick={() => windowAction('close')}><Icon name="close" size={16} /></button>
  </div>
</header>

<style>
  .titlebar { height: 36px; flex: 0 0 36px; display: flex; align-items: center; justify-content: space-between; padding-left: 14px; background: var(--bg-deep); border-bottom: 1px solid var(--line); }
  .brand { display: flex; align-items: center; gap: 10px; font-family: var(--display); font-weight: 600; font-size: 13px; }
  .brand img { border-radius: 4px; }
  .build { font-family: var(--font); font-size: 11px; color: var(--muted); border: 1px solid var(--line-strong); border-radius: 6px; padding: 1px 6px; font-weight: 500; }
  .controls { display: flex; height: 100%; }
  .controls button { width: 46px; height: 100%; background: transparent; border: 0; color: var(--muted); display: flex; align-items: center; justify-content: center; }
  .controls button:hover { background: var(--surface-2); color: var(--text); }
  .controls .close:hover { background: #c42b1c; color: #fff; }
</style>
