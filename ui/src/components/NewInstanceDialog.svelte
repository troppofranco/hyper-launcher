<script lang="ts">
  import { api, type BedrockStatus, type Edition, type LoaderKind, type Versions } from '../lib/api';
  import Segmented from './controls/Segmented.svelte';
  import { app } from '../lib/store.svelte';
  import Modal from './Modal.svelte';
  import { LOADERS } from '../lib/content';
  import InstanceIcon from './InstanceIcon.svelte';
  import IconPicker from './IconPicker.svelte';
  import Icon from './Icon.svelte';
  import { t } from '../lib/i18n.svelte';

  type LoaderChoice = 'vanilla' | LoaderKind;
  const CHOICES: [LoaderChoice, string][] = [['vanilla', 'Vanilla'], ...LOADERS];

  let name = $state('');
  let versions = $state<Versions | null>(null);
  let version = $state('');
  let showSnapshots = $state(false);
  let loader = $state<LoaderChoice>('vanilla');
  let launchAfter = $state(false);
  let loaderOk = $state<boolean | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  // Immagine scelta (PNG), applicata appena l'istanza è creata.
  let icon = $state<string | null>(null);
  let picking = $state(false);
  let edition = $state<Edition>('java');
  let bedrock = $state<BedrockStatus | null>(null);

  const options = $derived(
    (versions?.versions ?? []).filter((v) => showSnapshots || v.kind === 'release'),
  );
  const loaderName = $derived(CHOICES.find(([id]) => id === loader)?.[1] ?? 'Vanilla');
  const previewName = $derived(name.trim() || (edition === 'bedrock'
    ? 'Bedrock'
    : `${loader === 'vanilla' ? 'Minecraft' : loaderName} ${version}`));
  const canCreate = $derived(edition === 'bedrock'
    ? !!bedrock?.installed
    : !!version && !(loader !== 'vanilla' && loaderOk === false));

  // Stato del gioco Bedrock (installato? quale versione?) alla prima scelta.
  $effect(() => {
    if (edition !== 'bedrock' || bedrock) return;
    api.bedrockStatus().then((s) => (bedrock = s)).catch((e) => (error = String(e)));
  });

  $effect(() => {
    app.loadVersions().then((v) => {
      versions = v;
      version = v.latestRelease;
    }).catch((e) => (error = String(e)));
  });

  // Controlla che il loader scelto supporti la versione.
  $effect(() => {
    const [kind, game] = [loader, version];
    loaderOk = null;
    if (kind === 'vanilla' || !game) return;
    api.loaderVersions(kind, game)
      .then((list) => { if (game === version && kind === loader) loaderOk = list.length > 0; })
      .catch(() => (loaderOk = null));
  });

  function close() {
    app.newInstanceOpen = false;
  }

  async function create(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = null;
    try {
      const inst = edition === 'bedrock'
        ? await api.createInstance({ name: name.trim(), version: '', loader: null, edition })
        : await api.createInstance({ name: name.trim(), version, loader: loader === 'vanilla' ? null : loader });
      if (icon) await api.setInstanceIcon(inst.name, icon).catch((e) => app.error(e));
      await app.refreshInstances();
      close();
      app.go({ view: 'instance', name: inst.name });
      if (launchAfter) await app.play(inst.name);
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }
</script>

<Modal title={t('Nuova istanza')} onclose={close}>
  <form onsubmit={create} class="form">
    <Segmented bind:value={edition} label="Edizione" options={[['java', 'Java'], ['bedrock', 'Bedrock']]} />
    <div class="top">
      <button type="button" class="pick" title={t('Scegli un’immagine')} aria-label={t('Scegli un’immagine')} onclick={() => (picking = true)}>
        <InstanceIcon name={previewName} {icon} size={88} />
        <span class="pick-overlay"><Icon name="image" size={20} /></span>
      </button>
      <div class="fields">
        <label class="field">{t('Nome')}
          <input class="input" bind:value={name} placeholder={t('Lascia vuoto per un nome automatico')} maxlength="64" />
        </label>
        {#if edition === 'java'}
        <div class="field">
          <span class="row-label">
            <label for="version">{t('Versione di Minecraft')}</label>
            <label class="snap"><input type="checkbox" bind:checked={showSnapshots} /> {t('Mostra snapshot')}</label>
          </span>
          <select id="version" class="input" bind:value={version} disabled={!versions}>
            {#if !versions}<option>{t('Caricamento…')}</option>{/if}
            {#each options as v (v.id)}
              <option value={v.id}>{v.id}{v.id === versions?.latestRelease ? t(' (ultima)') : ''}</option>
            {/each}
          </select>
        </div>
        {:else if bedrock === null}
          <span class="hint">{t('Cerco Minecraft Bedrock…')}</span>
        {:else if bedrock.installed}
          <div class="detect">
            <span class="dot" aria-hidden="true"></span>
            <span class="detect-text">
              <span class="strong">{t('Minecraft Bedrock {version} trovato', { version: bedrock.version ?? '' })}</span>
              <span class="hint">{t('La versione è quella installata dal Microsoft Store e si aggiorna da sola.')}</span>
            </span>
          </div>
        {:else}
          <div class="detect missing">
            <span class="detect-text">
              <span class="strong">{t('Minecraft Bedrock non è installato')}</span>
              <span class="hint">{t('Installalo dal Microsoft Store, poi torna qui.')}</span>
            </span>
            <button type="button" class="btn small" onclick={() => api.openUrl('https://apps.microsoft.com/detail/9NBLGGH2JHXJ')}><Icon name="external" size={14} /> {t('Microsoft Store')}</button>
          </div>
        {/if}
      </div>
    </div>

    {#if edition === 'bedrock'}
      <p class="hint">{bedrock?.active
        ? t('Parte vuota: nessun mondo né pacchetto. Impostazioni e tasti arrivano dalla sincronizzazione, se attiva.')
        : t('È la prima installazione Bedrock: diventa quella con i mondi e i pacchetti che hai già nel gioco, senza spostare nulla.')}</p>
    {:else}
    <fieldset>
      <legend>{t('Mod loader')}</legend>
      <div class="loaders">
        {#each CHOICES as [id, label] (id)}
          <button type="button" class:on={loader === id} aria-pressed={loader === id} onclick={() => (loader = id)}>{label}</button>
        {/each}
      </div>
      {#if loader !== 'vanilla' && loaderOk === false}
        <span class="warn">{t('{loader} non supporta ancora {version}.', { loader: loaderName, version })}</span>
      {:else if loader === 'neoforge' || loader === 'forge'}
        <span class="hint">{t('Al primo avvio il launcher esegue l’installer ufficiale di {loader}: ci vuole qualche secondo in più.', { loader: loaderName })}</span>
      {/if}
    </fieldset>
    {/if}

    {#if error}<p class="error-box" role="alert">{error}</p>{/if}

    <div class="foot">
      <label class="check"><input type="checkbox" bind:checked={launchAfter} /> {t('Avvia dopo la creazione')}</label>
      <div class="actions">
        <button type="button" class="btn" onclick={close}>{t('Annulla')}</button>
        <button type="submit" class="btn primary" disabled={busy || !canCreate}>
          {busy ? t('Creazione…') : t('Crea istanza')}
        </button>
      </div>
    </div>
  </form>
</Modal>

{#if picking}
  <IconPicker removable={!!icon} onpick={(png) => (icon = png)} onclose={() => (picking = false)} />
{/if}

<style>
  .pick { position: relative; flex: none; padding: 0; border: 0; background: none; border-radius: 20px; display: block; }
  .pick-overlay { position: absolute; inset: 0; border-radius: 20px; background: rgba(8, 9, 12, .5); color: #fff; display: flex; align-items: center; justify-content: center; opacity: 0; transition: opacity .18s ease; }
  .pick:hover .pick-overlay, .pick:focus-visible .pick-overlay { opacity: 1; }
  .form { display: flex; flex-direction: column; gap: 18px; }
  .top { display: flex; gap: 16px; align-items: flex-start; }
  .fields { flex: 1; display: flex; flex-direction: column; gap: 14px; min-width: 0; }
  .row-label { display: flex; justify-content: space-between; align-items: center; }
  .snap { font-weight: 400; color: var(--muted); display: flex; gap: 6px; align-items: center; }
  fieldset { border: 0; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
  legend { font-weight: 600; font-size: 13px; margin-bottom: 8px; }
  .loaders { display: flex; gap: 8px; flex-wrap: wrap; }
  .loaders button { height: 40px; padding: 0 14px; border-radius: 10px; border: 1px solid var(--line-strong); background: var(--panel); font-weight: 600; font-size: 13px; }
  .loaders button.on { border-color: var(--accent-line); background: var(--accent-soft); }
  .warn { font-size: 12px; color: var(--warn-text); }
  .detect { display: flex; gap: 12px; align-items: center; padding: 12px 14px; border-radius: var(--radius-sm); background: color-mix(in srgb, var(--success) 14%, var(--panel)); border: 1px solid color-mix(in srgb, var(--success) 40%, transparent); }
  .detect.missing { background: var(--panel); border-color: var(--line-strong); }
  .dot { flex: none; width: 10px; height: 10px; border-radius: 5px; background: var(--success); }
  .detect-text { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .strong { font-weight: 600; }
  p.hint { margin: 0; line-height: 1.5; }
  .hint { font-size: 12px; color: var(--muted); }
  .foot { display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap; }
  .check { display: flex; align-items: center; gap: 8px; color: var(--text-2); }
  .actions { display: flex; gap: 10px; }
</style>
