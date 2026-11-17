<script lang="ts">
  import { app } from './lib/store.svelte';
  import Titlebar from './components/Titlebar.svelte';
  import Sidebar from './components/Sidebar.svelte';
  import NewsPanel from './components/NewsPanel.svelte';
  import UpdateBanner from './components/UpdateBanner.svelte';
  import Toast from './components/Toast.svelte';
  import NewInstanceDialog from './components/NewInstanceDialog.svelte';
  import AddAccountDialog from './components/AddAccountDialog.svelte';
  import Home from './views/Home.svelte';
  import Library from './views/Library.svelte';
  import InstancePage from './views/InstancePage.svelte';
  import Settings from './views/Settings.svelte';
  import Explore from './views/Explore.svelte';
  import ProjectPage from './views/ProjectPage.svelte';

  let ready = $state(false);
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

<div class="shell">
  <Titlebar />
  <UpdateBanner />
  <div class="body">
    <Sidebar />
    <main class="content scroll">
      {#if failed}
        <div class="page"><p class="fatal" role="alert">Impossibile avviare il launcher: {failed}</p></div>
      {:else if !ready}
        <div class="page"><p class="muted">Caricamento…</p></div>
      {:else if app.route.view === 'home'}
        <Home />
      {:else if app.route.view === 'library'}
        <Library />
      {:else if app.route.view === 'instance'}
        {#key app.route.name}<InstancePage name={app.route.name} />{/key}
      {:else if app.route.view === 'explore'}
        <Explore />
      {:else if app.route.view === 'project'}
        {#key app.route.id}<ProjectPage id={app.route.id} kind={app.route.kind} />{/key}
      {:else}
        <Settings />
      {/if}
    </main>
    {#if app.route.view === 'home' || app.route.view === 'library' || app.route.view === 'instance'}<NewsPanel />{/if}
  </div>
</div>

{#if app.newInstanceOpen}<NewInstanceDialog />{/if}
{#if app.addAccountOpen}<AddAccountDialog />{/if}
<Toast />

<style>
  .shell { height: 100%; display: flex; flex-direction: column; }
  .body { flex: 1; display: flex; min-height: 0; }
  .content { flex: 1; min-width: 0; }
  .fatal { color: #ff8a8e; user-select: text; }
</style>
