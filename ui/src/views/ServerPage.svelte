<script lang="ts">
  // Pagina di un server ospitato: console, statistiche, giocatori,
  // impostazioni e file. Lo stato del server si rilegge ogni 3 secondi.
  import { api, type ServerInfo } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { t } from '../lib/i18n.svelte';
  import Icon from '../components/Icon.svelte';
  import InstanceIcon from '../components/InstanceIcon.svelte';
  import Console from '../components/server/Console.svelte';
  import StatsCharts from '../components/server/StatsCharts.svelte';
  import Players from '../components/server/Players.svelte';
  import ServerSettings from '../components/server/ServerSettings.svelte';
  import FileBrowser from '../components/server/FileBrowser.svelte';
  import Plugins from '../components/server/Plugins.svelte';
  import DeleteServerDialog from '../components/server/DeleteServerDialog.svelte';
  import { copyText, kindLabel, poll, uptime } from '../components/server/util';

  let { name }: { name: string } = $props();

  let server = $state<ServerInfo | null>(null);
  let missing = $state(false);
  let busy = $state<'start' | 'stop' | 'restart' | null>(null);
  let deleteOpen = $state(false);

  let failed = false;

  async function refresh() {
    try {
      const list = await api.serverList();
      server = list.find((s) => s.name === name) ?? null;
      missing = !server;
      failed = false;
    } catch (e) {
      // Un solo avviso, non uno ogni 3 secondi.
      if (!failed) app.error(e);
      failed = true;
    }
  }

  $effect(() => poll(refresh, 3000));

  async function act(kind: 'start' | 'stop' | 'restart') {
    busy = kind;
    try {
      await (kind === 'start' ? api.serverStart(name) : kind === 'stop' ? api.serverStop(name) : api.serverRestart(name));
      if (kind === 'restart') app.info_(t('Server riavviato.'));
      await refresh();
    } catch (e) {
      app.error(e);
    } finally {
      busy = null;
    }
  }

  const running = $derived(!!server?.running);

  // Sezioni della pagina, come le schede di un'istanza.
  type Tab = 'console' | 'players' | 'plugins' | 'files' | 'settings';
  let tab = $state<Tab>('console');
  const tabs = $derived<[Tab, string][]>([
    ['console', t('Console')],
    ['players', running && server?.players ? t('Giocatori · {n}', { n: server.players }) : t('Giocatori')],
    // I plugin esistono solo su Paper.
    ...(server?.kind === 'paper' ? [['plugins', t('Plugin')] as [Tab, string]] : []),
    ['files', t('File')],
    ['settings', t('Impostazioni')],
  ]);
  $effect(() => {
    if (server && server.kind !== 'paper' && tab === 'plugins') tab = 'console';
  });
</script>

<div class="page">
  <button class="back" onclick={() => app.go({ view: 'servers' })}><Icon name="back" size={16} /> {t('Server')}</button>

  {#if missing}
    <p class="muted">{t('Server non trovato.')}</p>
  {:else if !server}
    <div class="card skeleton head-skel"></div>
  {:else}
    <header class="head enter">
      <InstanceIcon name={server.name} size={52} />
      <div class="title">
        <div class="name-row">
          <h1 class="page-title">{server.name}</h1>
          <span class="pill" class:on={running}>{running ? `${t('Acceso')}${server.startedAt ? ` · ${uptime(server.startedAt)}` : ''}` : t('Spento')}</span>
        </div>
        <div class="sub">
          <span class="kind">{kindLabel(server)}</span>
          <span>{t('Indirizzo:')} <span class="addr">{server.address}</span></span>
        </div>
      </div>
      <div class="actions">
        <button class="btn" title={server.address} onclick={() => copyText(server!.address)}><Icon name="copy" size={16} /> {t('Copia IP')}</button>
        {#if running}
          <button class="btn" disabled={!!busy} onclick={() => act('restart')}><Icon name="refresh" size={16} /> {busy === 'restart' ? t('Riavvio…') : t('Riavvia')}</button>
          <button class="btn danger" disabled={!!busy} onclick={() => act('stop')}>{busy === 'stop' ? t('Spegnimento…') : t('Ferma')}</button>
        {:else}
          <button class="btn icon-only" disabled={!!busy} aria-label={t('Elimina server')} title={t('Elimina server')} onclick={() => (deleteOpen = true)}><Icon name="trash" size={16} /></button>
          <button class="btn primary" disabled={!!busy} onclick={() => act('start')}><Icon name="play" size={16} /> {busy === 'start' ? t('Avvio…') : t('Avvia')}</button>
        {/if}
      </div>
    </header>

    <div class="tabs enter" style:--i={1} role="tablist" aria-label={t('Sezioni del server')}>
      {#each tabs as [id, label] (id)}
        <button role="tab" aria-selected={tab === id} class:on={tab === id} onclick={() => (tab = id)}>{label}</button>
      {/each}
    </div>

    {#key tab}
      <div class="section enter" role="tabpanel">
        {#if tab === 'console'}
          <Console {name} {running} />
          <StatsCharts {name} {running} maxPlayers={server.maxPlayers} />
        {:else if tab === 'players'}
          <Players {name} {running} />
        {:else if tab === 'plugins'}
          <Plugins {name} version={server.version} {running} onrestart={() => act('restart')} />
        {:else if tab === 'files'}
          <FileBrowser {name} {running} />
        {:else}
          <ServerSettings {server} onchanged={refresh} onrestart={() => act('restart')} />
        {/if}
      </div>
    {/key}
  {/if}
</div>

{#if deleteOpen}
  <DeleteServerDialog {name} onclose={() => (deleteOpen = false)} ondeleted={() => app.go({ view: 'servers' })} />
{/if}

<style>
  .page { gap: 16px; }
  .back { align-self: flex-start; display: flex; align-items: center; gap: 6px; background: none; border: 0; color: var(--muted); padding: 0; font-size: 13px; }
  .back:hover { color: var(--text); }
  .head-skel { height: 80px; }
  .head { display: flex; align-items: center; gap: 16px; flex-wrap: wrap; }
  .title { flex: 1; min-width: 240px; display: flex; flex-direction: column; gap: 6px; }
  .name-row { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .name-row h1 { font-size: 26px; overflow-wrap: anywhere; }
  .pill { font-size: 12px; font-weight: 600; padding: 3px 10px; border-radius: 12px; background: var(--surface-2); color: var(--muted); display: inline-flex; align-items: center; gap: 6px; white-space: nowrap; }
  .pill::before { content: ''; width: 7px; height: 7px; border-radius: 4px; border: 1.5px solid currentColor; box-sizing: border-box; }
  .pill.on { background: color-mix(in srgb, var(--success) 18%, transparent); color: var(--success-text); }
  .pill.on::before { background: currentColor; }
  .sub { display: flex; gap: 10px; align-items: center; color: var(--muted); font-size: 13px; flex-wrap: wrap; }
  .kind { font-family: var(--mono); font-size: 12px; padding: 2px 8px; border-radius: 6px; background: var(--surface-2); color: var(--text-2); }
  .addr { font-family: var(--mono); color: var(--text); user-select: text; }

  .actions { display: flex; gap: 10px; flex-wrap: wrap; }
  .icon-only { width: 44px; padding: 0; }
  .icon-only:hover:not(:disabled) { color: var(--danger-text); border-color: color-mix(in srgb, var(--danger) 60%, transparent); }
  .tabs { display: flex; gap: 6px; border-bottom: 1px solid var(--line); overflow-x: auto; }
  .tabs button { height: 44px; padding: 0 16px; background: transparent; border: 0; border-bottom: 2px solid transparent; color: var(--muted); font-weight: 600; white-space: nowrap; transition: color .2s, border-color .2s; }
  .tabs button:hover { color: var(--text); }
  .tabs button.on { color: var(--text); border-bottom-color: var(--accent-line); }
  .section { display: flex; flex-direction: column; gap: 14px; }
</style>
