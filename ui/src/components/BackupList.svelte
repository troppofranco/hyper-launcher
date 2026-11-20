<script lang="ts">
  // Elenco dei backup di un mondo (server o istanza): "Backup ora",
  // ripristina ed elimina con conferma. Le operazioni le passa chi lo usa.
  import type { BackupInfo } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { t } from '../lib/i18n.svelte';
  import { megabytes, relativeDay } from '../lib/format';
  import Icon from './Icon.svelte';
  import Modal from './Modal.svelte';

  let { title, subtitle, load, create, restore, remove, restoreBlocked = null, compact = false }: {
    title: string;
    subtitle?: string;
    load: () => Promise<BackupInfo[]>;
    create: () => Promise<unknown>;
    restore: (file: string) => Promise<unknown>;
    remove: (file: string) => Promise<unknown>;
    /** Motivo per cui ora non si può ripristinare (es. server acceso). */
    restoreBlocked?: string | null;
    compact?: boolean;
  } = $props();

  let list = $state<BackupInfo[] | null>(null);
  let creating = $state(false);
  let busy = $state(false);
  let ask = $state<{ kind: 'restore' | 'delete'; backup: BackupInfo } | null>(null);

  async function refresh() {
    try {
      list = await load();
    } catch (e) {
      app.error(e);
      list ??= [];
    }
  }

  $effect(() => {
    void refresh();
  });

  async function backupNow() {
    creating = true;
    try {
      await create();
      app.info_(t('Backup completato.'));
      await refresh();
    } catch (e) {
      app.error(e);
    } finally {
      creating = false;
    }
  }

  async function confirm() {
    const a = ask;
    ask = null;
    if (!a) return;
    busy = true;
    try {
      if (a.kind === 'restore') {
        await restore(a.backup.file);
        app.info_(t('Backup ripristinato.'));
      } else {
        await remove(a.backup.file);
      }
      await refresh();
    } catch (e) {
      app.error(e);
    } finally {
      busy = false;
    }
  }

  /** "Oggi, 18:00" / "Ieri, 21:40" / "12 nov, 09:00". */
  function when(secs: number): string {
    const time = new Date(secs * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    return `${relativeDay(secs)}, ${time}`;
  }
</script>

<section class="card list" class:compact>
  <header class="head">
    <div class="titles">
      <span class="title">{title}</span>
      {#if subtitle}<span class="muted small">{subtitle}</span>{/if}
    </div>
    <button class="btn primary small" disabled={creating} onclick={backupNow}>
      <Icon name="download" size={14} /> {creating ? t('Backup in corso…') : t('Backup ora')}
    </button>
  </header>
  {#if list === null}
    <p class="muted small pad">{t('Lettura dei backup…')}</p>
  {:else}
    {#each list as b (b.file)}
      <div class="row">
        <span class="ico"><Icon name="file" size={14} /></span>
        <span class="name">{when(b.created)} <span class="muted small">· {b.auto ? t('automatico') : t('manuale')}</span></span>
        <span class="muted small size">{megabytes(b.sizeBytes)}</span>
        <button class="btn small" disabled={busy || !!restoreBlocked} title={restoreBlocked ?? ''} onclick={() => (ask = { kind: 'restore', backup: b })}>{t('Ripristina')}</button>
        <button class="icon-btn" disabled={busy} aria-label={t('Elimina backup')} title={t('Elimina')} onclick={() => (ask = { kind: 'delete', backup: b })}><Icon name="trash" size={16} /></button>
      </div>
    {:else}
      <p class="muted small pad">{t('Ancora nessun backup.')}</p>
    {/each}
  {/if}
  {#if restoreBlocked && list?.length}<p class="note">{restoreBlocked}</p>{/if}
</section>

{#if ask}
  <Modal title={ask.kind === 'restore' ? t('Ripristinare il backup?') : t('Eliminare il backup?')} onclose={() => (ask = null)} width={460}>
    <p class="ask">{ask.kind === 'restore'
      ? t('Il mondo torna com’era il {when}. Quello attuale non si perde: resta salvato accanto come “prima del ripristino”.', { when: when(ask.backup.created) })
      : t('Il backup del {when} verrà cancellato per sempre.', { when: when(ask.backup.created) })}</p>
    <div class="actions">
      <button class="btn" onclick={() => (ask = null)}>{t('Annulla')}</button>
      <button class="btn {ask.kind === 'delete' ? 'danger' : 'primary'}" onclick={confirm}>{ask.kind === 'restore' ? t('Ripristina') : t('Elimina')}</button>
    </div>
  </Modal>
{/if}

<style>
  .list { overflow: hidden; }
  .head { display: flex; align-items: center; gap: 12px; padding: 14px 16px; }
  .titles { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .title { font-weight: 600; font-size: 15px; }
  .compact .head { padding: 10px 14px; }
  .compact .title { font-size: 14px; }
  .row { display: flex; align-items: center; gap: 12px; padding: 8px 12px 8px 16px; border-top: 1px solid var(--line); }
  .ico { width: 28px; height: 28px; border-radius: 8px; background: var(--surface-2); color: var(--muted); display: flex; align-items: center; justify-content: center; flex: none; }
  .name { flex: 1; min-width: 0; font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .size { flex: none; font-variant-numeric: tabular-nums; }
  .pad { padding: 0 16px 14px; margin: 0; }
  .note { margin: 0; padding: 10px 16px; border-top: 1px solid var(--line); color: var(--text-2); font-size: 12.5px; }
  .icon-btn { width: 32px; height: 32px; border-radius: 8px; border: 0; background: transparent; color: var(--muted); display: flex; align-items: center; justify-content: center; flex: none; }
  .icon-btn:hover:not(:disabled) { color: var(--danger-text); background: var(--surface-2); }
  .ask { margin: 0; line-height: 1.5; }
  .actions { display: flex; justify-content: flex-end; gap: 10px; }
</style>
