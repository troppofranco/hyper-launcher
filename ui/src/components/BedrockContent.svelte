<script lang="ts">
  // Una scheda di un'installazione Bedrock: pacchetti risorse, add-on, skin
  // pack o mondi. Si aggiungono file scaricati (.mcpack, .mcaddon, .mcworld,
  // .mctemplate) con il selettore o trascinandoli nella finestra.
  import { api, inTauri, type BedrockKind, type BedrockPack } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { t } from '../lib/i18n.svelte';
  import { relativeDay } from '../lib/format';
  import Icon from './Icon.svelte';
  import Modal from './Modal.svelte';
  import BackupList from './BackupList.svelte';

  let { instance, kind, blocked = null }: {
    instance: string;
    kind: BedrockKind;
    /** Motivo per cui ora non si può modificare (es. gioco aperto). */
    blocked?: string | null;
  } = $props();

  let packs = $state<BedrockPack[] | null>(null);
  let importing = $state(false);
  let dragging = $state(false);
  let ask = $state<BedrockPack | null>(null);
  let openWorld = $state<string | null>(null);

  const HINTS: Record<BedrockKind, string> = {
    resource: 'Si attivano nel gioco: Impostazioni → Risorse globali, oppure mondo per mondo.',
    behavior: 'Si attivano mondo per mondo dentro il gioco: Modifica mondo → Pacchetti comportamento.',
    skin: 'Si scelgono nel gioco dal guardaroba (Spogliatoio).',
    world: 'Ogni mondo ha i suoi backup, come nelle installazioni Java.',
    template: 'Compaiono nel gioco quando crei un mondo nuovo.',
  };
  const EMPTY: Record<BedrockKind, string> = {
    resource: 'Nessun pacchetto risorse.',
    behavior: 'Nessun add-on.',
    skin: 'Nessuno skin pack.',
    world: 'Nessun mondo ancora: crealo dal gioco o aggiungi un file .mcworld.',
    template: 'Nessun modello di mondo.',
  };
  const KIND_NAMES: Record<BedrockKind, string> = {
    resource: 'pacchetto risorse',
    behavior: 'add-on',
    skin: 'skin pack',
    world: 'mondo',
    template: 'modello di mondo',
  };

  async function refresh() {
    try {
      packs = await api.bedrockPacks(instance, kind);
    } catch (e) {
      packs = [];
      app.error(e);
    }
  }

  $effect(() => {
    void [instance, kind];
    packs = null;
    openWorld = null;
    refresh();
  });

  async function add(paths: string[] | null) {
    if (blocked) {
      app.error(blocked);
      return;
    }
    importing = true;
    try {
      const done = await api.bedrockImport(instance, paths);
      if (done.length) {
        const names = done.map((d) => d.name).join(', ');
        app.info_(done.some((d) => d.replaced)
          ? t('Aggiornato: {names}.', { names })
          : t('Aggiunto: {names}.', { names }));
      }
      await refresh();
    } catch (e) {
      app.error(e);
    } finally {
      importing = false;
    }
  }

  async function remove(pack: BedrockPack) {
    ask = null;
    try {
      await api.bedrockDelete(instance, pack);
      await refresh();
    } catch (e) {
      app.error(e);
    }
  }

  // File trascinati nella finestra (solo nell'app vera: il browser non dà i
  // percorsi). Valgono solo i formati di Bedrock.
  const BEDROCK_FILE = /\.(mcpack|mcaddon|mcworld|mctemplate|zip)$/i;
  $effect(() => {
    if (!inTauri) return;
    let off: (() => void) | null = null;
    let alive = true;
    import('@tauri-apps/api/webview').then(({ getCurrentWebview }) =>
      getCurrentWebview().onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === 'enter' || p.type === 'over') dragging = true;
        else if (p.type === 'leave') dragging = false;
        else if (p.type === 'drop') {
          dragging = false;
          const files = p.paths.filter((f) => BEDROCK_FILE.test(f));
          if (files.length) add(files);
          else if (p.paths.length) app.error(t('Trascina file .mcpack, .mcaddon, .mcworld o .mctemplate.'));
        }
      }),
    ).then((unlisten) => {
      if (alive) off = unlisten;
      else unlisten();
    });
    return () => {
      alive = false;
      off?.();
    };
  });
</script>

<div class="tools">
  <span class="muted small hint">{packs ? `${packs.length} · ` : ''}{t(HINTS[kind])}</span>
  <button class="btn" onclick={() => api.bedrockOpenFolder(instance, kind).catch((e) => app.error(e))}><Icon name="folder" size={16} /> {t('Apri cartella')}</button>
  <button class="btn primary" disabled={importing || !!blocked} title={blocked ?? undefined} onclick={() => add(null)}><Icon name="plus" size={16} /> {importing ? t('Aggiunta…') : t('Aggiungi file')}</button>
</div>

{#if packs === null}
  <p class="muted">{t('Lettura…')}</p>
{:else}
  <div class="list">
    {#each packs as pack (pack.scope + '/' + pack.folder)}
      <div class="card item">
        <div class="row">
          {#if pack.icon}<img class="thumb" src={pack.icon} alt="" />{:else}<span class="thumb empty"><Icon name={kind === 'world' ? 'globe' : 'file'} size={18} /></span>{/if}
          <span class="txt">
            <span class="name">{pack.name}</span>
            <span class="muted small desc">{kind === 'world' ? t('Ultimo accesso: {when}', { when: relativeDay(pack.modified) }) : pack.description}</span>
          </span>
          {#if pack.version}<span class="ver">{pack.version}</span>{/if}
          {#if kind === 'world'}
            <button class="btn small" aria-expanded={openWorld === pack.folder} onclick={() => (openWorld = openWorld === pack.folder ? null : pack.folder)}><Icon name="download" size={14} /> {t('Backup')}</button>
          {/if}
          <button class="btn small icon-only" disabled={!!blocked} aria-label={t('Elimina')} title={blocked ?? t('Elimina')} onclick={() => (ask = pack)}><Icon name="trash" size={14} /></button>
        </div>
        {#if kind === 'world' && openWorld === pack.folder}
          <BackupList
            compact
            title={t('Backup di {world}', { world: pack.name })}
            load={() => api.worldBackups(instance, pack.folder)}
            create={() => api.worldBackup(instance, pack.folder)}
            restore={(file) => api.worldBackupRestore(instance, pack.folder, file)}
            remove={(file) => api.worldBackupDelete(instance, pack.folder, file)}
            restoreBlocked={blocked}
          />
        {/if}
      </div>
    {:else}
      <p class="muted">{t(EMPTY[kind])}</p>
    {/each}
  </div>
{/if}

{#if inTauri}
  <div class="drop" class:on={dragging}>
    {t('Trascina qui i file .mcpack, .mcaddon, .mcworld o .mctemplate scaricati da internet: ognuno va nella scheda giusta da solo.')}
  </div>
{/if}

{#if ask}
  <Modal title={t('Eliminare {what}?', { what: t(KIND_NAMES[kind]) })} onclose={() => (ask = null)} width={440}>
    <p class="confirm">{t('“{name}” verrà cancellato per sempre da questa installazione.', { name: ask.name })}</p>
    <div class="actions">
      <button class="btn" onclick={() => (ask = null)}>{t('Annulla')}</button>
      <button class="btn danger" onclick={() => ask && remove(ask)}>{t('Elimina')}</button>
    </div>
  </Modal>
{/if}

<style>
  .tools { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
  .hint { flex: 1; min-width: 200px; line-height: 1.45; }
  .list { display: flex; flex-direction: column; gap: 8px; }
  .item { padding: 10px 12px; display: flex; flex-direction: column; gap: 12px; }
  .row { display: flex; align-items: center; gap: 12px; }
  .thumb { flex: none; width: 40px; height: 40px; border-radius: 9px; object-fit: cover; image-rendering: pixelated; }
  .thumb.empty { display: flex; align-items: center; justify-content: center; background: var(--surface-2); color: var(--muted); }
  .txt { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .name { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .desc { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ver { flex: none; font-family: var(--mono); font-size: 12px; color: var(--faint); font-variant-numeric: tabular-nums; }
  .icon-only { width: 32px; padding: 0; justify-content: center; }
  .icon-only:hover:not(:disabled) { color: var(--danger-text); }
  .drop { border: 1.5px dashed var(--line-strong); border-radius: var(--radius); padding: 16px; text-align: center; color: var(--muted); font-size: 13px; line-height: 1.5; transition: border-color .2s, background .2s; }
  .drop.on { border-color: var(--accent-line); background: var(--accent-soft); color: var(--text); }
  .confirm { margin: 0 0 18px; line-height: 1.5; color: var(--text-2); }
  .actions { display: flex; justify-content: flex-end; gap: 10px; }
</style>
