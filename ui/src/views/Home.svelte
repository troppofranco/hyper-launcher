<script lang="ts">
  // Home a widget: si riordinano trascinandoli, si sceglie la larghezza,
  // si nascondono o se ne aggiungono dal catalogo (modalità modifica).
  import { t } from '../lib/i18n.svelte';
  import type { WidgetConfig, WidgetSize } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { SIZES, WIDGETS, defaultLayout, newWidget, option, packSpans, widgetDef } from '../lib/widgets';
  import { motionLevel, ms } from '../lib/motion.svelte';
  import Icon from '../components/Icon.svelte';
  import Greeting from '../components/widgets/Greeting.svelte';
  import Quick from '../components/widgets/Quick.svelte';
  import Catalog from '../components/widgets/Catalog.svelte';
  import News from '../components/widgets/News.svelte';
  import Last from '../components/widgets/Last.svelte';
  import Skin from '../components/widgets/Skin.svelte';
  import Stats from '../components/widgets/Stats.svelte';
  import ServerList from '../components/ServerList.svelte';
  import PlaytimeWeek from '../components/PlaytimeWeek.svelte';
  import { flip } from 'svelte/animate';
  import { fly, scale, slide } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';

  // null = mai personalizzata: Home predefinita. Una Home svuotata resta vuota.
  const saved = $derived<WidgetConfig[]>(app.settings?.appearance.home ?? defaultLayout());
  /** Copia su cui si lavora in modalità modifica; salvata con "Fatto". */
  let draft = $state<WidgetConfig[]>([]);
  let saving = $state(false);
  let menu = $state<string | null>(null);
  let dragging = $state<string | null>(null);
  /** Ultimo widget sorvolato: si sposta solo quando cambia, niente scambi avanti e indietro. */
  let lastOver: string | null = null;

  const editing = $derived(app.homeEditing);
  const items = $derived(editing ? draft : saved);
  const hidden = $derived(WIDGETS.filter((w) => !draft.some((d) => d.kind === w.kind)));
  // Larghezze a incastro: ogni riga è piena, senza buchi.
  const spans = $derived(packSpans(items));

  // Entrando in modifica (anche dalle Impostazioni) si parte dalla Home salvata.
  let wasEditing = false;
  $effect(() => {
    if (editing && !wasEditing) draft = structuredClone($state.snapshot(saved));
    wasEditing = editing;
  });

  async function done() {
    if (!app.settings) return;
    saving = true;
    try {
      const settings = $state.snapshot(app.settings);
      settings.appearance.home = $state.snapshot(draft);
      await app.saveSettings(settings);
      app.homeEditing = false;
      menu = null;
    } catch (e) {
      app.error(e);
    } finally {
      saving = false;
    }
  }

  function cancel() {
    app.homeEditing = false;
    menu = null;
  }

  const index = (kind: string) => draft.findIndex((w) => w.kind === kind);

  function setSize(kind: string, size: WidgetSize) {
    draft[index(kind)].size = size;
  }

  function step(kind: string, key: string, delta: number) {
    const w = draft[index(kind)];
    const opt = widgetDef(kind)?.options.find((o) => o.key === key);
    if (!opt) return;
    w.options[key] = Math.min(opt.max, Math.max(opt.min, option(w, key) + delta));
  }

  function hide(kind: string) {
    draft.splice(index(kind), 1);
    menu = null;
  }

  function add(kind: string) {
    draft.push(newWidget(kind));
  }

  /** Sposta il widget trascinato al posto di quello sorvolato. */
  function moveTo(kind: string) {
    if (!dragging || dragging === kind || lastOver === kind) return;
    lastOver = kind;
    const [moved] = draft.splice(index(dragging), 1);
    draft.splice(index(kind), 0, moved);
  }

  // Trascinamento con il puntatore (mouse, penna o dito). Non si usa il
  // drag & drop nativo del browser: nell'app Tauri lo intercetta per il
  // rilascio di file sulla finestra, e il trascinamento non partirebbe.
  // Il widget segue il puntatore; gli altri si spostano per fargli posto.
  function pointerDown(e: PointerEvent, kind: string) {
    if (!editing || e.button !== 0 || (e.target as HTMLElement).closest('.menu, .menu-btn')) return;
    const el = e.currentTarget as HTMLElement;
    const scroller = el.closest('.content') as HTMLElement | null;
    const start = { x: e.clientX, y: e.clientY };
    const rect = el.getBoundingClientRect();
    const grab = { x: e.clientX - rect.left, y: e.clientY - rect.top };
    let pointer = start;
    let raf = 0;

    // Il widget resta sotto il puntatore anche quando il suo posto cambia.
    const follow = () => {
      el.style.transform = '';
      const r = el.getBoundingClientRect();
      el.style.transform = `translate(${pointer.x - grab.x - r.left}px, ${pointer.y - grab.y - r.top}px) scale(1.02)`;
    };
    // Vicino ai bordi della pagina si scorre da soli.
    const autoScroll = () => {
      raf = 0;
      if (!dragging || !scroller) return;
      const box = scroller.getBoundingClientRect();
      const edge = 70;
      const speed = pointer.y < box.top + edge ? -(box.top + edge - pointer.y) : pointer.y > box.bottom - edge ? pointer.y - (box.bottom - edge) : 0;
      if (speed) {
        scroller.scrollTop += speed / 3;
        follow();
        raf = requestAnimationFrame(autoScroll);
      }
    };

    const move = (ev: PointerEvent) => {
      pointer = { x: ev.clientX, y: ev.clientY };
      if (!dragging) {
        if (Math.hypot(pointer.x - start.x, pointer.y - start.y) < 6) return;
        dragging = kind;
        menu = null;
      }
      // Il widget sotto il puntatore, saltando quello trascinato (che lo segue).
      const over = document
        .elementsFromPoint(pointer.x, pointer.y)
        .map((n) => n.closest<HTMLElement>('.w[data-kind]'))
        .find((w) => w && w !== el);
      if (over?.dataset.kind) moveTo(over.dataset.kind);
      follow();
      if (!raf) raf = requestAnimationFrame(autoScroll);
    };
    const end = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', end);
      window.removeEventListener('pointercancel', end);
      cancelAnimationFrame(raf);
      el.style.transform = '';
      dragging = null;
      lastOver = null;
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', end);
    window.addEventListener('pointercancel', end);
  }

  function onWindowClick(e: MouseEvent) {
    if (menu && !(e.target as HTMLElement).closest('.menu, .menu-btn')) menu = null;
  }
</script>

<svelte:window onclick={onWindowClick} onkeydown={(e) => e.key === 'Escape' && (menu ? (menu = null) : editing && cancel())} />

{#snippet widget(w: WidgetConfig)}
  {#if w.kind === 'greeting'}<Greeting />
  {:else if w.kind === 'quick'}<Quick count={option(w, 'count')} />
  {:else if w.kind === 'catalog'}<Catalog count={option(w, 'count')} />
  {:else if w.kind === 'servers'}<ServerList limit={option(w, 'count')} showTitle={false} />
  {:else if w.kind === 'playtime'}<PlaytimeWeek showTitle={false} />
  {:else if w.kind === 'news'}<News count={option(w, 'count')} />
  {:else if w.kind === 'last'}<Last />
  {:else if w.kind === 'skin'}<Skin />
  {:else if w.kind === 'stats'}<Stats />
  {/if}
{/snippet}

<div class="page home">
  {#if editing}
    <div class="editbar" transition:slide={{ duration: ms(220) }}>
      <div class="txt">
        <span class="strong">{t('Stai modificando la Home')}</span>
        <span class="small">{t('Trascina i widget per riordinarli. Dal menu ⋯ scegli la larghezza, le opzioni o nascondili.')}</span>
      </div>
      <div class="actions">
        <button class="btn" onclick={() => (draft = defaultLayout())}>{t('Ripristina')}</button>
        <button class="btn" onclick={cancel}>{t('Annulla')}</button>
        <button class="btn primary" disabled={saving} onclick={done}><Icon name="check" size={16} /> {t('Fatto')}</button>
      </div>
    </div>
  {:else}
    <div class="tools">
      <button class="btn small ghost" onclick={() => (app.homeEditing = true)}><Icon name="list" size={15} /> {t('Personalizza')}</button>
    </div>
  {/if}

  <div class="grid" class:editing>
    {#each items as w, i (w.kind)}
      {@const def = widgetDef(w.kind)}
      <section
        class="w"
        style:grid-column="span {spans.get(w.kind) ?? 12}"
        class:dragging={dragging === w.kind}
        class:wiggle={editing && motionLevel() === 'full' && dragging === null}
        style:--i={i}
        aria-label={def ? t(def.name) : w.kind}
        data-kind={w.kind}
        onpointerdown={(e) => pointerDown(e, w.kind)}
        animate:flip={{ duration: dragging === w.kind ? 0 : ms(260), easing: cubicOut }}
        in:fly={{ y: 12, duration: ms(300), delay: editing ? 0 : ms(Math.min(i, 8) * 45), easing: cubicOut }}
      >
        {#if editing}
          <div class="chrome">
            <span class="grip" aria-hidden="true"><svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><circle cx="9" cy="6" r="1.6" /><circle cx="15" cy="6" r="1.6" /><circle cx="9" cy="12" r="1.6" /><circle cx="15" cy="12" r="1.6" /><circle cx="9" cy="18" r="1.6" /><circle cx="15" cy="18" r="1.6" /></svg></span>
            <span class="label">{def ? t(def.name) : w.kind} · {t(SIZES.find(([s]) => s === w.size)?.[1] ?? '')}</span>
            <button class="menu-btn" aria-label={t('Opzioni del widget')} aria-expanded={menu === w.kind} onclick={() => (menu = menu === w.kind ? null : w.kind)}>⋯</button>
            {#if menu === w.kind && def}
              <div class="menu" role="dialog" aria-label={t('Opzioni del widget')} transition:scale={{ start: 0.95, duration: ms(160), easing: cubicOut }}>
                <span class="mlabel">{t('Larghezza')}</span>
                <div class="sizes">
                  {#each SIZES as [size, label] (size)}
                    <button class:on={w.size === size} aria-pressed={w.size === size} onclick={() => setSize(w.kind, size)}>{t(label)}</button>
                  {/each}
                </div>
                {#each def.options as opt (opt.key)}
                  <span class="mlabel">{t(opt.label)}</span>
                  <div class="stepper">
                    <button aria-label={t('Meno')} disabled={option(w, opt.key) <= opt.min} onclick={() => step(w.kind, opt.key, -1)}>−</button>
                    <span>{option(w, opt.key)}</span>
                    <button aria-label={t('Più')} disabled={option(w, opt.key) >= opt.max} onclick={() => step(w.kind, opt.key, 1)}>+</button>
                  </div>
                {/each}
                <span class="msep"></span>
                <button class="hide" onclick={() => hide(w.kind)}><Icon name="close" size={14} /> {t('Nascondi widget')}</button>
              </div>
            {/if}
          </div>
        {/if}
        {#if def?.title && !editing}<h2 class="section-title">{t(def.title)}</h2>{/if}
        <div class="body" inert={editing}>{@render widget(w)}</div>
      </section>
    {/each}
  </div>

  {#if !editing && !items.length}
    <div class="card empty-home">
      <p class="muted">{t('La Home è vuota.')}</p>
      <button class="btn primary" onclick={() => (app.homeEditing = true)}>{t('Aggiungi widget')}</button>
    </div>
  {/if}

  {#if editing}
    <section class="catalog" transition:slide={{ duration: ms(220) }}>
      <h2 class="section-title">{t('Aggiungi widget')}</h2>
      {#if hidden.length}
        <div class="adds">
          {#each hidden as def (def.kind)}
            <button class="add" onclick={() => add(def.kind)} in:scale={{ start: 0.9, duration: ms(180) }}>
              <span class="plus">+</span>
              <span class="atxt"><span class="strong">{t(def.name)}</span><span class="muted small">{t(def.description)}</span></span>
            </button>
          {/each}
        </div>
      {:else}
        <p class="muted small">{t('Tutti i widget sono già nella Home.')}</p>
      {/if}
    </section>
  {/if}
</div>

<style>
  .home { gap: 18px; }
  .strong { font-weight: 600; }
  .tools { display: flex; justify-content: flex-end; margin-bottom: -10px; }
  .ghost { border-color: transparent; color: var(--muted); }
  .ghost:hover { color: var(--text); }
  .editbar { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 12px 16px; border-radius: var(--radius); background: var(--accent-soft); border: 1px solid var(--accent-line); flex-wrap: wrap; position: sticky; top: 0; z-index: 5; backdrop-filter: blur(12px); }
  .editbar .txt { display: flex; flex-direction: column; gap: 2px; }
  .editbar .small { color: var(--text-2); }
  .actions { display: flex; gap: 8px; }
  /* Griglia a incastro: le righe sono piene (vedi packSpans) e i widget di
     una riga hanno la stessa altezza, così i bordi si allineano. */
  .grid { display: grid; grid-template-columns: repeat(12, minmax(0, 1fr)); gap: 16px; align-items: stretch; container-type: inline-size; }
  .w { position: relative; min-width: 0; display: flex; flex-direction: column; container: widget / inline-size; padding: 16px 18px; border-radius: var(--radius); background: var(--surface); border: 1px solid var(--line); }
  .body { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  /* Il contenuto di un widget sta nella sua tessera: la sua card esterna
     diventa piatta e riempie lo spazio; quelle interne restano distinte. */
  .body :global(.card) { background: color-mix(in srgb, var(--surface-2) 70%, transparent); }
  .body > :global(.card), .body > :global(section > .card:only-child) { flex: 1; background: transparent; border: 0; box-shadow: none; padding: 0; transform: none; }
  /* Finestra stretta: un widget per riga. */
  @container (max-width: 720px) {
    .w { grid-column: span 12 !important; }
  }
  .section-title { font-size: 16px; font-weight: 600; margin: 0 0 12px; }
  .editing .w { border: 1.5px dashed var(--line-strong); cursor: grab; transition: border-color .2s ease, box-shadow .2s ease, opacity .2s ease; }
  .editing .w:hover { border-color: var(--accent-line); }
  .editing .w { touch-action: none; }
  /* Il widget in mano: sopra agli altri, ombra profonda, non intercetta il puntatore. */
  .editing .w.dragging { z-index: 20; pointer-events: none; cursor: grabbing; border-color: var(--accent); border-style: solid; background: var(--c-surface); box-shadow: 0 24px 60px var(--shadow), 0 0 0 1px var(--accent); transition: box-shadow .2s ease; }
  .editing .body { pointer-events: none; user-select: none; max-height: 340px; overflow: hidden; }
  .wiggle { animation: wiggle .5s ease-in-out infinite alternate; animation-delay: calc(var(--i) * -70ms); }
  @keyframes wiggle { from { transform: rotate(-.25deg); } to { transform: rotate(.25deg); } }
  .chrome { position: relative; display: flex; align-items: center; gap: 8px; margin-bottom: 10px; font-size: 12px; color: var(--muted); }
  .grip { display: flex; color: var(--faint); }
  .label { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; }
  .menu-btn { width: 34px; height: 28px; border-radius: 8px; border: 1px solid var(--line-strong); background: var(--surface-2); color: var(--text-2); font-size: 16px; line-height: 1; cursor: pointer; }
  .menu-btn[aria-expanded='true'] { border-color: var(--accent-line); background: var(--accent-soft); color: var(--text); }
  .menu { position: absolute; right: 0; top: 34px; z-index: 10; width: 230px; padding: 10px; border-radius: 12px; background: var(--c-surface-2); border: 1px solid var(--line-strong); box-shadow: 0 18px 40px var(--shadow); display: flex; flex-direction: column; gap: 6px; transform-origin: top right; cursor: default; }
  .mlabel { font-size: 11px; color: var(--muted); text-transform: uppercase; letter-spacing: .06em; padding: 4px 2px 0; }
  .sizes { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 4px; }
  .sizes button { height: 32px; border-radius: 8px; border: 1px solid var(--line-strong); background: transparent; color: var(--text-2); font-size: 12px; font-weight: 600; }
  .sizes button.on { border-color: var(--accent-line); background: var(--accent-soft); color: var(--text); }
  .stepper { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .stepper button { width: 32px; height: 30px; border-radius: 8px; border: 1px solid var(--line-strong); background: transparent; font-size: 16px; }
  .stepper span { font-family: var(--mono); font-weight: 600; }
  .msep { height: 1px; background: var(--line-strong); margin: 4px 0; }
  .hide { display: flex; align-items: center; gap: 8px; height: 34px; padding: 0 8px; border-radius: 8px; border: 0; background: transparent; color: var(--danger-text); font-weight: 600; text-align: left; }
  .hide:hover { background: color-mix(in srgb, var(--danger) 12%, transparent); }
  .empty-home { padding: 28px; display: flex; flex-direction: column; align-items: center; gap: 12px; }
  .empty-home p { margin: 0; }
  .catalog { display: flex; flex-direction: column; }
  .catalog p { margin: 0; }
  .adds { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 10px; }
  .add { display: flex; align-items: center; gap: 12px; padding: 12px 14px; border-radius: var(--radius); border: 1px dashed var(--accent-line); background: transparent; text-align: left; color: var(--text); transition: background .18s ease, transform .15s ease; }
  .add:hover { background: var(--accent-soft); }
  .add:active { transform: scale(.98); }
  .plus { width: 30px; height: 30px; flex: none; border-radius: 15px; background: var(--accent); color: #fff; display: flex; align-items: center; justify-content: center; font-size: 18px; font-weight: 600; }
  .atxt { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
</style>
