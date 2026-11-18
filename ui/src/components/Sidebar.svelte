<script lang="ts">
  import { t } from '../lib/i18n.svelte';
  import { app } from '../lib/store.svelte';
  import Icon from './Icon.svelte';
  import InstanceIcon from './InstanceIcon.svelte';
  import AccountMenu from './AccountMenu.svelte';
  import Avatar from './Avatar.svelte';
  import type { ComponentProps } from 'svelte';

  let menuOpen = $state(false);
  // Da Impostazioni → Layout: etichette accanto alle icone e quante istanze.
  const labels = $derived(!!app.settings?.appearance.sidebarLabels);
  const shortcuts = $derived(app.recent.slice(0, app.settings?.appearance.sidebarInstances ?? 5));
  const view = $derived(app.route.view);

  type Item = { id: string; icon: ComponentProps<typeof Icon>['name']; label: string; active: boolean; go: () => void };
  const items = $derived<Item[]>([
    { id: 'home', icon: 'home', label: t('Home'), active: view === 'home', go: () => app.go({ view: 'home' }) },
    { id: 'library', icon: 'library', label: t('Libreria'), active: view === 'library', go: () => app.go({ view: 'library' }) },
    { id: 'explore', icon: 'compass', label: t('Esplora'), active: view === 'explore' || view === 'project', go: () => app.go({ view: 'explore' }) },
    { id: 'customize', icon: 'shirt', label: t('Personalizzazione'), active: view === 'customize', go: () => app.go({ view: 'customize' }) },
    { id: 'new', icon: 'plus', label: t('Nuova istanza'), active: false, go: () => (app.newInstanceOpen = true) },
  ]);
</script>

<nav class="sidebar" class:labels aria-label={t('Navigazione principale')}>
  <div class="account-wrap">
    <button class="avatar-btn" aria-label={app.activeAccount ? t('Account: {nome}', { nome: app.activeAccount.name }) : t('Aggiungi un account')} aria-expanded={menuOpen} onclick={() => (menuOpen = !menuOpen)}>
      <span class="avatar">{#if app.activeAccount}<Avatar account={app.activeAccount} size={40} />{:else}+{/if}</span>
      {#if labels}<span class="text name">{app.activeAccount?.name ?? t('Aggiungi un account')}</span>{/if}
    </button>
    {#if menuOpen}<AccountMenu onclose={() => (menuOpen = false)} />{/if}
  </div>
  <div class="sep"></div>
  {#each items as item (item.id)}
    <button class="nav" class:active={item.active} aria-label={item.label} title={labels ? undefined : item.label} onclick={item.go}>
      <Icon name={item.icon} size={22} />{#if labels}<span class="text">{item.label}</span>{/if}
    </button>
  {/each}
  {#if shortcuts.length}<div class="sep"></div>{/if}
  <!-- Solo le istanze scorrono: il menu account non viene tagliato. -->
  <div class="list">
  {#each shortcuts as inst (inst.name)}
    <button
      class="shortcut"
      class:current={app.route.view === 'instance' && app.route.name === inst.name}
      class:running={!!app.launches[inst.name]}
      aria-label={inst.name}
      title={labels ? undefined : inst.name}
      onclick={() => app.go({ view: 'instance', name: inst.name })}
    >
      <span class="ico"><InstanceIcon name={inst.name} icon={inst.icon} size={labels ? 32 : 42} /></span>
      {#if labels}<span class="text">{inst.name}</span>{/if}
    </button>
  {/each}
  </div>
  <button class="nav" class:active={view === 'settings'} aria-label={t('Impostazioni')} title={labels ? undefined : t('Impostazioni')} onclick={() => app.go({ view: 'settings' })}>
    <Icon name="settings" size={22} />{#if labels}<span class="text">{t('Impostazioni')}</span>{/if}
  </button>
</nav>

<style>
  .sidebar { flex: 0 0 76px; display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 14px 0; background: var(--panel); border-right: 1px solid var(--line); transition: flex-basis .25s var(--ease); }
  .sidebar.labels { flex-basis: 210px; align-items: stretch; padding: 14px 12px; gap: 4px; }
  .account-wrap { position: relative; }
  .avatar-btn { display: flex; align-items: center; gap: 10px; padding: 0; border: 0; background: none; border-radius: 14px; min-width: 0; width: 100%; }
  .labels .avatar-btn { padding: 4px; }
  .labels .avatar-btn:hover { background: var(--surface-2); }
  .avatar { width: 48px; height: 48px; flex: none; border-radius: 14px; border: 2px solid var(--accent); background: var(--accent-soft); font-weight: 700; font-size: 17px; font-family: var(--display); display: flex; align-items: center; justify-content: center; }
  .labels .avatar { width: 40px; height: 40px; border-radius: 12px; }
  .labels .avatar :global(.head), .labels .avatar :global(.letter) { width: 32px !important; height: 32px !important; }
  .text { font-weight: 600; font-size: 13.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-width: 0; text-align: left; }
  .name { font-size: 14px; }
  .sep { width: 32px; height: 1px; background: var(--line); margin: 6px 0; align-self: center; }
  .labels .sep { width: 100%; }
  .nav { width: 48px; height: 48px; border-radius: 14px; border: 0; background: transparent; color: var(--muted); display: flex; align-items: center; justify-content: center; gap: 12px; transition: background .2s ease, color .2s ease, transform .15s ease; }
  .labels .nav { width: 100%; height: 42px; justify-content: flex-start; padding: 0 12px; border-radius: var(--radius-sm); }
  .nav:active { transform: scale(.92); }
  .labels .nav:active { transform: scale(.98); }
  .nav:hover { background: var(--surface-2); color: var(--text); }
  .nav.active { background: var(--accent); color: #fff; }
  .shortcut { padding: 0; border: 0; background: transparent; border-radius: 13px; position: relative; display: flex; align-items: center; gap: 10px; color: var(--text-2); transition: transform .18s var(--ease), background .2s ease; }
  .shortcut:hover { transform: scale(1.07); }
  .labels .shortcut { padding: 4px 8px; border-radius: var(--radius-sm); }
  .labels .shortcut:hover { transform: none; background: var(--surface-2); }
  .ico { display: flex; position: relative; border-radius: 13px; }
  .shortcut.current .ico { outline: 2px solid var(--accent); outline-offset: 2px; }
  .shortcut.running .ico::after { content: ''; position: absolute; right: -2px; bottom: -2px; width: 12px; height: 12px; border-radius: 6px; background: var(--success); border: 2px solid var(--c-panel); }
  .list { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 4px 0; width: 100%; }
  .labels .list { align-items: stretch; gap: 4px; }
</style>
