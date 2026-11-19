<script lang="ts">
  import { app } from './lib/store.svelte';
  import Titlebar from './components/Titlebar.svelte';
  import Sidebar from './components/Sidebar.svelte';
  import NewsPanel from './components/NewsPanel.svelte';
  import UpdateDialog from './components/UpdateDialog.svelte';
  import Toast from './components/Toast.svelte';
  import NewInstanceDialog from './components/NewInstanceDialog.svelte';
  import AddAccountDialog from './components/AddAccountDialog.svelte';
  import BlockedFilesDialog from './components/BlockedFilesDialog.svelte';
  import Home from './views/Home.svelte';
  import Library from './views/Library.svelte';
  import InstancePage from './views/InstancePage.svelte';
  import Settings from './views/Settings.svelte';
  import Explore from './views/Explore.svelte';
  import ProjectPage from './views/ProjectPage.svelte';
  import Customize from './views/Customize.svelte';
  import Servers from './views/Servers.svelte';
  import ServerPage from './views/ServerPage.svelte';
  import Onboarding from './components/Onboarding.svelte';
  import Backdrop from './components/Backdrop.svelte';
  import { t } from './lib/i18n.svelte';
  import { fly } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { ms } from './lib/motion.svelte';

  let ready = $state(false);
  const look = $derived(app.settings?.appearance);
  const routeKey = $derived(
    app.route.view === 'instance' ? `instance:${app.route.name}` : app.route.view === 'project' ? `project:${app.route.id}` : app.route.view === 'server' ? `server:${app.route.name}` : app.route.view,
  );
  let failed = $state<string | null>(null);

  $effect(() => {
    app.init().then(() => (ready = true)).catch((e) => (failed = String(e)));
  });

  // Niente menu contestuale del browser nell'app.
  function noContextMenu(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (!target.closest('input, textarea, pre')) e.preventDefault();
  }
</script>

<svelte:window oncontextmenu={noContextMenu} />

<Backdrop />
<div class="shell">
  <Titlebar />
  {#if ready && app.settings && !app.settings.setupDone}
    <Onboarding />
  {:else}
  <div class="body" class:right={look?.sidebarSide === 'right'}>
    <Sidebar />
    <main class="content scroll">
      {#if failed}
        <div class="page"><p class="fatal" role="alert">{t('Impossibile avviare il launcher:')} {failed}</p></div>
      {:else if !ready}
        <div class="page"><p class="muted">{t('Caricamento…')}</p></div>
      {:else}
        <!-- Ogni cambio di pagina entra con una breve dissolvenza dal basso. -->
        {#key routeKey}
          <div class="view" in:fly={{ y: 14, duration: ms(280), easing: cubicOut }}>
            {#if app.route.view === 'home'}
              <Home />
            {:else if app.route.view === 'library'}
              <Library />
            {:else if app.route.view === 'instance'}
              <InstancePage name={app.route.name} />
            {:else if app.route.view === 'explore'}
              <Explore />
            {:else if app.route.view === 'project'}
              <ProjectPage id={app.route.id} kind={app.route.kind} />
            {:else if app.route.view === 'servers'}
              <Servers />
            {:else if app.route.view === 'server'}
              <ServerPage name={app.route.name} />
            {:else if app.route.view === 'customize'}
              <Customize />
            {:else}
              <Settings />
            {/if}
          </div>
        {/key}
      {/if}
    </main>
    {#if look?.showNews !== false && (app.route.view === 'home' || app.route.view === 'library' || app.route.view === 'instance')}<NewsPanel />{/if}
  </div>
  {/if}
</div>

{#if app.newInstanceOpen}<NewInstanceDialog />{/if}
{#if app.addAccountOpen}<AddAccountDialog />{/if}
{#if app.blockedFiles?.length}<BlockedFilesDialog />{/if}
<UpdateDialog />
<Toast />

<style>
  .shell { position: relative; z-index: 1; height: 100%; display: flex; flex-direction: column; }
  /* Barra laterale e notizie sono pannelli arrotondati che "galleggiano"
     nella finestra, staccati dai bordi. */
  .body { flex: 1; display: flex; min-height: 0; padding: 0 10px 10px; gap: 10px; }
  .body > :global(.sidebar), .body > :global(.news) { border-radius: var(--radius); border: 1px solid var(--line); box-shadow: 0 10px 30px var(--shadow); }
  /* Barra laterale a destra: ultima della riga (dopo notizie e contenuti). */
  .body.right > :global(.sidebar) { order: 2; }
  .content { flex: 1; min-width: 0; }
  .view { min-height: 100%; }
  .fatal { color: var(--danger-text); user-select: text; }
</style>
