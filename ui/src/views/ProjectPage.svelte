<script lang="ts">
  import { t } from '../lib/i18n.svelte';
  import { api, type ContentKind, type InstallPlan, type Project, type World } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { compact, megabytes, relativeIso } from '../lib/format';
  import { IRIS_PROJECT_ID, incompatibility, kindLabel } from '../lib/content';
  import Icon from '../components/Icon.svelte';
  import ProjectIcon from '../components/ProjectIcon.svelte';
  import ProgressBar from '../components/ProgressBar.svelte';

  let { id, kind }: { id: string; kind: ContentKind } = $props();

  let project = $state<Project | null>(null);
  let failed = $state<string | null>(null);
  let plan = $state<InstallPlan | null>(null);
  let planError = $state<string | null>(null);
  let worlds = $state<World[]>([]);
  let world = $state<string | null>(null);
  let busy = $state(false);
  let done = $state(false);

  const ex = app.explore;
  const isPack = $derived(kind === 'modpack');
  const target = $derived(app.instances.find((i) => i.name === ex.instance) ?? null);
  const blocked = $derived(incompatibility(kind, target));
  const needsWorld = $derived(kind === 'datapack');
  const addsIris = $derived(kind === 'shader' && !!plan?.files.some((f) => f.projectId === IRIS_PROJECT_ID));
  const progress = $derived(busy ? app.contentProgress[isPack ? '' : (ex.instance ?? '')] : undefined);

  $effect(() => {
    api.project(id).then((p) => (project = p)).catch((e) => (failed = String(e)));
  });

  // Mondi dell'istanza, per i data pack.
  $effect(() => {
    const inst = ex.instance;
    worlds = [];
    world = null;
    if (!inst || !needsWorld) return;
    api.listWorlds(inst).then((w) => {
      worlds = w;
      world = w[0]?.name ?? null;
    }).catch((e) => app.error(e));
  });

  // Anteprima di cosa verrà installato (dipendenze comprese).
  $effect(() => {
    const inst = ex.instance;
    const w = world;
    plan = null;
    planError = null;
    done = false;
    if (!project || isPack || !inst || blocked || (needsWorld && !w)) return;
    api.plan(inst, project.id, kind, w).then((p) => (plan = p)).catch((e) => (planError = String(e)));
  });

  async function install() {
    if (!project) return;
    busy = true;
    if (isPack) await app.installPack(project.id);
    else done = await app.installContent(ex.instance!, project.id, project.title, kind, world);
    busy = false;
  }
</script>

<div class="page">
  <button class="back" onclick={() => app.go({ view: 'explore' })}><Icon name="back" size={16} /> {t('Esplora')}</button>

  {#if failed}
    <p class="error-box" role="alert">{t('Impossibile caricare il progetto: {errore}', { errore: failed })}</p>
  {:else if !project}
    <p class="muted">{t('Caricamento…')}</p>
  {:else}
    <div class="layout">
      <div class="main">
        <div class="head">
          <ProjectIcon url={project.iconUrl} name={project.title} size={72} />
          <div>
            <h1 class="page-title">{project.title}</h1>
            <p class="muted">{kindLabel(kind)} · ↓ {compact(project.downloads)} {t('download')}</p>
          </div>
        </div>
        <p class="desc">{project.description}</p>
        {#if project.gallery.length}
          <div class="gallery">
            {#each project.gallery.slice(0, 6) as img (img.url)}
              <figure><img src={img.url} alt={img.title ?? ''} loading="lazy" />{#if img.title}<figcaption>{img.title}</figcaption>{/if}</figure>
            {/each}
          </div>
        {/if}
      </div>

      <aside class="side">
        <section class="card box">
          {#if isPack}
            <span class="label">{t('Nuova istanza')}</span>
            <p class="small muted">{t('Crea un’istanza con l’ultima versione di questo modpack per Fabric: Minecraft, loader, mod e configurazioni.')}</p>
          {:else}
            <label class="label" for="project-target">{t('Installa in')}</label>
            <select id="project-target" class="input" bind:value={ex.instance}>
              <option value={null}>{t('Scegli un’istanza…')}</option>
              {#each app.recent as inst (inst.name)}
                <option value={inst.name}>{inst.name} · {inst.loader ? inst.loader.name : 'Vanilla'} {inst.version}</option>
              {/each}
            </select>
            {#if needsWorld && ex.instance}
              {#if worlds.length}
                <label class="label" for="project-world">{t('Mondo')}</label>
                <select id="project-world" class="input" bind:value={world}>
                  {#each worlds as w (w.name)}<option value={w.name}>{w.name}</option>{/each}
                </select>
              {:else}
                <p class="small muted">{t('Questa istanza non ha ancora mondi: i data pack si installano dentro un mondo. Crealo dal gioco e torna qui.')}</p>
              {/if}
            {/if}
          {/if}

          {#if blocked}<p class="note warn">{blocked}</p>{/if}
          {#if addsIris}<p class="note warn"><strong>{t('Serve Iris.')}</strong> {t('Gli shader su Fabric funzionano con Iris, che non è in questa istanza: lo installo insieme, con le sue dipendenze.')}</p>{/if}
          {#if planError}<p class="error-box">{planError}</p>{/if}

          {#if plan}
            <ul class="plan">
              {#each plan.files as f (f.file)}
                <li><span class="file">{f.name}</span><span class="tag">{f.replaces ? t('aggiorna') : f.folder}</span></li>
              {/each}
            </ul>
          {/if}

          {#if busy}
            <ProgressBar {progress} />
          {:else if done}
            <p class="installed"><Icon name="check" size={16} /> {t('Installato in {istanza}', { istanza: ex.instance ?? '' })}</p>
            <button class="btn" onclick={() => ex.instance && app.go({ view: 'instance', name: ex.instance })}>{t('Apri l’istanza')}</button>
          {:else if isPack}
            <button class="btn primary" onclick={install}>{t('Crea istanza')}</button>
          {:else}
            <button class="btn primary" disabled={!plan} onclick={install}>
              {plan ? t('Installa {n} file · {size}', { n: plan.files.length, size: megabytes(plan.totalBytes) }) : t('Installa')}
            </button>
            <span class="faint small">{t('Ogni file viene verificato con il suo hash prima dell’uso.')}</span>
          {/if}
        </section>

        <section class="card box">
          <span class="label">{t('Informazioni')}</span>
          <div class="row"><span class="muted">{t('Aggiornato')}</span><span>{relativeIso(project.updated)}</span></div>
          {#if project.license}<div class="row"><span class="muted">{t('Licenza')}</span><span>{project.license}</span></div>{/if}
          <button class="link" onclick={() => api.openUrl(project!.pageUrl)}>{t('Apri su modrinth.com')} <Icon name="external" size={14} /></button>
          {#if project.sourceUrl?.startsWith('https://')}
            <button class="link" onclick={() => api.openUrl(project!.sourceUrl!)}>{t('Codice sorgente')} <Icon name="external" size={14} /></button>
          {/if}
        </section>
      </aside>
    </div>
  {/if}
</div>

<style>
  .back { align-self: flex-start; display: flex; align-items: center; gap: 6px; background: none; border: 0; color: var(--muted); padding: 0; font-size: 13px; }
  .back:hover { color: var(--text); }
  .layout { display: grid; grid-template-columns: minmax(0, 1fr) 300px; gap: 24px; align-items: start; }
  @media (max-width: 980px) { .layout { grid-template-columns: 1fr; } }
  .main { display: flex; flex-direction: column; gap: 18px; min-width: 0; }
  .head { display: flex; align-items: center; gap: 18px; }
  .head p { margin: 6px 0 0; }
  .desc { margin: 0; color: var(--text-2); line-height: 1.6; max-width: 65ch; user-select: text; }
  .gallery { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 10px; }
  figure { margin: 0; display: flex; flex-direction: column; gap: 6px; }
  figure img { width: 100%; aspect-ratio: 16 / 9; object-fit: cover; border-radius: 10px; background: var(--surface-2); }
  figcaption { font-size: 12px; color: var(--muted); }
  .side { display: flex; flex-direction: column; gap: 14px; }
  .box { padding: 16px; display: flex; flex-direction: column; gap: 10px; }
  .box p { margin: 0; }
  .note { margin: 0; padding: 10px 12px; border-radius: 10px; font-size: 13px; line-height: 1.5; }
  .note.warn { background: color-mix(in srgb, #e5a13a 14%, transparent); color: var(--warn-text); }
  .plan { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
  .plan li { display: flex; justify-content: space-between; gap: 10px; font-size: 13px; }
  .file { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tag { flex: none; font-size: 11px; padding: 2px 7px; border-radius: 6px; background: var(--surface-2); color: var(--muted); }
  .faint { color: var(--faint); }
  .installed { display: flex; align-items: center; gap: 6px; color: var(--success-text); font-weight: 600; }
  .row { display: flex; justify-content: space-between; gap: 10px; font-size: 13px; }
  .link { align-self: flex-start; display: inline-flex; align-items: center; gap: 6px; background: none; border: 0; padding: 0; color: var(--accent-line); font-size: 13px; }
</style>
