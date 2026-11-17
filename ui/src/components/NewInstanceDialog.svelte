<script lang="ts">
  import { api, type LoaderKind, type Versions } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import Modal from './Modal.svelte';
  import { LOADERS } from '../lib/content';
  import InstanceIcon from './InstanceIcon.svelte';

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

  const options = $derived(
    (versions?.versions ?? []).filter((v) => showSnapshots || v.kind === 'release'),
  );
  const loaderName = $derived(CHOICES.find(([id]) => id === loader)?.[1] ?? 'Vanilla');
  const previewName = $derived(name.trim() || `${loader === 'vanilla' ? 'Minecraft' : loaderName} ${version}`);

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
      const inst = await api.createInstance({ name: name.trim(), version, loader: loader === 'vanilla' ? null : loader });
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

<Modal title="Nuova istanza" onclose={close}>
  <form onsubmit={create} class="form">
    <div class="top">
      <InstanceIcon name={previewName} size={88} />
      <div class="fields">
        <label class="field">Nome
          <input class="input" bind:value={name} placeholder="Lascia vuoto per un nome automatico" maxlength="64" />
        </label>
        <div class="field">
          <span class="row-label">
            <label for="version">Versione di Minecraft</label>
            <label class="snap"><input type="checkbox" bind:checked={showSnapshots} /> Mostra snapshot</label>
          </span>
          <select id="version" class="input" bind:value={version} disabled={!versions}>
            {#if !versions}<option>Caricamento…</option>{/if}
            {#each options as v (v.id)}
              <option value={v.id}>{v.id}{v.id === versions?.latestRelease ? ' (ultima)' : ''}</option>
            {/each}
          </select>
        </div>
      </div>
    </div>

    <fieldset>
      <legend>Mod loader</legend>
      <div class="loaders">
        {#each CHOICES as [id, label] (id)}
          <button type="button" class:on={loader === id} aria-pressed={loader === id} onclick={() => (loader = id)}>{label}</button>
        {/each}
      </div>
      {#if loader !== 'vanilla' && loaderOk === false}
        <span class="warn">{loaderName} non supporta ancora {version}.</span>
      {:else if loader === 'neoforge' || loader === 'forge'}
        <span class="hint">Al primo avvio il launcher esegue l’installer ufficiale di {loaderName}: ci vuole qualche secondo in più.</span>
      {/if}
    </fieldset>

    {#if error}<p class="error-box" role="alert">{error}</p>{/if}

    <div class="foot">
      <label class="check"><input type="checkbox" bind:checked={launchAfter} /> Avvia dopo la creazione</label>
      <div class="actions">
        <button type="button" class="btn" onclick={close}>Annulla</button>
        <button type="submit" class="btn primary" disabled={busy || !version || (loader !== 'vanilla' && loaderOk === false)}>
          {busy ? 'Creazione…' : 'Crea istanza'}
        </button>
      </div>
    </div>
  </form>
</Modal>

<style>
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
  .warn { font-size: 12px; color: #f0b37e; }
  .hint { font-size: 12px; color: var(--muted); }
  .foot { display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap; }
  .check { display: flex; align-items: center; gap: 8px; color: var(--text-2); }
  .actions { display: flex; gap: 10px; }
</style>
