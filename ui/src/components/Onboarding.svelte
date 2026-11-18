<script lang="ts">
  // Configurazione iniziale, al primo avvio: lingua, primo account, aspetto.
  // Le scelte d'aspetto si vedono subito; si salvano tutte alla fine.
  import { api, type Settings } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { LANGUAGES, t, type Lang } from '../lib/i18n.svelte';
  import { ACCENTS, THEMES } from '../lib/look';
  import { VALID_PLAYER_NAME } from './AddAccountDialog.svelte';
  import Icon from './Icon.svelte';
  import { fly } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { prefersReducedMotion } from 'svelte/motion';

  const STEPS = ['lang', 'account', 'look'] as const;
  let step = $state(0);
  // Avanti o indietro: la pagina nuova entra dal lato giusto.
  let dir = $state(1);
  let draft = $state<Settings>($state.snapshot(app.settings!));
  let playerName = $state('');
  let busy = $state(false);
  let error = $state<string | null>(null);

  const motion = (x: number) => ({ x: prefersReducedMotion.current ? 0 : x, duration: prefersReducedMotion.current ? 0 : 320, easing: cubicOut });
  const validName = $derived(VALID_PLAYER_NAME.test(playerName));
  const hasAccount = $derived(app.accounts.length > 0);

  // Anteprima immediata di lingua, tema e colore.
  $effect(() => {
    app.applySettings($state.snapshot(draft));
  });

  function go(delta: number) {
    error = null;
    dir = delta;
    step = Math.max(0, Math.min(STEPS.length - 1, step + delta));
  }

  async function createAccount(e: SubmitEvent) {
    e.preventDefault();
    if (!validName) return;
    busy = true;
    error = null;
    try {
      app.accounts = await api.addOfflineAccount(playerName);
      go(1);
    } catch (err) {
      error = t(String(err));
    } finally {
      busy = false;
    }
  }

  async function finish() {
    busy = true;
    error = null;
    try {
      const done = { ...$state.snapshot(draft), setupDone: true };
      await api.saveSettings(done);
      app.applySettings(done);
    } catch (err) {
      error = t(String(err));
      busy = false;
    }
  }
</script>

<div class="onboarding">
  <div class="glow" aria-hidden="true"></div>
  <div class="card box">
    <div class="steps" aria-hidden="true">
      {#each STEPS as s, i (s)}<span class="pip" class:on={i <= step}></span>{/each}
    </div>

    {#key step}
      <div class="pane" in:fly={motion(dir * 40)}>
        {#if STEPS[step] === 'lang'}
          <div class="logo"><Icon name="play" size={26} /></div>
          <h1>{t('Benvenuto in Hyper Launcher')}</h1>
          <p class="muted">{t('Bastano tre passaggi per iniziare. Prima di tutto, la lingua.')}</p>
          <div class="options">
            {#each LANGUAGES as [id, label] (id)}
              <button class="option" class:on={draft.language === id} aria-pressed={draft.language === id} onclick={() => (draft.language = id as Lang)}>
                <span class="code">{id.toUpperCase()}</span>{label}
              </button>
            {/each}
          </div>
        {:else if STEPS[step] === 'account'}
          <h1>{hasAccount ? t('Il tuo account') : t('Crea il tuo account')}</h1>
          {#if hasAccount}
            <p class="muted">{t('Hai già un account: {nome}. Puoi aggiungerne altri quando vuoi dal menu in alto a destra.', { nome: app.activeAccount?.name ?? '' })}</p>
          {:else}
            <p class="muted">{t('Scegli il nome con cui giocare. È un account offline: funziona in singolo giocatore e sui server in modalità offline.')}</p>
            <form class="form" id="account-form" onsubmit={createAccount}>
              <label class="field">{t('Nome giocatore')}
                <!-- svelte-ignore a11y_autofocus -->
                <input class="input" bind:value={playerName} maxlength="16" placeholder="Steve" autocomplete="off" autofocus />
              </label>
              <span class="muted small">{t('3–16 caratteri: lettere, numeri e _.')}</span>
            </form>
          {/if}
        {:else}
          <h1>{t('Scegli il tuo stile')}</h1>
          <p class="muted">{t('Puoi cambiarlo quando vuoi in Impostazioni → Aspetto.')}</p>
          <span class="strong">{t('Tema')}</span>
          <div class="options">
            {#each THEMES as [id, label] (id)}
              <button class="option" class:on={draft.theme === id} aria-pressed={draft.theme === id} onclick={() => (draft.theme = id)}>
                <span class="mini {id}"></span>{t(label)}
              </button>
            {/each}
          </div>
          <span class="strong">{t('Colore accento')}</span>
          <div class="swatches">
            {#each ACCENTS as [color, label] (color)}
              <button class="swatch" class:on={draft.accent === color} style:background={color} aria-label={t(label)} title={t(label)} aria-pressed={draft.accent === color} onclick={() => (draft.accent = color)}></button>
            {/each}
          </div>
        {/if}
      </div>
    {/key}

    {#if error}<p class="error-box" role="alert">{error}</p>{/if}

    <div class="actions">
      {#if step > 0}<button class="btn" disabled={busy} onclick={() => go(-1)}>{t('Indietro')}</button>{/if}
      <span class="spacer"></span>
      {#if STEPS[step] === 'account' && !hasAccount}
        <button class="btn ghost" disabled={busy} onclick={() => go(1)}>{t('Più tardi')}</button>
        <button class="btn primary" type="submit" form="account-form" disabled={busy || !validName}>{t('Crea account')}</button>
      {:else if step < STEPS.length - 1}
        <button class="btn primary" onclick={() => go(1)}>{t('Avanti')}</button>
      {:else}
        <button class="btn primary" disabled={busy} onclick={finish}>{t('Inizia a giocare')}</button>
      {/if}
    </div>
  </div>
</div>

<style>
  .onboarding { position: relative; flex: 1; display: flex; align-items: center; justify-content: center; padding: 24px; overflow: hidden; }
  .glow { position: absolute; width: 640px; height: 640px; border-radius: 50%; background: radial-gradient(circle, color-mix(in srgb, var(--accent) 30%, transparent), transparent 65%); filter: blur(20px); animation: drift 9s ease-in-out infinite alternate; pointer-events: none; }
  @keyframes drift { from { transform: translate(-60px, -30px) scale(1); } to { transform: translate(60px, 30px) scale(1.12); } }
  .box { position: relative; width: min(520px, 100%); padding: 32px; display: flex; flex-direction: column; gap: 20px; box-shadow: 0 24px 60px var(--shadow); animation: rise .5s var(--ease) backwards; }
  .steps { display: flex; gap: 6px; }
  .pip { flex: 1; height: 4px; border-radius: 2px; background: var(--line-strong); transition: background .3s ease; }
  .pip.on { background: var(--accent); }
  .pane { display: flex; flex-direction: column; gap: 14px; min-height: 230px; }
  .logo { width: 52px; height: 52px; border-radius: 14px; background: var(--accent); color: #fff; display: flex; align-items: center; justify-content: center; }
  h1 { font-family: var(--display); font-size: 26px; font-weight: 600; }
  p { margin: 0; line-height: 1.5; }
  .strong { font-weight: 600; }
  .options { display: flex; gap: 10px; flex-wrap: wrap; }
  .option { flex: 1 1 140px; display: flex; align-items: center; gap: 12px; height: 56px; padding: 0 14px; border-radius: 12px; border: 1px solid var(--line-strong); background: var(--panel); font-weight: 600; transition: border-color .18s ease, background .18s ease, transform .12s ease; }
  .option:hover { border-color: var(--accent-line); }
  .option:active { transform: scale(.98); }
  .option.on { border-color: var(--accent-line); background: var(--accent-soft); }
  .code { font-family: var(--mono); font-size: 12px; padding: 4px 6px; border-radius: 6px; background: var(--surface-2); color: var(--muted); }
  .mini { width: 30px; height: 30px; border-radius: 8px; border: 1px solid var(--line-strong); }
  .mini.dark { background: linear-gradient(135deg, #0f1115 50%, #1e222b 50%); }
  .mini.light { background: linear-gradient(135deg, #ffffff 50%, #e8eaef 50%); }
  .swatches { display: flex; gap: 10px; }
  .swatch { width: 40px; height: 40px; border-radius: 20px; border: 3px solid transparent; transition: transform .15s ease; }
  .swatch:hover { transform: scale(1.08); }
  .swatch.on { border-color: var(--text); }
  .form { display: flex; flex-direction: column; gap: 8px; }
  .actions { display: flex; gap: 10px; align-items: center; }
  .spacer { flex: 1; }
  .ghost { border-color: transparent; color: var(--muted); }
</style>
