<script lang="ts">
  // Giocatori del server: online, operatori, whitelist e bannati. Le azioni
  // sono comandi della console (op, kick, ban…), quindi solo a server acceso.
  import { api, type ServerPlayers } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import { t } from '../../lib/i18n.svelte';
  import InstanceIcon from '../InstanceIcon.svelte';
  import { poll } from './util';

  let { name, running }: { name: string; running: boolean } = $props();

  type Tab = 'online' | 'ops' | 'whitelist' | 'banned';
  // Comandi per aggiungere e togliere da ciascun elenco.
  const LISTS: Record<Exclude<Tab, 'online'>, { add: string; remove: string; addLabel: string; removeLabel: string; empty: string }> = {
    ops: { add: 'op', remove: 'deop', addLabel: 'Rendi operatore', removeLabel: 'Togli', empty: 'Nessun operatore.' },
    whitelist: { add: 'whitelist add', remove: 'whitelist remove', addLabel: 'Aggiungi', removeLabel: 'Togli', empty: 'La whitelist è vuota.' },
    banned: { add: 'ban', remove: 'pardon', addLabel: 'Banna', removeLabel: 'Perdona', empty: 'Nessun giocatore bannato.' },
  };
  // Solo caratteri validi in un nome di Minecraft: il nome finisce in un comando.
  const VALID = /^[A-Za-z0-9_]{1,16}$/;

  let tab = $state<Tab>('online');
  let players = $state<ServerPlayers | null>(null);
  let busy = $state(false);
  let newName = $state('');

  async function load() {
    players = await api.serverPlayers(name);
  }

  $effect(() => {
    if (running) return poll(load, 5000);
    load().catch((e) => app.error(e));
  });

  const isOp = (n: string) => !!players?.ops.some((o) => o.name.toLowerCase() === n.toLowerCase());

  async function run(line: string, done?: string) {
    busy = true;
    try {
      await api.serverCommand(name, line);
      if (done) app.info_(done);
      // Il server applica il comando subito, ma lo scrive sui file un attimo dopo.
      await new Promise((r) => setTimeout(r, 300));
      await load();
    } catch (e) {
      app.error(e);
    } finally {
      busy = false;
    }
  }

  async function add(e: SubmitEvent) {
    e.preventDefault();
    if (tab === 'online' || !VALID.test(newName)) return;
    await run(`${LISTS[tab].add} ${newName}`);
    newName = '';
  }

  const tabs = $derived<[Tab, string][]>([
    ['online', players ? t('Online · {n}', { n: players.online.length }) : t('Online')],
    ['ops', t('Operatori')],
    ['whitelist', t('Whitelist')],
    ['banned', t('Bannati')],
  ]);
  const entries = $derived(players && tab !== 'online' ? players[tab] : []);
  const validNew = $derived(VALID.test(newName));
</script>

<section class="card panel" aria-labelledby="players-{name}">
  <div class="head">
    <h2 id="players-{name}">{t('Giocatori')}</h2>
    <div class="tabs" role="tablist" aria-label={t('Elenchi dei giocatori')}>
      {#each tabs as [id, label] (id)}
        <button role="tab" aria-selected={tab === id} class:on={tab === id} onclick={() => (tab = id)}>{label}</button>
      {/each}
    </div>
  </div>

  {#if !running}<p class="hint small">{t('Avvia il server per modificare')}</p>{/if}

  <div class="rows" role="tabpanel">
    {#if !players}
      <div class="skeleton row-skel"></div>
    {:else if tab === 'online'}
      {#each players.online as p (p)}
        <div class="row">
          <InstanceIcon name={p} size={32} />
          <span class="who"><span class="pname">{p}</span>{#if isOp(p)}<span class="badge">OP</span>{/if}</span>
          {#if isOp(p)}
            <button class="mini" disabled={!running || busy} onclick={() => run(`deop ${p}`)}>{t('Deop')}</button>
          {:else}
            <button class="mini" disabled={!running || busy} onclick={() => run(`op ${p}`)}>{t('Op')}</button>
          {/if}
          <button class="mini" disabled={!running || busy} onclick={() => run(`kick ${p}`, t('{nome} espulso.', { nome: p }))}>{t('Kick')}</button>
          <button class="mini danger" disabled={!running || busy} onclick={() => run(`ban ${p}`, t('{nome} bannato.', { nome: p }))}>{t('Ban')}</button>
        </div>
      {:else}
        <p class="muted empty">{running ? t('Nessuno online in questo momento.') : t('Il server è spento.')}</p>
      {/each}
    {:else}
      {@const list = LISTS[tab]}
      {#each entries as p (p.uuid || p.name)}
        <div class="row">
          <InstanceIcon name={p.name} size={32} />
          <span class="who col">
            <span class="pname">{p.name}</span>
            {#if 'reason' in p && p.reason}<span class="muted small">{p.reason}</span>{/if}
          </span>
          <button class="mini" disabled={!running || busy} aria-label={`${t(list.removeLabel)}: ${p.name}`} onclick={() => run(`${list.remove} ${p.name}`)}>{t(list.removeLabel)}</button>
        </div>
      {:else}
        <p class="muted empty">{t(list.empty)}</p>
      {/each}
      <form class="add" onsubmit={add}>
        <input class="input" bind:value={newName} maxlength="16" placeholder={t('Nome del giocatore')} aria-label={t('Nome del giocatore')} disabled={!running} autocomplete="off" spellcheck="false" />
        <button type="submit" class="btn small" disabled={!running || busy || !validNew}>{t(list.addLabel)}</button>
      </form>
    {/if}
  </div>
</section>

<style>
  .panel { padding: 16px; display: flex; flex-direction: column; gap: 12px; min-width: 0; }
  .head { display: flex; justify-content: space-between; align-items: center; gap: 10px; flex-wrap: wrap; }
  h2 { font-size: 16px; font-weight: 600; }
  .tabs { display: flex; gap: 3px; padding: 3px; border-radius: var(--radius-sm); background: var(--panel); border: 1px solid var(--line); flex-wrap: wrap; }
  .tabs button { height: 28px; padding: 0 10px; border: 0; border-radius: calc(var(--radius-sm) - 3px); background: transparent; color: var(--text-2); font-size: 12px; font-weight: 600; transition: background .2s ease, color .2s ease; }
  .tabs button:hover { color: var(--text); }
  .tabs button.on { background: var(--accent); color: #fff; }
  .hint { margin: 0; color: var(--warn-text); }
  .rows { display: flex; flex-direction: column; gap: 8px; }
  .row { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border-radius: var(--radius-sm); background: var(--panel); }
  .row-skel { height: 48px; border-radius: var(--radius-sm); }
  .who { flex: 1; min-width: 0; display: flex; align-items: center; gap: 8px; }
  .who.col { flex-direction: column; align-items: flex-start; gap: 1px; }
  .pname { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .badge { font-size: 10px; font-weight: 700; padding: 2px 6px; border-radius: 5px; background: var(--accent-soft); color: var(--accent-line); }
  .mini { height: 30px; padding: 0 10px; border-radius: 8px; border: 1px solid var(--line-strong); background: transparent; color: var(--text-2); font-size: 12px; font-weight: 600; flex: none; }
  .mini:hover:not(:disabled) { background: var(--surface-2); color: var(--text); }
  .mini.danger { border-color: color-mix(in srgb, var(--danger) 55%, transparent); color: var(--danger-text); }
  .empty { margin: 6px 2px; }
  .add { display: flex; gap: 8px; margin-top: 4px; }
  .add .input { flex: 1; height: 36px; min-width: 0; }
</style>
