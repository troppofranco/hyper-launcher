<script lang="ts">
  import { api, type Settings } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import Icon from '../components/Icon.svelte';
  import { LANGUAGES, t } from '../lib/i18n.svelte';
  import { ACCENTS, THEMES } from '../lib/look';

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
      app.info_(t('Impostazioni salvate.'));
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
      app.updateDismissed = false;
      if (!app.update.current) app.info_(t('Questa è una versione di sviluppo: gli aggiornamenti automatici sono disattivati.'));
      else if (!app.update.available) app.info_(t('Hyper Launcher è aggiornato.'));
    } catch (e) {
      app.error(e);
    } finally {
      checking = false;
    }
  }
</script>

{#snippet toggle(on: boolean, label: string, hint: string, flip: () => void)}
  <div class="row">
    <div><div class="strong">{t(label)}</div><div class="muted small">{t(hint)}</div></div>
    <button class="switch" role="switch" aria-checked={on} aria-label={t(label)} onclick={flip}></button>
  </div>
{/snippet}

<div class="page settings">
  <div class="nav">
    <h1 class="page-title">{t('Impostazioni')}</h1>
    {#each sections as [id, label] (id)}
      <button class:on={section === id} onclick={() => (section = id)}>{t(label)}</button>
    {/each}
    <p class="muted small">Hyper Launcher {app.info?.version}</p>
  </div>

  {#if draft}
    <div class="card panel">
      {#if section === 'general'}
        <h2>{t('Generale')}</h2>
        {@render toggle(draft.minimizeOnLaunch, 'Riduci a icona quando il gioco parte', 'Il launcher torna visibile quando chiudi Minecraft.', () => draft && (draft.minimizeOnLaunch = !draft.minimizeOnLaunch))}
        {@render toggle(draft.showConsole, 'Mostra la console di Java', 'Apre la finestra nera con i messaggi di Java accanto al gioco. Il registro resta comunque nella scheda Console dell’istanza.', () => draft && (draft.showConsole = !draft.showConsole))}
      {:else if section === 'sync'}
        <h2>{t('Sincronizzazione')}</h2>
        <p class="muted">{t('Cambi un tasto o la distanza di rendering in un’istanza e lo ritrovi in tutte le altre. Le impostazioni scelte qui vengono copiate in ogni istanza prima di avviarla e salvate quando chiudi il gioco. Lingua, server recenti e resource pack restano di ogni istanza.')}</p>
        {@render toggle(draft.sync.enabled, 'Condividi le impostazioni tra le istanze', 'Si può escludere una singola istanza dalle sue impostazioni.', () => draft && (draft.sync.enabled = !draft.sync.enabled))}
        <div class="groups" class:off={!draft.sync.enabled}>
          {#each SYNC_GROUPS as [key, label, hint] (key)}
            <label class="group">
              <input type="checkbox" bind:checked={draft.sync[key]} disabled={!draft.sync.enabled} />
              <span><span class="strong">{t(label)}</span><span class="muted small">{t(hint)}</span></span>
            </label>
          {/each}
        </div>
      {:else if section === 'look'}
        <h2>{t('Aspetto')}</h2>
        <span class="strong">{t('Tema')}</span>
        <div class="choices">
          {#each THEMES as [id, label] (id)}
            <button class="choice" class:on={draft.theme === id} aria-pressed={draft.theme === id} onclick={() => draft && (draft.theme = id)}>
              <span class="mini {id}"></span>{t(label)}
            </button>
          {/each}
        </div>
        <span class="strong">{t('Colore accento')}</span>
        <div class="swatches">
          {#each ACCENTS as [color, label] (color)}
            <button class="swatch" class:on={draft.accent === color} style:background={color} aria-label={t(label)} title={t(label)} aria-pressed={draft.accent === color} onclick={() => draft && (draft.accent = color)}></button>
          {/each}
        </div>
        <label class="field">{t('Lingua')}
          <select class="input" bind:value={draft.language}>
            {#each LANGUAGES as [id, label] (id)}<option value={id}>{label}</option>{/each}
          </select>
        </label>
      {:else if section === 'java'}
        <h2>{t('Java e memoria')}</h2>
        <label class="field">{t('Memoria predefinita · {gb} GB', { gb: memoryGb })}
          <input type="range" min="1" max="32" step="1" value={memoryGb} oninput={(e) => draft && (draft.defaultMemoryMb = Number(e.currentTarget.value) * 1024)} />
        </label>
        <label class="field">{t('Java personalizzato (facoltativo)')}
          <input class="input" bind:value={draft.javaPath} placeholder={t('Vuoto = runtime ufficiale Mojang scaricato in automatico')} />
        </label>
      {:else if section === 'accounts'}
        <h2>{t('Account Microsoft')}</h2>
        <p class="muted">{t('Per il login Microsoft serve il client ID di un’app Azure ("Personal Microsoft accounts only", "Allow public client flows" = Yes) approvata da Mojang per la Minecraft API. I token restano solo nel portachiavi di Windows.')}</p>
        <label class="field">{t('Client ID Azure')}
          <input class="input mono" bind:value={draft.msaClientId} placeholder="00000000-0000-0000-0000-000000000000" />
        </label>
      {:else if section === 'storage'}
        <h2>{t('Archiviazione')}</h2>
        <div class="box">
          <div><div class="strong">{t('Cartella dati')}</div><div class="muted small path">{app.info?.dataDir}</div></div>
          <button class="btn" onclick={() => api.openFolder(null, 'data')}><Icon name="folder" size={16} /> {t('Apri cartella')}</button>
        </div>
      {:else}
        <h2>{t('Informazioni')}</h2>
        <p class="muted">Hyper Launcher {app.info?.version}{app.info?.dev ? ` (${t('versione di sviluppo')})` : ''}. {t('Gli aggiornamenti si installano da soli all’avvio.')}</p>
        <button class="btn primary self-start" disabled={checking} onclick={checkUpdate}><Icon name="refresh" size={16} /> {checking ? t('Controllo…') : t('Controlla aggiornamenti')}</button>
      {/if}

      {#if section !== 'storage' && section !== 'about'}
        <div class="save">
          <button class="btn" disabled={!dirty || saving} onclick={() => (draft = app.settings ? $state.snapshot(app.settings) : draft)}>{t('Annulla')}</button>
          <button class="btn primary" disabled={!dirty || saving} onclick={save}>{saving ? t('Salvataggio…') : t('Salva')}</button>
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
  .swatch { width: 40px; height: 40px; border-radius: 20px; border: 3px solid transparent; transition: transform .15s ease; }
  .swatch:hover { transform: scale(1.08); }
  .swatch.on { border-color: var(--text); }
  .choices { display: flex; gap: 10px; flex-wrap: wrap; }
  .choice { display: flex; align-items: center; gap: 10px; height: 48px; padding: 0 16px 0 10px; border-radius: 12px; border: 1px solid var(--line-strong); background: var(--panel); font-weight: 600; transition: border-color .18s ease, background .18s ease; }
  .choice.on { border-color: var(--accent-line); background: var(--accent-soft); }
  .mini { width: 30px; height: 30px; border-radius: 8px; border: 1px solid var(--line-strong); }
  .mini.dark { background: linear-gradient(135deg, #0f1115 50%, #1e222b 50%); }
  .mini.light { background: linear-gradient(135deg, #ffffff 50%, #e8eaef 50%); }
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
