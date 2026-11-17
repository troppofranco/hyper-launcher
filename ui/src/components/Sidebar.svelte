<script lang="ts">
  import { app } from '../lib/store.svelte';
  import Icon from './Icon.svelte';
  import InstanceIcon from './InstanceIcon.svelte';
  import AccountMenu from './AccountMenu.svelte';
  import { initial } from '../lib/format';

  let menuOpen = $state(false);
  const shortcuts = $derived(app.recent.slice(0, 5));
  const view = $derived(app.route.view);
</script>

<nav class="sidebar" aria-label="Navigazione principale">
  <div class="account-wrap">
    <button class="avatar" aria-label={app.activeAccount ? `Account: ${app.activeAccount.name}` : 'Aggiungi un account'} aria-expanded={menuOpen} onclick={() => (menuOpen = !menuOpen)}>
      {app.activeAccount ? initial(app.activeAccount.name) : '+'}
    </button>
    {#if menuOpen}<AccountMenu onclose={() => (menuOpen = false)} />{/if}
  </div>
  <div class="sep"></div>
  <button class="nav" class:active={view === 'home'} aria-label="Home" onclick={() => app.go({ view: 'home' })}><Icon name="home" size={22} /></button>
  <button class="nav" class:active={view === 'library'} aria-label="Libreria" onclick={() => app.go({ view: 'library' })}><Icon name="library" size={22} /></button>
  <button class="nav" class:active={view === 'explore' || view === 'project'} aria-label="Esplora" title="Esplora" onclick={() => app.go({ view: 'explore' })}><Icon name="compass" size={22} /></button>
  <button class="nav" aria-label="Nuova istanza" onclick={() => (app.newInstanceOpen = true)}><Icon name="plus" size={22} /></button>
  {#if shortcuts.length}<div class="sep"></div>{/if}
  {#each shortcuts as inst (inst.name)}
    <button
      class="shortcut"
      class:current={app.route.view === 'instance' && app.route.name === inst.name}
      class:running={!!app.launches[inst.name]}
      aria-label={inst.name}
      title={inst.name}
      onclick={() => app.go({ view: 'instance', name: inst.name })}
    >
      <InstanceIcon name={inst.name} size={42} />
    </button>
  {/each}
  <div class="spacer"></div>
  <button class="nav" class:active={view === 'settings'} aria-label="Impostazioni" onclick={() => app.go({ view: 'settings' })}><Icon name="settings" size={22} /></button>
</nav>

<style>
  .sidebar { flex: 0 0 76px; display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 14px 0; background: var(--panel); border-right: 1px solid #1f232c; }
  .account-wrap { position: relative; }
  .avatar { width: 48px; height: 48px; border-radius: 14px; border: 2px solid var(--accent); background: var(--accent-soft); font-weight: 700; font-size: 17px; font-family: var(--display); }
  .sep { width: 32px; height: 1px; background: #262a34; margin: 6px 0; }
  .nav { width: 48px; height: 48px; border-radius: 14px; border: 0; background: transparent; color: var(--muted); display: flex; align-items: center; justify-content: center; }
  .nav:hover { background: var(--surface-2); color: var(--text); }
  .nav.active { background: var(--accent); color: #fff; }
  .shortcut { padding: 0; border: 0; background: transparent; border-radius: 13px; position: relative; }
  .shortcut.current { outline: 2px solid var(--accent); outline-offset: 2px; }
  .shortcut.running::after { content: ''; position: absolute; right: -2px; bottom: -2px; width: 12px; height: 12px; border-radius: 6px; background: #3ecf8e; border: 2px solid var(--panel); }
  .spacer { flex: 1; }
</style>
