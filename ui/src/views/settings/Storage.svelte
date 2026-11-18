<script lang="ts">
  import { t } from '../../lib/i18n.svelte';
  import { api, type StorageUsage } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import Icon from '../../components/Icon.svelte';

  let usage = $state<StorageUsage | null>(null);
  let clearing = $state(false);

  const load = () => api.storageUsage().then((u) => (usage = u)).catch((e) => app.error(e));
  $effect(() => {
    void load();
  });

  const ROWS: [keyof StorageUsage, string][] = [
    ['instances', 'Istanze (mondi, mod, impostazioni)'],
    ['libraries', 'Librerie'],
    ['assets', 'Asset (suoni, lingue, texture)'],
    ['versions', 'Versioni di Minecraft'],
    ['runtimes', 'Java'],
    ['cache', 'Cache dei download'],
  ];
  const total = $derived(usage ? Object.values(usage).reduce((a, b) => a + b, 0) : 0);

  function size(bytes: number): string {
    if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
    if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(0)} MB`;
    return `${Math.round(bytes / 1e3)} KB`;
  }

  async function clearCache() {
    clearing = true;
    try {
      await api.clearDownloadCache();
      app.info_(t('Cache dei download svuotata.'));
      await load();
    } catch (e) {
      app.error(e);
    } finally {
      clearing = false;
    }
  }
</script>

<div class="box enter" style:--i={0}>
  <div><div class="strong">{t('Cartella dati')}</div><div class="muted small path">{app.info?.dataDir}</div></div>
  <button class="btn" onclick={() => api.openFolder(null, 'data')}><Icon name="folder" size={16} /> {t('Apri cartella')}</button>
</div>

<section class="usage enter" style:--i={1}>
  <div class="head"><span class="strong">{t('Spazio usato')}</span><span class="total">{usage ? size(total) : '…'}</span></div>
  <div class="bar" aria-hidden="true">
    {#if usage}
      {#each ROWS as [key], i (key)}
        <span class="seg c{i}" style:width="{total ? (usage[key] / total) * 100 : 0}%"></span>
      {/each}
    {/if}
  </div>
  <ul>
    {#each ROWS as [key, label], i (key)}
      <li><span class="key c{i}"></span><span class="lbl">{t(label)}</span><span class="num">{usage ? size(usage[key]) : '…'}</span></li>
    {/each}
  </ul>
</section>

<div class="box enter" style:--i={2}>
  <div><div class="strong">{t('Svuota cache dei download')}</div><div class="muted small">{t('Toglie installer e modpack già usati. Giochi, mondi e mod restano dove sono.')}</div></div>
  <button class="btn" disabled={clearing || !usage?.cache} onclick={clearCache}><Icon name="trash" size={16} /> {clearing ? t('Pulizia…') : t('Svuota')}</button>
</div>

<style>
  .strong { font-weight: 600; }
  .box { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 14px; border-radius: var(--radius-sm); background: var(--panel); flex-wrap: wrap; }
  .path { user-select: text; word-break: break-all; }
  .usage { display: flex; flex-direction: column; gap: 10px; }
  .head { display: flex; justify-content: space-between; }
  .total { font-family: var(--mono); color: var(--muted); }
  .bar { display: flex; height: 12px; border-radius: 6px; overflow: hidden; background: var(--surface-2); }
  .seg { height: 100%; transition: width .6s var(--ease); }
  ul { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px 18px; }
  li { display: flex; align-items: center; gap: 8px; font-size: 13px; min-width: 0; }
  .lbl { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-2); }
  .num { font-family: var(--mono); font-size: 12px; color: var(--muted); }
  .key { width: 10px; height: 10px; border-radius: 3px; flex: none; }
  .c0 { background: var(--accent); }
  .c1 { background: #2f9bd6; }
  .c2 { background: #3fb97f; }
  .c3 { background: #e5a13a; }
  .c4 { background: #c0306a; }
  .c5 { background: var(--faint); }
</style>
