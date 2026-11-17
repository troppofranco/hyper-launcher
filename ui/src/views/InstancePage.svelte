<script lang="ts">
  import { untrack } from 'svelte';
  import { api, type Mod, type World } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { initial, megabytes, playtime, relativeDay } from '../lib/format';
  import Icon from '../components/Icon.svelte';
  import InstanceIcon from '../components/InstanceIcon.svelte';
  import PlayButton from '../components/PlayButton.svelte';
  import Modal from '../components/Modal.svelte';

  let { name }: { name: string } = $props();

  type Tab = 'mods' | 'worlds' | 'console' | 'settings';
  let tab = $state<Tab>('mods');
  let mods = $state<Mod[]>([]);
  let worlds = $state<World[]>([]);
  let modQuery = $state('');

  // Impostazioni istanza (modulo).
  let customMemory = $state(false);
  let memoryGb = $state(4);
  let javaPath = $state('');
  let jvmArgs = $state('');
  let saving = $state(false);
  let deleteOpen = $state(false);
  let deleteConfirm = $state('');

  const inst = $derived(app.instances.find((i) => i.name === name) ?? null);
  const launch = $derived(app.launches[name]);
  const log = $derived(app.logs[name] ?? []);
  const shownMods = $derived(mods.filter((m) => !modQuery.trim() || m.name.toLowerCase().includes(modQuery.trim().toLowerCase())));
  const status = $derived(launch?.phase === 'running' ? 'In gioco' : launch ? 'Installazione' : 'Pronta');

  // Ricarica i dati della scheda quando cambia istanza o scheda.
  $effect(() => {
    const n = name;
    if (tab === 'mods') api.listMods(n).then((m) => (mods = m)).catch((e) => app.error(e));
    if (tab === 'worlds') api.listWorlds(n).then((w) => (worlds = w)).catch((e) => app.error(e));
  });

  // Il modulo delle impostazioni riparte dai valori salvati solo quando si
  // apre la scheda: un aggiornamento dell'elenco istanze (es. a fine partita)
  // non deve cancellare quello che l'utente sta scrivendo.
  $effect(() => {
    if (tab !== 'settings') return;
    untrack(() => {
      if (!inst) return;
      customMemory = inst.memoryMb !== null;
      memoryGb = Math.round((inst.memoryMb ?? app.settings?.defaultMemoryMb ?? 4096) / 1024);
      javaPath = inst.javaPath ?? '';
      jvmArgs = inst.jvmArgs.join(' ');
    });
  });

  // Se l'istanza è vanilla la scheda Mod non serve: si parte dai mondi.
  $effect(() => {
    if (inst && !inst.loader && tab === 'mods') tab = 'worlds';
  });

  async function toggle(mod: Mod) {
    try {
      await api.setModEnabled(name, mod.file, !mod.enabled);
      mods = await api.listMods(name);
    } catch (e) {
      app.error(e);
    }
  }

  async function removeMod(mod: Mod) {
    try {
      await api.deleteMod(name, mod.file);
      mods = await api.listMods(name);
    } catch (e) {
      app.error(e);
    }
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    saving = true;
    try {
      await api.updateInstance(name, {
        memoryMb: customMemory ? memoryGb * 1024 : null,
        javaPath: javaPath.trim() || null,
        jvmArgs,
      });
      await app.refreshInstances();
      app.info_('Impostazioni dell’istanza salvate.');
    } catch (err) {
      app.error(err);
    } finally {
      saving = false;
    }
  }

  async function remove() {
    try {
      await api.deleteInstance(name);
      deleteOpen = false;
      app.go({ view: 'library' });
      await app.refreshInstances();
    } catch (e) {
      app.error(e);
    }
  }

  const tabs = $derived<[Tab, string][]>([
    ...(inst?.loader ? [['mods', `Mod · ${mods.length}`] as [Tab, string]] : []),
    ['worlds', 'Mondi'],
    ['console', 'Console'],
    ['settings', 'Impostazioni istanza'],
  ]);
</script>

{#if !inst}
  <div class="page"><p class="muted">Istanza non trovata.</p></div>
{:else}
  <div class="page">
    <button class="back" onclick={() => app.go({ view: 'library' })}><Icon name="back" size={16} /> Libreria</button>

    <section class="card hero">
      <div class="hero-main">
        <InstanceIcon name={inst.name} size={64} />
        <div class="title">
          <div class="sub">
            {#if inst.loader}<span class="loader">{inst.loader.name} {inst.loader.version}</span><span>·</span>{/if}
            <span>Minecraft {inst.version}</span>
          </div>
          <h1 class="page-title">{inst.name}</h1>
        </div>
        <PlayButton name={inst.name} size="large" />
      </div>
      <div class="facts">
        <div><span class="label">Stato</span><span class="value">{status}</span></div>
        <div><span class="label">Ultima partita</span><span class="value">{relativeDay(inst.lastPlayed)}</span></div>
        <div><span class="label">Tempo di gioco</span><span class="value">{playtime(inst.playtimeSecs)}</span></div>
      </div>
    </section>

    <div class="tabs" role="tablist" aria-label="Contenuto istanza">
      {#each tabs as [id, label] (id)}
        <button role="tab" aria-selected={tab === id} class:on={tab === id} onclick={() => (tab = id)}>{label}</button>
      {/each}
    </div>

    {#if tab === 'mods'}
      <div class="tools">
        <input class="input grow" type="search" aria-label="Cerca tra le mod installate" placeholder="Cerca tra le mod installate…" bind:value={modQuery} />
        <button class="btn" onclick={() => api.openFolder(name, 'mods')}><Icon name="folder" size={16} /> Apri cartella mod</button>
      </div>
      <p class="hint muted">Per aggiungere mod copia i file <code>.jar</code> nella cartella mod. Il catalogo Modrinth arriverà nella prossima fase.</p>
      <div class="card list">
        {#each shownMods as m (m.file)}
          <div class="mod-row" class:off={!m.enabled}>
            <span class="mod-ico">{initial(m.name)}</span>
            <span class="mod-meta"><span class="mod-name">{m.name}</span><span class="muted small">{m.file} · {megabytes(m.sizeBytes)}</span></span>
            <button class="switch" role="switch" aria-checked={m.enabled} aria-label="{m.enabled ? 'Disattiva' : 'Attiva'} {m.name}" onclick={() => toggle(m)}></button>
            <button class="icon-btn" aria-label="Elimina {m.name}" onclick={() => removeMod(m)}><Icon name="trash" size={18} /></button>
          </div>
        {:else}
          <p class="muted pad">{mods.length ? 'Nessuna mod corrisponde alla ricerca.' : 'Nessuna mod installata.'}</p>
        {/each}
      </div>
    {:else if tab === 'worlds'}
      <div class="tools"><button class="btn" onclick={() => api.openFolder(name, 'saves')}><Icon name="folder" size={16} /> Apri cartella mondi</button></div>
      <div class="worlds">
        {#each worlds as w (w.name)}
          <div class="card world"><span class="mod-name">{w.name}</span><span class="muted small">Ultimo accesso: {relativeDay(w.lastPlayed)}</span></div>
        {:else}
          <p class="muted">Nessun mondo ancora: crealo dal gioco.</p>
        {/each}
      </div>
    {:else if tab === 'console'}
      <!-- Una riga per elemento: a ogni nuova riga si aggiunge un nodo invece
           di ricostruire tutto il testo. -->
      <div class="console scroll" role="log">
        {#each log as line (line.id)}<div>{line.text}</div>{:else}<div class="muted">La console mostra il log del gioco quando l’istanza è avviata.</div>{/each}
      </div>
    {:else}
      <form class="card form" onsubmit={save}>
        <div class="setting">
          <div><div class="mod-name">Memoria dedicata</div><div class="muted small">Se spento usa la memoria predefinita delle impostazioni ({Math.round((app.settings?.defaultMemoryMb ?? 4096) / 1024)} GB).</div></div>
          <button type="button" class="switch" role="switch" aria-checked={customMemory} aria-label="Memoria dedicata" onclick={() => (customMemory = !customMemory)}></button>
        </div>
        {#if customMemory}
          <label class="field">Memoria massima · {memoryGb} GB
            <input type="range" min="1" max="32" step="1" bind:value={memoryGb} />
          </label>
        {/if}
        <label class="field">Java (facoltativo)
          <input class="input" bind:value={javaPath} placeholder="Vuoto = automatico (runtime ufficiale Mojang)" />
        </label>
        <label class="field">Argomenti JVM aggiuntivi
          <input class="input" bind:value={jvmArgs} placeholder="es. -XX:+UseZGC" />
        </label>
        <div class="form-actions">
          <button type="button" class="btn danger" disabled={!!launch} onclick={() => { deleteConfirm = ''; deleteOpen = true; }}><Icon name="trash" size={16} /> Elimina istanza</button>
          <span class="actions">
            <button type="button" class="btn" onclick={() => api.openFolder(name, 'game')}><Icon name="folder" size={16} /> Apri cartella</button>
            <button type="submit" class="btn primary" disabled={saving}>Salva</button>
          </span>
        </div>
      </form>
    {/if}
  </div>

  {#if deleteOpen}
    <Modal title="Eliminare l’istanza?" onclose={() => (deleteOpen = false)} width={480}>
      <p class="confirm-text">Verranno cancellati per sempre i mondi, le mod e le impostazioni di <strong>{inst.name}</strong>. Non si può annullare.</p>
      <label class="field">Scrivi il nome dell’istanza per confermare
        <input class="input" bind:value={deleteConfirm} placeholder={inst.name} />
      </label>
      <div class="actions end">
        <button class="btn" onclick={() => (deleteOpen = false)}>Annulla</button>
        <button class="btn danger" disabled={deleteConfirm !== inst.name} onclick={remove}>Elimina definitivamente</button>
      </div>
    </Modal>
  {/if}
{/if}

<style>
  .back { align-self: flex-start; display: flex; align-items: center; gap: 6px; background: none; border: 0; color: var(--muted); padding: 0; font-size: 13px; }
  .back:hover { color: var(--text); }
  .hero { overflow: hidden; }
  .hero-main { display: flex; align-items: center; gap: 18px; padding: 22px; flex-wrap: wrap; }
  .title { flex: 1; min-width: 200px; }
  .sub { display: flex; gap: 8px; font-size: 12px; color: var(--muted); }
  .loader { color: #e2b97e; }
  .title h1 { margin-top: 4px; }
  .facts { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); border-top: 1px solid var(--line); }
  .facts div { padding: 14px 22px; display: flex; flex-direction: column; gap: 4px; }
  .facts div + div { border-left: 1px solid var(--line); }
  .value { font-weight: 600; }
  .tabs { display: flex; gap: 6px; border-bottom: 1px solid var(--line); }
  .tabs button { height: 44px; padding: 0 16px; background: transparent; border: 0; border-bottom: 2px solid transparent; color: var(--muted); font-weight: 600; }
  .tabs button.on { color: var(--text); border-bottom-color: var(--accent-line); }
  .tools { display: flex; gap: 10px; flex-wrap: wrap; }
  .grow { flex: 1 1 260px; }
  .hint { margin: -8px 0 0; font-size: 12px; }
  code { font-family: var(--mono); font-size: 12px; }
  .list { overflow: hidden; }
  .mod-row { display: flex; align-items: center; gap: 14px; padding: 12px 16px; border-bottom: 1px solid #1f232c; }
  .mod-row:last-child { border-bottom: 0; }
  .mod-row.off .mod-meta { opacity: .55; }
  .mod-ico { width: 36px; height: 36px; border-radius: 8px; background: #252a33; display: flex; align-items: center; justify-content: center; font-weight: 700; color: var(--text-2); flex: 0 0 auto; }
  .mod-meta { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .mod-name { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pad { padding: 16px; margin: 0; }
  .icon-btn { width: 36px; height: 36px; border-radius: 8px; border: 0; background: transparent; color: var(--muted); display: flex; align-items: center; justify-content: center; }
  .icon-btn:hover { color: #ff8a8e; background: var(--surface-2); }
  .worlds { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 12px; }
  .world { padding: 16px; display: flex; flex-direction: column; gap: 4px; }
  .console { min-height: 320px; max-height: 460px; background: var(--bg-deep); border: 1px solid var(--line); border-radius: var(--radius); padding: 16px; font-family: var(--mono); font-size: 12px; line-height: 1.7; color: var(--text-2); white-space: pre-wrap; word-break: break-word; user-select: text; }
  .form { padding: 22px; display: flex; flex-direction: column; gap: 16px; max-width: 680px; }
  .setting { display: flex; justify-content: space-between; align-items: center; gap: 16px; }
  .form-actions { display: flex; justify-content: space-between; gap: 10px; flex-wrap: wrap; margin-top: 6px; }
  .actions { display: flex; gap: 10px; }
  .actions.end { justify-content: flex-end; }
  .confirm-text { margin: 0; line-height: 1.5; }
</style>
