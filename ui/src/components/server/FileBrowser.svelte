<script lang="ts">
  // File del server: elenco della cartella a sinistra, editor di testo a
  // destra. I file binari o troppo grandi mostrano il messaggio del backend.
  import { api, type ServerFile } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import { t } from '../../lib/i18n.svelte';
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
  <div class="side">
    <h2 id="files-{name}">{t('File')}</h2>
    <nav class="crumbs" aria-label={t('Percorso')}>
      <button aria-label={t('Cartella principale')} onclick={() => void list('')}>/</button>
      {#each crumbs as c, i (i)}
        {#if i > 0}<span class="sep">/</span>{/if}<button onclick={() => void list(crumbs.slice(0, i + 1).join('/'))}>{c}</button>
      {/each}
    </nav>
    <ul class="list scroll">
      {#if path}
        <li><button class="entry" onclick={up}><span class="ico"><Icon name="back" size={14} /></span><span class="fname">..</span></button></li>
      {/if}
      {#if files === null}
        <li class="skeleton skel"></li>
      {:else}
        {#each sorted as f (f.name)}
          {@const full = join(path, f.name)}
          <li class="item">
            <button class="entry" class:on={selected === full} onclick={() => activate(f)} aria-current={selected === full ? 'true' : undefined}>
              <span class="ico"><Icon name={f.dir ? 'folder' : 'file'} size={14} /></span>
              <span class="fname">{f.name}</span>
              {#if !f.dir}<span class="fsize">{size(f.size)}</span>{/if}
            </button>
            {#if !f.dir}
              <button class="del" aria-label={t('Elimina {file}', { file: f.name })} title={t('Elimina')} onclick={() => remove(f)}><Icon name="trash" size={14} /></button>
            {/if}
          </li>
        {:else}
          <li class="muted small empty">{t('Cartella vuota.')}</li>
        {/each}
      {/if}
    </ul>
  </div>

  <div class="editor">
    {#if open}
      <div class="ebar">
        <span class="ename">{open.path}{#if dirty} <span class="mod">· {t('modificato')}</span>{/if}</span>
        <span class="einfo">{t('{n} righe', { n: lineCount })}</span>
        <button class="btn small primary" disabled={!dirty || saving} onclick={save}>{saving ? t('Salvataggio…') : t('Salva')}</button>
      </div>
      <textarea bind:value={open.content} {onkeydown} spellcheck="false" aria-label={t('Contenuto di {file}', { file: open.path })}></textarea>
    {:else if openError}
      <div class="placeholder"><span class="strong">{openError.path}</span><span class="err">{openError.message}</span></div>
    {:else}
      <div class="placeholder muted">{loadingFile ? t('Apertura…') : t('Scegli un file di testo per modificarlo.')}</div>
    {/if}
  </div>
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
  .panel { padding: 16px; display: flex; gap: 14px; height: 340px; min-width: 0; }
  .side { flex: 0 0 300px; min-width: 0; display: flex; flex-direction: column; gap: 6px; }
  h2 { font-size: 16px; font-weight: 600; }
  .crumbs { display: flex; flex-wrap: wrap; align-items: center; gap: 2px; font-family: var(--mono); font-size: 12px; color: var(--muted); }
  .crumbs button { border: 0; background: none; padding: 2px 4px; border-radius: 5px; color: var(--muted); font-family: var(--mono); font-size: 12px; }
  .crumbs button:hover { background: var(--surface-2); color: var(--text); }
  .sep { color: var(--faint); }
  .list { list-style: none; margin: 0; padding: 0 4px 0 0; flex: 1; min-height: 0; display: flex; flex-direction: column; gap: 2px; }
  .item { display: flex; align-items: center; gap: 2px; }
  .entry { flex: 1; min-width: 0; display: flex; align-items: center; gap: 8px; height: 32px; padding: 0 10px; border: 0; border-radius: 8px; background: transparent; color: var(--text-2); text-align: left; }
  .entry:hover { background: var(--surface-2); color: var(--text); }
  .entry.on { background: var(--accent-soft); color: var(--text); }
  .ico { display: flex; color: var(--muted); flex: none; }
  .fname { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .fsize { font-size: 12px; color: var(--muted); flex: none; font-variant-numeric: tabular-nums; }
  .del { width: 28px; height: 28px; flex: none; border: 0; border-radius: 7px; background: transparent; color: var(--faint); display: flex; align-items: center; justify-content: center; opacity: 0; transition: opacity .15s ease; }
  .item:hover .del, .del:focus-visible { opacity: 1; }
  .del:hover { color: var(--danger-text); background: var(--surface-2); }
  .skel { height: 32px; border-radius: 8px; }
  .empty { padding: 6px 10px; }
  .editor { flex: 1; min-width: 0; display: flex; flex-direction: column; border-radius: var(--radius-sm); background: var(--bg-deep); border: 1px solid var(--line); overflow: hidden; }
  .ebar { display: flex; align-items: center; gap: 12px; padding: 6px 8px 6px 12px; border-bottom: 1px solid var(--line); font-size: 12px; color: var(--muted); }
  .ename { flex: 1; min-width: 0; font-family: var(--mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-2); }
  .mod { color: var(--warn-text); }
  .einfo { font-variant-numeric: tabular-nums; }
  .ebar .btn { height: 28px; }
  textarea { flex: 1; min-height: 0; resize: none; border: 0; outline: none; background: transparent; padding: 10px 14px; font-family: var(--mono); font-size: 12px; line-height: 1.6; color: var(--text-2); tab-size: 2; white-space: pre; }
  .placeholder { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 6px; padding: 20px; text-align: center; }
  .strong { font-weight: 600; font-family: var(--mono); font-size: 12px; }
  .err { color: var(--danger-text); max-width: 420px; user-select: text; }
  .ask { margin: 0; line-height: 1.5; }
  .actions { display: flex; justify-content: flex-end; gap: 10px; }
  @media (max-width: 760px) { .panel { flex-direction: column; height: auto; } .side { flex-basis: auto; max-height: 260px; } .editor { height: 300px; } }
</style>
