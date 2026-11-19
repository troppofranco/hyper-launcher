<script lang="ts">
  // Server Minecraft ospitati su questo computer: elenco, avvio e creazione.
  import { api, type ServerInfo } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { t } from '../lib/i18n.svelte';
  import Icon from '../components/Icon.svelte';
  import InstanceIcon from '../components/InstanceIcon.svelte';
  import NewServerDialog from '../components/server/NewServerDialog.svelte';
  import { copyText, gb, kindLabel, poll } from '../components/server/util';

  let servers = $state<ServerInfo[] | null>(null);
  let newOpen = $state(false);
  /** Server con un avvio o uno spegnimento in corso. */
  let busy = $state<Record<string, boolean>>({});
  let failed = false;

  async function refresh() {
    try {
      servers = await api.serverList();
      failed = false;
    } catch (e) {
      // Un solo avviso, non uno ogni 3 secondi.
      if (!failed) app.error(e);
      failed = true;
      servers ??= [];
    }
  }

  $effect(() => poll(refresh, 3000));

  async function toggle(s: ServerInfo) {
    busy[s.name] = true;
    try {
      await (s.running ? api.serverStop(s.name) : api.serverStart(s.name));
      await refresh();
    } catch (e) {
      app.error(e);
    } finally {
      delete busy[s.name];
    }
  }
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1 class="page-title">{t('Server')}</h1>
      <p class="muted">{t('I tuoi server Minecraft, ospitati su questo computer.')}</p>
    </div>
    <button class="btn primary" onclick={() => (newOpen = true)}><Icon name="plus" size={18} /> {t('Nuovo server')}</button>
  </div>

  {#if servers === null}
    <div class="grid"><div class="card skeleton"></div><div class="card skeleton"></div></div>
  {:else if !servers.length}
    <div class="card empty enter">
      <span class="empty-icon"><Icon name="server" size={28} /></span>
      <h2>{t('Nessun server ancora')}</h2>
      <p class="muted">{t('Crea un server per giocare con gli amici: il launcher scarica tutto e lo tiene acceso per te.')}</p>
      <button class="btn primary" onclick={() => (newOpen = true)}><Icon name="plus" size={18} /> {t('Nuovo server')}</button>
    </div>
  {:else}
    <div class="grid">
      {#each servers as s, i (s.name)}
        <section class="card server enter" style:--i={i} aria-label={s.name}>
          <div class="top">
            <InstanceIcon name={s.name} icon={s.icon} size={48} />
            <span class="meta">
              <span class="name">{s.name}</span>
              <span class="sub">{kindLabel({ ...s, build: null })} · {gb(s.memoryMb)}{s.keepRunning ? ` · ${t('resta acceso')}` : ''}</span>
            </span>
            <span class="pill" class:on={s.running}>{s.running ? t('Acceso') : t('Spento')}</span>
          </div>
          <div class="bottom">
            <span class="info">
              {#if s.running}<span>{t('{n} / {max} giocatori', { n: s.players, max: s.maxPlayers })}</span><span class="dot">·</span>{/if}
              <span class="addr">{s.address}</span>
              <button class="link" aria-label={t('Copia l’indirizzo di {nome}', { nome: s.name })} onclick={() => copyText(s.address)}>{t('Copia')}</button>
            </span>
            <span class="actions">
              <button class="btn small" onclick={() => app.go({ view: 'server', name: s.name })}>{t('Apri')}</button>
              {#if s.running}
                <button class="btn small danger" disabled={busy[s.name]} onclick={() => toggle(s)}>{t('Ferma')}</button>
              {:else}
                <button class="btn small primary" disabled={busy[s.name]} onclick={() => toggle(s)}>{t('Avvia')}</button>
              {/if}
            </span>
          </div>
        </section>
      {/each}
    </div>
  {/if}

  <p class="card note enter" style:--i={3}>
    <Icon name="compass" size={18} />
    <span>{t('Gli amici nella tua stessa rete entrano con l’indirizzo mostrato. Per chi è fuori casa serve aprire la porta sul router (port forwarding).')}</span>
  </p>
</div>

{#if newOpen}
  <NewServerDialog existing={servers ?? []} onclose={() => (newOpen = false)} />
{/if}

<style>
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(340px, 1fr)); gap: 16px; }
  .skeleton { height: 140px; }
  .server { padding: 18px; display: flex; flex-direction: column; gap: 14px; transition: border-color .2s ease, box-shadow .22s ease; }
  .server:hover { border-color: var(--line-strong); box-shadow: 0 10px 26px var(--shadow); }
  .top { display: flex; align-items: center; gap: 14px; }
  .meta { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
  .name { font-weight: 600; font-size: 16px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sub { font-size: 12px; color: var(--muted); }
  .pill { flex: none; font-size: 12px; font-weight: 600; padding: 3px 10px; border-radius: 12px; background: var(--surface-2); color: var(--muted); display: inline-flex; align-items: center; gap: 6px; }
  .pill::before { content: ''; width: 7px; height: 7px; border-radius: 4px; border: 1.5px solid currentColor; box-sizing: border-box; }
  .pill.on { background: color-mix(in srgb, var(--success) 18%, transparent); color: var(--success-text); }
  .pill.on::before { background: currentColor; }
  .bottom { display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap; }
  .info { display: flex; align-items: center; gap: 6px; color: var(--text-2); min-width: 0; flex-wrap: wrap; }
  .dot { color: var(--faint); }
  .addr { font-family: var(--mono); font-size: 12px; user-select: text; }
  .link { border: 0; background: none; padding: 0 2px; color: var(--accent-line); font-size: 13px; font-weight: 600; }
  .link:hover { text-decoration: underline; }
  .actions { display: flex; gap: 8px; }
  .empty { padding: 36px 24px; display: flex; flex-direction: column; align-items: center; gap: 10px; text-align: center; }
  .empty h2 { font-size: 18px; font-weight: 600; }
  .empty p { margin: 0 0 6px; max-width: 440px; line-height: 1.5; }
  .empty-icon { width: 56px; height: 56px; border-radius: 16px; background: var(--accent-soft); color: var(--accent-line); display: flex; align-items: center; justify-content: center; }
  .note { margin: 0; padding: 14px 16px; display: flex; gap: 12px; align-items: flex-start; color: var(--text-2); line-height: 1.5; }
  .note :global(svg) { flex: none; margin-top: 2px; color: var(--muted); }
</style>
