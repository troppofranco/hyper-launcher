<script lang="ts">
  // Creazione guidata di un server in tre passi: tipo e versione, risorse,
  // EULA di Mojang. Durante la creazione mostra l'avanzamento del download.
  import { api, on, type SearchHit, type ServerInfo, type ServerKind, type ServerProgressEvent } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import { t } from '../../lib/i18n.svelte';
  import Modal from '../Modal.svelte';
  import ProgressBar from '../ProgressBar.svelte';
  import Toggle from '../controls/Toggle.svelte';
  import { fly } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { moving, ms } from '../../lib/motion.svelte';
  import { freePort, VALID_SERVER_NAME } from './util';

  let { existing, onclose }: { existing: ServerInfo[]; onclose: () => void } = $props();

  const STEPS = ['Tipo', 'Risorse', 'EULA'] as const;
  /** Un tipo di server oppure "da modpack" (tipo e versioni li dà il pack). */
  type Choice = ServerKind | 'modpack';
  const KINDS: [Choice, string, string][] = [
    ['vanilla', 'Vanilla', 'Il server ufficiale di Mojang, senza aggiunte.'],
    ['paper', 'Paper', 'plugin Bukkit/Spigot, prestazioni migliori'],
    ['fabric', 'Fabric', 'mod leggere e moderne'],
    ['neoforge', 'NeoForge', 'grandi mod moderne'],
    ['forge', 'Forge', 'mod e modpack classici'],
    ['modpack', 'Da modpack', 'le stesse mod dei tuoi amici'],
  ];
  const LABELS: Record<Choice, string> = { vanilla: 'Vanilla', paper: 'Paper', fabric: 'Fabric', neoforge: 'NeoForge', forge: 'Forge', modpack: 'Modpack' };
  const STAGES: Record<ServerProgressEvent['stage'], string> = {
    download: 'Download del server',
    java: 'Download di Java',
    setup: 'Preparazione',
  };

  const taken = (n: string) => existing.some((s) => s.name.toLowerCase() === n.toLowerCase());
  function defaultName() {
    if (!taken('Server')) return 'Server';
    let n = 2;
    while (taken(`Server ${n}`)) n++;
    return `Server ${n}`;
  }
  const initialPort = () => freePort(existing);

  let step = $state(0);
  let dir = $state(1);
  let kind = $state<Choice>('paper');
  let versions = $state<string[] | null>(null);
  let version = $state('');
  // Loader (Fabric, NeoForge, Forge): versioni per la versione del gioco.
  const loaderKind = $derived(kind === 'fabric' || kind === 'neoforge' || kind === 'forge' ? kind : null);
  let loaderVersions = $state<string[] | null>(null);
  let loaderVersion = $state('');
  // Modpack: cercato su Modrinth oppure un file .mrpack dal PC.
  let packQuery = $state('');
  let packHits = $state<SearchHit[] | null>(null);
  let pack = $state<{ project: string | null; file: string | null; name: string; detail: string } | null>(null);
  // Valori iniziali presi una volta sola dall'elenco aperto: chi crea un
  // server nel frattempo non cambia ciò che l'utente sta scrivendo.
  let name = $state(defaultName());
  let memoryGb = $state(4);
  let port = $state(initialPort());
  let keepRunning = $state(true);
  let autostart = $state(false);
  let eula = $state(false);
  let creating = $state(false);
  let progress = $state<ServerProgressEvent | null>(null);
  let error = $state<string | null>(null);

  const trimmed = $derived(name.trim());
  const nameError = $derived(
    !trimmed ? t('Scegli un nome.')
    : !VALID_SERVER_NAME.test(trimmed) ? t('Solo lettere senza accenti, numeri, spazi, - e _ (massimo 32).')
    : taken(trimmed) ? t('Esiste già un server con questo nome.')
    : null,
  );
  const portClash = $derived(existing.find((s) => s.port === port) ?? null);
  const portOk = $derived(Number.isInteger(port) && port >= 1024 && port <= 65535);
  const typeReady = $derived(kind === 'modpack' ? !!pack : !!version && (!loaderKind || !!loaderVersion));
  const canNext = $derived(step === 0 ? !nameError && typeReady : step === 1 ? portOk && !portClash : eula);
  /** "Fabric 1.21.4 · 0.16.10", "Paper 1.21.4", "Cobblemon (modpack)". */
  const summary = $derived(kind === 'modpack'
    ? `${pack?.name ?? ''}${pack?.detail ? ` · ${pack.detail}` : ''}`
    : `${LABELS[kind]} ${version}${loaderVersion && loaderKind ? ` · ${loaderVersion}` : ''}`);

  // Versioni del tipo scelto (si parte dalla più recente).
  $effect(() => {
    const k = kind;
    versions = null;
    version = '';
    if (k === 'modpack') return;
    api.serverVersions(k)
      .then((list) => { if (k === kind) { versions = list; version = list[0] ?? ''; } })
      .catch((e) => { versions = []; error = t(String(e)); });
  });

  // Versioni del loader per la versione del gioco scelta.
  $effect(() => {
    const [l, v] = [loaderKind, version];
    loaderVersions = null;
    loaderVersion = '';
    if (!l || !v) return;
    api.loaderVersions(l, v)
      .then((list) => { if (l === loaderKind && v === version) { loaderVersions = list; loaderVersion = list[0] ?? ''; } })
      .catch((e) => { loaderVersions = []; error = t(String(e)); });
  });

  // Modpack su Modrinth (i più scaricati finché non si scrive).
  $effect(() => {
    if (kind !== 'modpack') return;
    const text = packQuery;
    const timer = setTimeout(() => {
      api.search({ text, kind: 'modpack', instance: null, gameVersion: null, loaders: ['fabric', 'neoforge', 'forge'], categories: [], sort: text.trim() ? 'relevance' : 'downloads', offset: 0, serverSide: true })
        .then((r) => { if (text === packQuery) packHits = r.hits.slice(0, 6); })
        .catch((e) => app.error(e));
    }, text ? 300 : 0);
    return () => clearTimeout(timer);
  });

  // Più memoria per le mod.
  $effect(() => {
    memoryGb = kind === 'modpack' ? 6 : loaderKind ? 5 : 4;
  });

  async function pickFile() {
    try {
      const p = await api.serverPickModpack();
      if (p) pack = { project: null, file: p.path, name: p.name, detail: `${p.loader ?? 'Vanilla'} · Minecraft ${p.game}` };
    } catch (e) {
      app.error(e);
    }
  }

  // Senza "resta acceso" non può partire con il computer.
  $effect(() => {
    if (!keepRunning) autostart = false;
  });

  $effect(() => {
    const off = on('server-progress', (p) => { if (creating && p.name === trimmed) progress = p; });
    return () => void off.then((f) => f());
  });

  const motion = (x: number) => ({ x: moving() ? x : 0, duration: ms(300), easing: cubicOut });

  function go(delta: number) {
    error = null;
    dir = delta;
    step = Math.max(0, Math.min(STEPS.length - 1, step + delta));
  }

  function close() {
    if (!creating) onclose();
  }

  async function create() {
    if (!eula || creating) return;
    creating = true;
    error = null;
    progress = null;
    try {
      const common = { name: trimmed, memoryMb: memoryGb * 1024, port, keepRunning, autostart, eula };
      const s = kind === 'modpack'
        ? await api.serverCreateModpack({ ...common, project: pack?.project ?? null, file: pack?.file ?? null })
        : await api.serverCreate({ ...common, kind, version, loaderVersion: loaderKind ? loaderVersion : null });
      app.info_(t('Server “{nome}” creato.', { nome: s.name }));
      creating = false;
      onclose();
      app.go({ view: 'server', name: s.name });
    } catch (e) {
      app.error(e);
      creating = false;
    }
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!canNext) return;
    if (step < STEPS.length - 1) go(1);
    else void create();
  }
</script>

<Modal title={t('Nuovo server')} onclose={close} width={700}>
  <form class="wizard" onsubmit={submit}>
    <div class="progress-head">
      <div class="steps" aria-hidden="true">
        {#each STEPS as s, i (s)}<span class="pip" class:on={i <= step}></span>{/each}
      </div>
      <span class="muted small" aria-live="polite">{t('Passo {n} di {tot} · {nome}', { n: step + 1, tot: STEPS.length, nome: t(STEPS[step]) })}</span>
    </div>

    {#key step}
      <div class="pane" in:fly={motion(dir * 36)}>
        {#if step === 0}
          <fieldset>
            <legend>{t('Tipo di server')}</legend>
            <div class="kinds">
              {#each KINDS as [id, label, desc] (id)}
                <button type="button" class="kind" class:on={kind === id} aria-pressed={kind === id} onclick={() => (kind = id)}>
                  <span class="letter">{label[0]}</span>
                  <span class="kind-text"><span class="strong">{label}</span><span class="muted small">{t(desc)}</span></span>
                </button>
              {/each}
            </div>
          </fieldset>
          {#if kind === 'modpack'}
            <div class="pack">
              <div class="pack-tools">
                <input class="input grow" type="search" bind:value={packQuery} placeholder={t('Cerca un modpack su Modrinth…')} aria-label={t('Cerca un modpack')} />
                <button type="button" class="btn" onclick={pickFile}>{t('Scegli file .mrpack')}</button>
              </div>
              {#if pack?.file}
                <div class="pack-item on"><span class="strong">{pack.name}</span><span class="muted small">{pack.detail}</span></div>
              {/if}
              <div class="pack-list">
                {#each packHits ?? [] as h (h.id)}
                  <button type="button" class="pack-item" class:on={pack?.project === h.id} aria-pressed={pack?.project === h.id} onclick={() => (pack = { project: h.id, file: null, name: h.title, detail: t('da Modrinth') })}>
                    <span class="strong">{h.title}</span><span class="muted small">{h.description}</span>
                  </button>
                {:else}
                  <span class="muted small">{packHits === null ? t('Caricamento…') : t('Nessun modpack trovato.')}</span>
                {/each}
              </div>
              <span class="hint">{t('Loader, versione e mod li prende dal modpack (solo quelle che servono al server).')}</span>
            </div>
          {:else}
            <div class="versions" class:two={!!loaderKind}>
              <label class="field">{t('Versione di Minecraft')}
                <select class="input" bind:value={version} disabled={!versions?.length}>
                  {#if versions === null}<option value="">{t('Caricamento…')}</option>{:else if !versions.length}<option value="">{t('Nessuna versione disponibile')}</option>{/if}
                  {#each versions ?? [] as v, i (v)}<option value={v}>{v}{i === 0 ? t(' (ultima)') : ''}</option>{/each}
                </select>
              </label>
              {#if loaderKind}
                <label class="field">{t('Versione di {loader}', { loader: LABELS[kind] })}
                  <select class="input" bind:value={loaderVersion} disabled={!loaderVersions?.length}>
                    {#if loaderVersions === null}<option value="">{t('Caricamento…')}</option>{:else if !loaderVersions.length}<option value="">{t('Nessuna versione per questo Minecraft')}</option>{/if}
                    {#each loaderVersions ?? [] as v, i (v)}<option value={v}>{v}{i === 0 ? t(' (consigliata)') : ''}</option>{/each}
                  </select>
                </label>
              {/if}
            </div>
          {/if}
          <label class="field">{t('Nome')}
            <input class="input" bind:value={name} maxlength="32" autocomplete="off" aria-invalid={!!nameError} />
            {#if nameError}<span class="warn">{nameError}</span>{:else}<span class="hint">{t('Lettere, numeri, spazi, - e _ (massimo 32).')}</span>{/if}
          </label>
        {:else if step === 1}
          <div class="summary">
            <span class="letter">{LABELS[kind][0]}</span>
            <span class="kind-text">
              <span class="strong">{summary}</span>
              <span class="muted small">{trimmed}</span>
            </span>
          </div>
          <label class="field">
            <span class="row-label"><span>{t('Memoria')}</span><span class="val">{memoryGb} GB</span></span>
            <input type="range" min="1" max="16" step="1" bind:value={memoryGb} />
            <span class="hint">{t('Consigliati 4 GB fino a ~10 giocatori, 6–8 GB con molti plugin.')}</span>
          </label>
          <label class="field">{t('Porta')}
            <input class="input port" type="number" min="1024" max="65535" bind:value={port} />
            {#if portClash}<span class="warn">{t('La porta {porta} è già usata da “{nome}”.', { porta: port, nome: portClash.name })}</span>
            {:else if !portOk}<span class="warn">{t('Scegli una porta tra 1024 e 65535.')}</span>
            {:else}<span class="hint">{t('Gli amici si collegano a questa porta. 25565 è quella predefinita di Minecraft.')}</span>{/if}
          </label>
          <Toggle bind:checked={keepRunning} label="Resta acceso anche a launcher chiuso" hint="Il server gira in un processo in background, senza finestre." />
          <div class="row" class:disabled={!keepRunning}>
            <div class="txt">
              <span class="strong">{t('Avvia con il computer')}</span>
              <span class="muted small">{keepRunning ? t('Parte da solo quando accedi a Windows.') : t('Disponibile solo se il server resta acceso a launcher chiuso.')}</span>
            </div>
            <button type="button" class="switch" role="switch" aria-checked={autostart} aria-label={t('Avvia con il computer')} disabled={!keepRunning} onclick={() => (autostart = !autostart)}></button>
          </div>
        {:else}
          <h3>{t('EULA di Minecraft')}</h3>
          <p>{t('Per far girare un server Minecraft bisogna accettare il contratto di licenza di Mojang (EULA). In breve: il server è per giocare, non puoi vendere vantaggi di gioco né distribuire il software di Minecraft.')}</p>
          <button type="button" class="btn small link-btn" onclick={() => api.openUrl('https://aka.ms/MinecraftEULA')}>{t('Leggi l’EULA completa')}</button>
          <label class="check"><input type="checkbox" bind:checked={eula} required disabled={creating} /> {t('Accetto l’EULA di Minecraft')}</label>
          {#if creating}
            <div class="creating" aria-live="polite">
              <span class="strong small">{progress ? t(STAGES[progress.stage]) : t('Preparazione…')}{#if progress?.total} · {progress.done}/{progress.total}{/if}</span>
              <ProgressBar progress={progress ?? undefined} label={false} />
            </div>
          {/if}
        {/if}
      </div>
    {/key}

    {#if error}<p class="error-box" role="alert">{error}</p>{/if}

    <div class="actions">
      {#if step > 0}<button type="button" class="btn" disabled={creating} onclick={() => go(-1)}>{t('Indietro')}</button>{:else}<button type="button" class="btn" onclick={close}>{t('Annulla')}</button>{/if}
      <span class="spacer"></span>
      {#if step < STEPS.length - 1}
        <button type="submit" class="btn primary" disabled={!canNext}>{step === 1 ? t('Avanti · EULA') : t('Avanti')}</button>
      {:else}
        <button type="submit" class="btn primary" disabled={!eula || creating}>{creating ? t('Creazione…') : t('Crea server')}</button>
      {/if}
    </div>
  </form>
</Modal>

<style>
  .wizard { display: flex; flex-direction: column; gap: 18px; }
  .progress-head { display: flex; flex-direction: column; gap: 8px; }
  .steps { display: flex; gap: 6px; }
  .pip { flex: 1; height: 4px; border-radius: 2px; background: var(--line-strong); transition: background .3s ease; }
  .pip.on { background: var(--accent); }
  .pane { display: flex; flex-direction: column; gap: 16px; min-height: 300px; }
  fieldset { border: 0; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
  legend { font-weight: 600; font-size: 13px; margin-bottom: 8px; }
  .kinds { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 10px; }
  .versions { display: grid; grid-template-columns: 1fr; gap: 12px; }
  .versions.two { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .pack { display: flex; flex-direction: column; gap: 10px; }
  .pack-tools { display: flex; gap: 10px; }
  .grow { flex: 1; min-width: 0; }
  .pack-list { display: flex; flex-direction: column; gap: 6px; max-height: 170px; overflow-y: auto; }
  .pack-item { display: flex; flex-direction: column; align-items: flex-start; gap: 2px; padding: 9px 12px; text-align: left; border-radius: var(--radius-sm); border: 1px solid var(--line); background: var(--panel); }
  .pack-item .small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; }
  .pack-item:hover { border-color: var(--accent-line); }
  .pack-item.on { border-color: var(--accent-line); background: var(--accent-soft); }
  .kind { display: flex; align-items: center; gap: 12px; padding: 12px; text-align: left; border-radius: var(--radius-sm); border: 1px solid var(--line-strong); background: var(--panel); transition: border-color .18s ease, background .18s ease, transform .12s ease; }
  .kind:hover { border-color: var(--accent-line); }
  .kind:active { transform: scale(.98); }
  .kind.on { border-color: var(--accent-line); background: var(--accent-soft); }
  .letter { flex: none; width: 40px; height: 40px; border-radius: 10px; background: var(--accent-soft); color: var(--accent-line); display: flex; align-items: center; justify-content: center; font-weight: 700; font-family: var(--display); }
  .kind.on .letter { background: var(--accent); color: #fff; }
  .kind-text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .strong { font-weight: 600; }
  .summary { display: flex; align-items: center; gap: 12px; padding: 12px; border-radius: var(--radius-sm); background: var(--panel); border: 1px solid var(--line); }
  .row-label { display: flex; justify-content: space-between; align-items: baseline; }
  .val { font-family: var(--mono); font-size: 12px; color: var(--muted); font-weight: 400; }
  .hint { font-size: 12px; color: var(--muted); font-weight: 400; }
  .warn { font-size: 12px; color: var(--warn-text); font-weight: 400; }
  .port { width: 160px; font-family: var(--mono); }
  .row { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 4px 0; }
  .row.disabled .txt { opacity: .7; }
  .txt { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  h3 { font-size: 16px; font-weight: 600; }
  p { margin: 0; line-height: 1.55; color: var(--text-2); }
  .link-btn { align-self: flex-start; }
  .check { display: flex; align-items: center; gap: 8px; font-weight: 600; }
  .check input { width: 16px; height: 16px; accent-color: var(--accent); }
  .creating { display: flex; flex-direction: column; gap: 8px; padding: 12px; border-radius: var(--radius-sm); background: var(--panel); border: 1px solid var(--line); }
  .actions { display: flex; gap: 10px; align-items: center; }
  .spacer { flex: 1; }
</style>
