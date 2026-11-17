<script lang="ts">
  import { api, type Settings } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import Icon from '../components/Icon.svelte';

  type Section = 'general' | 'sync' | 'look' | 'java' | 'accounts' | 'storage' | 'about';
  const sections: [Section, string][] = [
    ['general', 'Generale'],
    ['sync', 'Sincronizzazione'],
    ['look', 'Aspetto'],
    ['java', 'Java e memoria'],
    ['accounts', 'Account Microsoft'],
    ['storage', 'Archiviazione'],
    ['about', 'Informazioni'],
  ];
  const SYNC_GROUPS: ['keybinds' | 'video' | 'audio' | 'controls', string, string][] = [
    ['keybinds', 'Tasti', 'Tutti i comandi assegnati, comprese le mod che usano i tasti di Minecraft.'],
    ['video', 'Video', 'Distanza di rendering, FOV, luminosità, scala dell’interfaccia, FPS, nuvole e particelle.'],
    ['audio', 'Audio', 'Volumi per categoria, audio direzionale e sottotitoli.'],
    ['controls', 'Controlli', 'Sensibilità del mouse, assi invertiti, salto automatico, accovacciarsi e scattare con un tasto.'],
  ];
  const ACCENTS: [string, string][] = [
    ['#6e5ae6', 'Viola'],
    ['#2f6fe4', 'Blu'],
    ['#12856f', 'Verde acqua'],
    ['#c2571a', 'Arancio'],
    ['#c0306a', 'Lampone'],
  ];

  let section = $state<Section>('general');
  let draft = $state<Settings | null>(null);
  let saving = $state(false);
  let checking = $state(false);

  $effect(() => {
    if (app.settings && !draft) draft = $state.snapshot(app.settings);
  });

  const memoryGb = $derived(draft ? Math.round(draft.defaultMemoryMb / 1024) : 4);
  const dirty = $derived(!!draft && !!app.settings && JSON.stringify(draft) !== JSON.stringify(app.settings));

  async function save() {
    if (!draft) return;
    saving = true;
    try {
      await api.saveSettings(draft);
      app.applySettings($state.snapshot(draft));
      app.info_('Impostazioni salvate.');
    } catch (e) {
      app.error(e);
    } finally {
      saving = false;
    }
  }

  async function checkUpdate() {
    checking = true;
    try {
      app.update = await api.checkUpdate();
      if (!app.update.current) app.info_('Questa è una build di sviluppo: gli aggiornamenti automatici sono disattivati.');
      else if (!app.update.available) app.info_('Hyper Launcher è aggiornato.');
    } catch (e) {
      app.error(e);
    } finally {
      checking = false;
    }
  }
</script>

<div class="page settings">
  <div class="nav">
    <h1 class="page-title">Impostazioni</h1>
    {#each sections as [id, label] (id)}
      <button class:on={section === id} onclick={() => (section = id)}>{label}</button>
    {/each}
    <p class="muted small">Hyper Launcher {app.info?.version}{app.info?.build ? ` · build ${app.info.build}` : ' · sviluppo'}</p>
  </div>

  {#if draft}
    <div class="card panel">
      {#if section === 'general'}
        <h2>Generale</h2>
        <div class="row">
          <div><div class="strong">Riduci a icona quando il gioco parte</div><div class="muted small">Il launcher torna visibile quando chiudi Minecraft.</div></div>
          <button class="switch" role="switch" aria-checked={draft.minimizeOnLaunch} aria-label="Riduci a icona quando il gioco parte" onclick={() => draft && (draft.minimizeOnLaunch = !draft.minimizeOnLaunch)}></button>
        </div>
      {:else if section === 'sync'}
        <h2>Sincronizzazione</h2>
        <p class="muted">Cambi un tasto o la distanza di rendering in un’istanza e lo ritrovi in tutte le altre. Le impostazioni scelte qui vengono copiate in ogni istanza prima di avviarla e salvate quando chiudi il gioco. Lingua, server recenti e resource pack restano di ogni istanza.</p>
        <div class="row">
          <div><div class="strong">Condividi le impostazioni tra le istanze</div><div class="muted small">Si può escludere una singola istanza dalle sue impostazioni.</div></div>
          <button class="switch" role="switch" aria-checked={draft.sync.enabled} aria-label="Condividi le impostazioni tra le istanze" onclick={() => draft && (draft.sync.enabled = !draft.sync.enabled)}></button>
        </div>
        <div class="groups" class:off={!draft.sync.enabled}>
          {#each SYNC_GROUPS as [key, label, hint] (key)}
            <label class="group">
              <input type="checkbox" bind:checked={draft.sync[key]} disabled={!draft.sync.enabled} />
              <span><span class="strong">{label}</span><span class="muted small">{hint}</span></span>
            </label>
          {/each}
        </div>
      {:else if section === 'look'}
        <h2>Aspetto</h2>
        <span class="strong">Colore accento</span>
        <div class="swatches">
          {#each ACCENTS as [color, label] (color)}
            <button class="swatch" class:on={draft.accent === color} style:background={color} aria-label={label} aria-pressed={draft.accent === color} onclick={() => draft && (draft.accent = color)}></button>
          {/each}
        </div>
      {:else if section === 'java'}
        <h2>Java e memoria</h2>
        <label class="field">Memoria predefinita · {memoryGb} GB
          <input type="range" min="1" max="32" step="1" value={memoryGb} oninput={(e) => draft && (draft.defaultMemoryMb = Number(e.currentTarget.value) * 1024)} />
        </label>
        <label class="field">Java personalizzato (facoltativo)
          <input class="input" bind:value={draft.javaPath} placeholder="Vuoto = runtime ufficiale Mojang scaricato in automatico" />
        </label>
      {:else if section === 'accounts'}
        <h2>Account Microsoft</h2>
        <p class="muted">Per il login Microsoft serve il client ID di un'app Azure ("Personal Microsoft accounts only", "Allow public client flows" = Yes) approvata da Mojang per la Minecraft API. I token restano solo nel portachiavi di Windows.</p>
        <label class="field">Client ID Azure
          <input class="input mono" bind:value={draft.msaClientId} placeholder="00000000-0000-0000-0000-000000000000" />
        </label>
      {:else if section === 'storage'}
        <h2>Archiviazione</h2>
        <div class="box">
          <div><div class="strong">Cartella dati</div><div class="muted small path">{app.info?.dataDir}</div></div>
          <button class="btn" onclick={() => api.openFolder(null, 'data')}><Icon name="folder" size={16} /> Apri cartella</button>
        </div>
      {:else}
        <h2>Informazioni</h2>
        <p class="muted">Hyper Launcher {app.info?.version}{app.info?.build ? ` · build ${app.info.build}` : ' (build di sviluppo)'}. Gli aggiornamenti si installano da soli all'avvio.</p>
        <button class="btn primary self-start" disabled={checking} onclick={checkUpdate}><Icon name="refresh" size={16} /> {checking ? 'Controllo…' : 'Controlla aggiornamenti'}</button>
      {/if}

      {#if section !== 'storage' && section !== 'about'}
        <div class="save">
          <button class="btn" disabled={!dirty || saving} onclick={() => (draft = app.settings ? $state.snapshot(app.settings) : draft)}>Annulla</button>
          <button class="btn primary" disabled={!dirty || saving} onclick={save}>{saving ? 'Salvataggio…' : 'Salva'}</button>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .settings { flex-direction: row; align-items: flex-start; flex-wrap: wrap; }
  .nav { flex: 0 0 240px; display: flex; flex-direction: column; gap: 4px; }
  .nav h1 { margin-bottom: 12px; font-size: 24px; }
  .nav button { height: 44px; text-align: left; padding: 0 14px; border-radius: 10px; border: 1px solid transparent; background: transparent; color: var(--text-2); font-weight: 600; }
  .nav button:hover { background: var(--surface-2); }
  .nav button.on { border-color: var(--accent-line); background: var(--accent-soft); color: var(--text); }
  .nav p { margin-top: 16px; }
  .panel { flex: 1 1 420px; min-width: 0; padding: 24px; display: flex; flex-direction: column; gap: 18px; }
  h2 { font-size: 18px; font-weight: 600; }
  p { margin: 0; line-height: 1.5; }
  .strong { font-weight: 600; }
  .row { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 4px 0; }
  .swatches { display: flex; gap: 10px; }
  .swatch { width: 40px; height: 40px; border-radius: 20px; border: 3px solid transparent; }
  .swatch.on { border-color: #fff; }
  .mono { font-family: var(--mono); }
  .box { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 14px; border-radius: 10px; background: var(--panel); flex-wrap: wrap; }
  .path { user-select: text; word-break: break-all; }
  .self-start { align-self: flex-start; }
  .groups { display: flex; flex-direction: column; gap: 4px; }
  .groups.off { opacity: .5; }
  .group { display: flex; gap: 12px; align-items: flex-start; padding: 10px 12px; border-radius: 10px; background: var(--panel); }
  .group input { margin-top: 3px; accent-color: var(--accent); width: 16px; height: 16px; }
  .group > span { display: flex; flex-direction: column; gap: 2px; }
  .save { display: flex; justify-content: flex-end; gap: 10px; border-top: 1px solid var(--line); padding-top: 16px; }
</style>
