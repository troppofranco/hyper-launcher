<script lang="ts">
  // File del server: l'elenco della cartella a tutta larghezza, come i
  // plugin; aprendo un file l'editor prende il suo posto. I file binari o
  // troppo grandi mostrano il messaggio del backend.
  import { api, type ServerFile } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import { t } from '../../lib/i18n.svelte';
  import { relativeDay } from '../../lib/format';
  import Icon from '../Icon.svelte';
  import Modal from '../Modal.svelte';

  let { name, running }: { name: string; running: boolean } = $props();

  interface Ask { title: string; text: string; confirm: string; danger?: boolean; run: () => void | Promise<void> }

  let path = $state('');
  let files = $state<ServerFile[] | null>(null);
  let open = $state<{ path: string; content: string; original: string } | null>(null);
  let openError = $state<{ path: string; message: string } | null>(null);
  let loadingFile = $state(false);
  let saving = $state(false);
  let ask = $state<Ask | null>(null);

  const join = (dir: string, file: string) => (dir ? `${dir}/${file}` : file);
  const crumbs = $derived(path ? path.split('/') : []);
  const sorted = $derived([...(files ?? [])].sort((a, b) => Number(b.dir) - Number(a.dir) || a.name.localeCompare(b.name)));
  const dirty = $derived(!!open && open.content !== open.original);
  const lineCount = $derived(open ? open.content.split('\n').length : 0);
  const selected = $derived(open?.path ?? openError?.path ?? null);
  /** Un file è aperto (o in apertura): l'editor sostituisce l'elenco. */
  const editing = $derived(!!open || !!openError || loadingFile);

  async function list(dir: string) {
    try {
      files = await api.serverFiles(name, dir);
      path = dir;
    } catch (e) {
      app.error(e);
      files ??= [];
    }
  }

  $effect(() => {
    void list('');
  });

  function size(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  /** Prima di lasciare un file modificato si chiede conferma. */
  function guard(next: () => void) {
    if (!dirty) return next();
    ask = { title: t('Modifiche non salvate'), text: t('Le modifiche a {file} andranno perse.', { file: open!.path }), confirm: t('Scarta modifiche'), danger: true, run: next };
  }

  async function openFile(file: string) {
    loadingFile = true;
    openError = null;
    try {
      const content = await api.serverReadFile(name, file);
      open = { path: file, content, original: content };
    } catch (e) {
      open = null;
      openError = { path: file, message: t(typeof e === 'string' ? e : e instanceof Error ? e.message : String(e)) };
    } finally {
      loadingFile = false;
    }
  }

  function activate(f: ServerFile) {
    const full = join(path, f.name);
    if (f.dir) void list(full);
    else if (full !== selected) guard(() => void openFile(full));
  }

  /** Torna all'elenco (chiedendo conferma se ci sono modifiche). */
  function close() {
    guard(() => {
      open = null;
      openError = null;
    });
  }

  function up() {
    void list(path.split('/').slice(0, -1).join('/'));
  }

  async function write() {
    if (!open) return;
    saving = true;
    try {
      await api.serverWriteFile(name, open.path, open.content);
      open.original = open.content;
      app.info_(t('{file} salvato.', { file: open.path }));
      await list(path);
    } catch (e) {
      app.error(e);
    } finally {
      saving = false;
    }
  }

  function save() {
    if (!dirty || saving) return;
    if (running) ask = { title: t('Salvare il file?'), text: t('Il server è acceso: alcune modifiche valgono solo dopo il riavvio.'), confirm: t('Salva'), run: write };
    else void write();
  }

  function remove(f: ServerFile) {
    const full = join(path, f.name);
    ask = {
      title: t('Eliminare il file?'),
      text: t('{file} verrà cancellato per sempre. Non si può annullare.', { file: full }),
      confirm: t('Elimina'),
      danger: true,
      run: async () => {
        try {
          await api.serverDeleteFile(name, full);
          if (selected === full) { open = null; openError = null; }
          await list(path);
        } catch (e) {
          app.error(e);
        }
      },
    };
  }

  function onkeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
      e.preventDefault();
      save();
    }
  }

  async function confirmAsk() {
    const a = ask;
    ask = null;
    await a?.run();
  }
</script>

<section class="card panel" aria-labelledby="files-{name}">
  {#if editing}
    <header class="ebar">
      <button class="btn small" onclick={close}><Icon name="back" size={14} /> {t('File')}</button>
      <span class="ename" id="files-{name}">{selected ?? ''}{#if dirty} <span class="mod">· {t('modificato')}</span>{/if}</span>
      {#if open}
        <span class="einfo">{t('{n} righe', { n: lineCount })}</span>
        <button class="btn small primary" disabled={!dirty || saving} onclick={save}>{saving ? t('Salvataggio…') : t('Salva')}</button>
      {/if}
    </header>
    {#if open}
      <textarea bind:value={open.content} {onkeydown} spellcheck="false" aria-label={t('Contenuto di {file}', { file: open.path })}></textarea>
    {:else if openError}
      <div class="placeholder"><span class="err">{openError.message}</span></div>
    {:else}
      <div class="placeholder muted">{t('Apertura…')}</div>
    {/if}
  {:else}
    <header class="head">
      <h2 id="files-{name}">{t('File')}</h2>
      <nav class="crumbs" aria-label={t('Percorso')}>
        <button aria-label={t('Cartella principale')} onclick={() => void list('')}>/</button>
        {#each crumbs as c, i (i)}
          {#if i > 0}<span class="sep">/</span>{/if}<button onclick={() => void list(crumbs.slice(0, i + 1).join('/'))}>{c}</button>
        {/each}
      </nav>
    </header>
    <ul class="list">
      {#if path}
        <li class="row">
          <button class="entry" onclick={up}><span class="ico"><Icon name="back" size={16} /></span><span class="fname">..</span></button>
        </li>
      {/if}
      {#if files === null}
        <li class="pad"><div class="skeleton skel"></div></li>
      {:else}
        {#each sorted as f, i (f.name)}
          <li class="row enter" style:--i={Math.min(i, 12)}>
            <button class="entry" onclick={() => activate(f)}>
              <span class="ico" class:dir={f.dir}><Icon name={f.dir ? 'folder' : 'file'} size={16} /></span>
              <span class="fname">{f.name}</span>
              <span class="fdate">{relativeDay(f.modified)}</span>
              <span class="fsize">{f.dir ? '' : size(f.size)}</span>
            </button>
            {#if f.dir}
              <span class="del-space"></span>
            {:else}
              <button class="del" aria-label={t('Elimina {file}', { file: f.name })} title={t('Elimina')} onclick={() => remove(f)}><Icon name="trash" size={16} /></button>
            {/if}
          </li>
        {:else}
          <li class="muted small pad">{t('Cartella vuota.')}</li>
        {/each}
      {/if}
    </ul>
  {/if}
</section>

{#if ask}
  <Modal title={ask.title} onclose={() => (ask = null)} width={440}>
    <p class="ask">{ask.text}</p>
    <div class="actions">
      <button class="btn" onclick={() => (ask = null)}>{t('Annulla')}</button>
      <button class="btn {ask.danger ? 'danger' : 'primary'}" onclick={confirmAsk}>{ask.confirm}</button>
    </div>
  </Modal>
{/if}

<style>
  .panel { overflow: hidden; display: flex; flex-direction: column; min-width: 0; }
  .head { display: flex; align-items: baseline; gap: 12px; padding: 14px 16px 10px; flex-wrap: wrap; }
  h2 { margin: 0; font-size: 15px; font-weight: 600; }
  .crumbs { display: flex; flex-wrap: wrap; align-items: center; gap: 2px; font-family: var(--mono); font-size: 12px; color: var(--muted); }
  .crumbs button { border: 0; background: none; padding: 2px 4px; border-radius: 5px; color: var(--muted); font-family: var(--mono); font-size: 12px; }
  .crumbs button:hover { background: var(--surface-2); color: var(--text); }
  .sep { color: var(--faint); }
  .list { list-style: none; margin: 0; padding: 0; }
  .row { display: flex; align-items: center; gap: 6px; padding: 4px 10px; border-top: 1px solid var(--line); }
  .entry { flex: 1; min-width: 0; display: flex; align-items: center; gap: 14px; height: 40px; padding: 0 8px; border: 0; border-radius: 8px; background: transparent; color: var(--text-2); text-align: left; }
  .entry:hover { background: var(--surface-2); color: var(--text); }
  .ico { width: 30px; height: 30px; border-radius: 8px; background: var(--surface-2); display: flex; align-items: center; justify-content: center; color: var(--muted); flex: none; }
  .ico.dir { color: var(--accent-line); }
  .fname { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 500; }
  .fdate { flex: 0 0 120px; font-size: 12px; color: var(--muted); text-align: right; }
  .fsize { flex: 0 0 72px; font-size: 12px; color: var(--muted); text-align: right; font-variant-numeric: tabular-nums; }
  .del, .del-space { width: 36px; height: 36px; flex: none; }
  .del { border: 0; border-radius: 8px; background: transparent; color: var(--muted); display: flex; align-items: center; justify-content: center; }
  .del:hover { color: var(--danger-text); background: var(--surface-2); }
  .skel { height: 32px; border-radius: 8px; }
  .pad { padding: 12px 16px 16px; }
  .ebar { display: flex; align-items: center; gap: 12px; padding: 10px 12px; border-bottom: 1px solid var(--line); font-size: 12px; color: var(--muted); }
  .ename { flex: 1; min-width: 0; font-family: var(--mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-2); }
  .mod { color: var(--warn-text); margin-left: 4px; }
  .einfo { font-variant-numeric: tabular-nums; }
  .ebar .btn { height: 30px; display: inline-flex; align-items: center; gap: 6px; }
  /* L'editor occupa tutta la larghezza e quasi tutta l'altezza visibile. */
  textarea { height: clamp(360px, calc(100dvh - 300px), 1200px); resize: none; border: 0; outline: none; background: var(--bg-deep); padding: 12px 16px; font-family: var(--mono); font-size: 12.5px; line-height: 1.6; color: var(--text-2); tab-size: 2; white-space: pre; }
  .placeholder { min-height: 240px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 6px; padding: 20px; text-align: center; }
  .err { color: var(--danger-text); max-width: 420px; user-select: text; }
  .ask { margin: 0; line-height: 1.5; }
  .actions { display: flex; justify-content: flex-end; gap: 10px; }
  @media (max-width: 640px) { .fdate { display: none; } }
</style>
