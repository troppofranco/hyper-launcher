<script lang="ts">
  import { api, type Skin, type Skins } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { t } from '../lib/i18n.svelte';
  import { pickFile, readSkin, skinFront } from '../lib/image';
  import Icon from '../components/Icon.svelte';
  import Modal from '../components/Modal.svelte';
  import SkinViewer from '../components/SkinViewer.svelte';

  let data = $state<Skins | null>(null);
  let defaults = $state<Skin[] | null>(null);
  let fronts = $state<Record<string, string>>({});
  /** Skin mostrata nell'anteprima (della libreria o predefinita). */
  let preview = $state<Skin | null>(null);
  let defaultsOpen = $state(true);
  let busy = $state(false);
  // Aggiunta di una skin dal PC: nome e modello prima di salvarla.
  let pending = $state<{ image: string; name: string; slim: boolean } | null>(null);

  const selected = $derived(data?.skins.find((s) => s.id === data?.selected) ?? null);

  async function thumbs(skins: Skin[]) {
    for (const s of skins) {
      if (fronts[s.id]) continue;
      skinFront(s.image, s.slim).then((f) => (fronts[s.id] = f)).catch(() => {});
    }
  }

  $effect(() => {
    api.listSkins().then((d) => {
      data = d;
      preview ??= d.skins.find((s) => s.id === d.selected) ?? null;
      thumbs(d.skins);
    }).catch((e) => app.error(e));
    api.defaultSkins().then((d) => {
      defaults = d;
      preview ??= d[0] ?? null;
      thumbs(d);
    }).catch(() => (defaults = []));
  });

  async function run(op: () => Promise<Skins>) {
    busy = true;
    try {
      data = await op();
      thumbs(data.skins);
    } catch (e) {
      app.error(e);
    } finally {
      busy = false;
    }
  }

  async function addFromPc() {
    const file = await pickFile('image/png');
    if (!file) return;
    try {
      const image = await readSkin(file);
      pending = { image, name: file.name.replace(/\.png$/i, ''), slim: false };
    } catch (e) {
      app.error(e);
    }
  }

  async function savePending(e: SubmitEvent) {
    e.preventDefault();
    if (!pending) return;
    const p = pending;
    pending = null;
    await run(() => api.addSkin(p.name, p.image, p.slim));
    preview = data?.skins.find((s) => s.id === data?.selected) ?? preview;
    app.info_(t('Skin “{nome}” aggiunta e scelta.', { nome: p.name }));
  }

  /** Una skin predefinita diventa tua: copiata nella libreria e scelta. */
  async function useDefault(s: Skin) {
    // Se è già nella libreria si sceglie quella, senza farne una copia.
    const existing = data?.skins.find((x) => x.image === s.image);
    if (existing) return choose(existing);
    await run(() => api.addSkin(s.name, s.image, s.slim));
    preview = data?.skins.find((x) => x.id === data?.selected) ?? preview;
  }

  async function choose(s: Skin | null) {
    await run(() => api.selectSkin(s?.id ?? null));
    if (s) preview = s;
  }

  async function remove(s: Skin) {
    await run(() => api.removeSkin(s.id));
    if (preview?.id === s.id) preview = selected;
  }

  const isLibrary = $derived(!!preview && !preview.id.startsWith('default:'));
</script>

<div class="page custom">
  <div class="page-head enter">
    <div>
      <h1 class="page-title">{t('Personalizzazione')}</h1>
      <p class="muted">{t('Scegli la skin del tuo personaggio e guardala da ogni lato.')}</p>
    </div>
  </div>

  <div class="layout">
    <section class="card stage enter" style:--i={1} aria-label={t('Anteprima')}>
      <div class="glow"></div>
      <SkinViewer skin={preview?.image ?? null} slim={preview?.slim ?? false} />
      <div class="stage-info">
        <span class="stage-name">{preview?.name ?? t('Nessuna skin')}</span>
        <span class="muted small">{preview ? (preview.slim ? t('Braccia sottili') : t('Braccia classiche')) : ''}</span>
      </div>
      <div class="stage-actions">
        {#if preview && isLibrary}
          {#if data?.selected === preview.id}
            <span class="in-use"><Icon name="check" size={16} /> {t('In uso')}</span>
          {:else}
            <button class="btn primary" disabled={busy} onclick={() => preview && choose(preview)}>{t('Usa questa skin')}</button>
          {/if}
        {:else if preview}
          <button class="btn primary" disabled={busy} onclick={() => preview && useDefault(preview)}>{t('Usa questa skin')}</button>
        {/if}
      </div>
      {#if data && !data.offline}
        <p class="note">{t('Con un account Microsoft in gioco vedi la skin del tuo profilo Minecraft: qui puoi provarne altre in anteprima.')}</p>
      {:else if data}
        <p class="note">{t('Con l’account offline la skin si vede nelle tue partite tramite un resource pack che il launcher attiva da solo all’avvio.')}</p>
      {/if}
    </section>

    <div class="side">
      <section class="enter" style:--i={2} aria-labelledby="mine">
        <h2 id="mine" class="section-title">{t('Le tue skin')} <span class="muted small">{data?.skins.length ?? 0}</span></h2>
        <div class="grid">
          <button class="tile add" onclick={addFromPc} disabled={busy}>
            <Icon name="plus" size={22} />
            <span>{t('Aggiungi skin')}</span>
          </button>
          {#each data?.skins ?? [] as s, i (s.id)}
            <div class="tile-wrap enter" style:--i={i + 3}>
              <button class="tile" class:selected={data?.selected === s.id} class:previewing={preview?.id === s.id} onclick={() => (preview = s)} ondblclick={() => choose(s)} title={t('Clic: anteprima · doppio clic: usa')}>
                {#if fronts[s.id]}<img src={fronts[s.id]} alt="" />{/if}
                {#if data?.selected === s.id}<span class="dot"><Icon name="check" size={12} /></span>{/if}
              </button>
              <span class="tile-name">{s.name}</span>
              <button class="tile-x" aria-label={t('Rimuovi {nome}', { nome: s.name })} onclick={() => remove(s)}><Icon name="trash" size={14} /></button>
            </div>
          {/each}
        </div>
        {#if data?.selected}
          <button class="link" onclick={() => choose(null)}>{t('Torna alla skin predefinita del gioco')}</button>
        {/if}
      </section>

      <section class="enter" style:--i={4} aria-labelledby="defaults">
        <button id="defaults" class="section-title toggle" aria-expanded={defaultsOpen} onclick={() => (defaultsOpen = !defaultsOpen)}>
          <span class="chev" class:open={defaultsOpen}><Icon name="back" size={14} /></span> {t('Skin predefinite')}
        </button>
        {#if defaultsOpen}
          {#if defaults === null}
            <p class="muted small">{t('Caricamento…')}</p>
          {:else if !defaults.length}
            <p class="muted small">{t('Le skin predefinite si leggono dal gioco: compariranno dopo aver avviato una qualsiasi istanza recente.')}</p>
          {:else}
            <div class="grid">
              {#each defaults as s, i (s.id)}
                <div class="tile-wrap enter" style:--i={i}>
                  <button class="tile" class:previewing={preview?.id === s.id} onclick={() => (preview = s)} ondblclick={() => useDefault(s)}>
                    {#if fronts[s.id]}<img src={fronts[s.id]} alt="" />{/if}
                  </button>
                  <span class="tile-name">{s.name}</span>
                </div>
              {/each}
            </div>
          {/if}
        {/if}
      </section>
    </div>
  </div>
</div>

{#if pending}
  <Modal title={t('Nuova skin')} onclose={() => (pending = null)} width={440}>
    <form class="add-form" onsubmit={savePending}>
      <div class="add-preview">
        <SkinViewer skin={pending.image} slim={pending.slim} width={160} height={220} />
      </div>
      <label class="field">{t('Nome')}
        <input class="input" bind:value={pending.name} maxlength="40" />
      </label>
      <fieldset>
        <legend class="field">{t('Modello')}</legend>
        <div class="models">
          <button type="button" class:on={!pending.slim} onclick={() => pending && (pending.slim = false)}>{t('Braccia classiche')}</button>
          <button type="button" class:on={pending.slim} onclick={() => pending && (pending.slim = true)}>{t('Braccia sottili')}</button>
        </div>
      </fieldset>
      <div class="actions">
        <button type="button" class="btn" onclick={() => (pending = null)}>{t('Annulla')}</button>
        <button type="submit" class="btn primary">{t('Salva e usa')}</button>
      </div>
    </form>
  </Modal>
{/if}

<style>
  .layout { display: grid; grid-template-columns: minmax(280px, 380px) minmax(0, 1fr); gap: 22px; align-items: start; }
  .stage { position: relative; overflow: hidden; display: flex; flex-direction: column; align-items: center; gap: 12px; padding: 20px; }
  .glow { position: absolute; inset: auto -30% 30% -30%; height: 60%; background: radial-gradient(closest-side, color-mix(in srgb, var(--accent) 35%, transparent), transparent); filter: blur(10px); animation: breathe 5s ease-in-out infinite; pointer-events: none; }
  .stage :global(canvas) { position: relative; }
  .stage-info { position: relative; display: flex; flex-direction: column; align-items: center; gap: 2px; }
  .stage-name { font-family: var(--display); font-size: 20px; font-weight: 600; }
  .stage-actions { position: relative; min-height: 44px; display: flex; align-items: center; }
  .in-use { display: inline-flex; align-items: center; gap: 6px; color: var(--success-text); font-weight: 600; }
  .note { position: relative; margin: 0; font-size: 12px; color: var(--muted); text-align: center; line-height: 1.5; }
  .side { display: flex; flex-direction: column; gap: 24px; min-width: 0; }
  .section-title { font-size: 16px; font-weight: 600; margin-bottom: 12px; display: flex; align-items: center; gap: 8px; }
  .toggle { background: none; border: 0; padding: 0; color: var(--text); }
  .chev { display: inline-flex; transform: rotate(180deg); transition: transform .2s ease; }
  .chev.open { transform: rotate(-90deg); }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(104px, 1fr)); gap: 12px; }
  .tile-wrap { position: relative; display: flex; flex-direction: column; align-items: center; gap: 6px; }
  .tile { width: 100%; aspect-ratio: 3 / 4; border-radius: 14px; border: 1px solid var(--line); background: var(--surface); display: flex; align-items: center; justify-content: center; position: relative; transition: transform .2s cubic-bezier(.2,.8,.2,1), border-color .2s, background .2s; }
  .tile:hover { transform: translateY(-3px); border-color: var(--line-strong); background: var(--surface-2); }
  .tile.previewing { border-color: var(--accent-line); }
  .tile.selected { border-color: var(--accent); box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 35%, transparent); }
  .tile img { height: 82%; image-rendering: pixelated; transition: transform .25s cubic-bezier(.2,.8,.2,1); }
  .tile:hover img { transform: scale(1.05); }
  .tile.add { border-style: dashed; flex-direction: column; gap: 8px; color: var(--muted); font-size: 13px; }
  .tile.add:hover { color: var(--text); border-color: var(--accent-line); }
  .dot { position: absolute; top: 8px; left: 8px; width: 20px; height: 20px; border-radius: 10px; background: var(--accent); color: #fff; display: flex; align-items: center; justify-content: center; }
  .tile-name { font-size: 12px; color: var(--text-2); max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tile-x { position: absolute; top: 6px; right: 6px; width: 26px; height: 26px; border-radius: 7px; border: 0; background: var(--bg-deep); color: var(--muted); display: flex; align-items: center; justify-content: center; opacity: 0; transition: opacity .15s; }
  .tile-wrap:hover .tile-x { opacity: 1; }
  .tile-x:hover { color: var(--danger-text); }
  .link { margin-top: 10px; background: none; border: 0; padding: 0; color: var(--accent-line); font-size: 13px; }
  .add-form { display: flex; flex-direction: column; gap: 14px; }
  .add-preview { display: flex; justify-content: center; background: var(--panel); border-radius: 12px; }
  fieldset { border: 0; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .models { display: flex; gap: 8px; }
  .models button { flex: 1; height: 40px; border-radius: 10px; border: 1px solid var(--line-strong); background: var(--panel); font-weight: 600; font-size: 13px; transition: background .15s, border-color .15s; }
  .models button.on { border-color: var(--accent-line); background: var(--accent-soft); }
  .actions { display: flex; justify-content: flex-end; gap: 10px; }
  @keyframes breathe { 0%, 100% { opacity: .55; transform: scale(1); } 50% { opacity: .9; transform: scale(1.08); } }
  @media (max-width: 900px) { .layout { grid-template-columns: 1fr; } }
</style>
