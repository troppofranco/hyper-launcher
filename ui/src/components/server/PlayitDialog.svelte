<script lang="ts">
  // Accesso da internet con playit.gg, in tre passi: collega l'account
  // (approvazione nel browser, una volta sola), attiva il tunnel del server,
  // condividi l'indirizzo. Se è già attivo mostra l'indirizzo e le opzioni.
  import { api, on, type PlayitClaimStatus, type PlayitProgressEvent, type ServerInfo } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import { t } from '../../lib/i18n.svelte';
  import Icon from '../Icon.svelte';
  import Modal from '../Modal.svelte';
  import { copyText } from './util';

  let { server, onclose, onchanged }: {
    server: ServerInfo;
    onclose: () => void;
    onchanged: (s: ServerInfo) => void;
  } = $props();

  let connected = $state<boolean | null>(null);
  let claim = $state<PlayitClaimStatus | null>(null);
  let connecting = $state(false);
  let stage = $state<PlayitProgressEvent['stage'] | null>(null);
  let enabling = $state(false);
  let busy = $state(false);

  $effect(() => {
    api.playitStatus().then((s) => (connected = s.connected)).catch((e) => app.error(e));
  });

  $effect(() => {
    const offClaim = on('playit-claim', (s) => (claim = s));
    const offProgress = on('playit-progress', (p) => { if (p.name === server.name) stage = p.stage; });
    return () => {
      void offClaim.then((f) => f());
      void offProgress.then((f) => f());
    };
  });

  async function connect() {
    connecting = true;
    claim = null;
    try {
      await api.playitConnect();
      connected = true;
    } catch (e) {
      app.error(e);
    } finally {
      connecting = false;
    }
  }

  async function enable() {
    enabling = true;
    stage = null;
    try {
      onchanged(await api.serverPlayitEnable(server.name));
    } catch (e) {
      app.error(e);
    } finally {
      enabling = false;
    }
  }

  async function act(op: () => Promise<unknown>) {
    busy = true;
    try {
      await op();
    } catch (e) {
      app.error(e);
    } finally {
      busy = false;
    }
  }

  const claimText: Record<PlayitClaimStatus, string> = {
    WaitingForUserVisit: 'Si è aperto il browser: accedi a playit.gg (o crea un account gratuito)…',
    WaitingForUser: 'Ora premi “Approva” nella pagina di playit.gg…',
    UserAccepted: 'Approvato! Ultimo passaggio…',
    UserRejected: 'Collegamento rifiutato.',
  };
  const stageText: Record<PlayitProgressEvent['stage'], string> = {
    agent: 'Preparo il programma di playit…',
    tunnel: 'Creo il tunnel verso il server…',
    address: 'Aspetto l’indirizzo pubblico…',
  };

  const step = $derived(server.publicAddress ? 3 : connected ? 2 : 1);
</script>

<Modal title={t('Gioca con amici fuori casa')} {onclose} width={520}>
  <div class="wrap">
    {#if !server.publicAddress}
      <p class="intro">{t('Con playit.gg (gratis) il server diventa raggiungibile da internet con un indirizzo da condividere: niente porte da aprire sul router.')}</p>
      <ol class="steps">
        <li class:done={step > 1} class:on={step === 1}>
          <span class="dot">{#if step > 1}<Icon name="check" size={12} />{:else}1{/if}</span>
          <div class="body">
            <strong>{t('Collega playit.gg')}</strong>
            {#if connected === null}
              <span class="muted small">{t('Controllo…')}</span>
            {:else if connected}
              <span class="muted small">{t('Collegato a questo PC.')}</span>
            {:else if connecting}
              <span class="small live">{t(claim ? claimText[claim] : 'Apro il browser…')}</span>
            {:else}
              <span class="muted small">{t('Si apre il browser: accedi e approva Hyper Launcher. Serve una volta sola per tutti i server.')}</span>
              <button class="btn primary small" onclick={connect}><Icon name="external" size={14} /> {t('Collega')}</button>
            {/if}
          </div>
        </li>
        <li class:on={step === 2}>
          <span class="dot">2</span>
          <div class="body">
            <strong>{t('Attiva l’accesso da internet')}</strong>
            {#if enabling}
              <span class="small live">{t(stage ? stageText[stage] : 'Avvio…')}</span>
            {:else}
              <span class="muted small">{t('Crea un tunnel Minecraft Java verso la porta {port} del server.', { port: server.port })}</span>
              {#if step === 2}<button class="btn primary small" onclick={enable}><Icon name="globe" size={14} /> {t('Attiva')}</button>{/if}
            {/if}
          </div>
        </li>
      </ol>
    {:else}
      <div class="ready">
        <span class="label">{t('Indirizzo da dare agli amici')}</span>
        <div class="addr-row">
          <span class="addr">{server.publicAddress}</span>
          <button class="btn primary small" onclick={() => copyText(server.publicAddress!)}><Icon name="copy" size={14} /> {t('Copia')}</button>
        </div>
        <p class="muted small">{server.running
          ? t('Il server è raggiungibile da internet finché è acceso.')
          : t('Funziona quando il server è acceso: avvialo e passa l’indirizzo ai tuoi amici.')}</p>
      </div>
      <div class="manage">
        <button class="btn small" disabled={busy} onclick={() => act(async () => onchanged(await api.serverPlayitDisable(server.name)))}>{t('Disattiva accesso da internet')}</button>
        <button class="btn small danger" disabled={busy} onclick={() => act(async () => { await api.playitDisconnect(); connected = false; onchanged({ ...server, publicAddress: null }); })}>{t('Scollega playit.gg')}</button>
      </div>
    {/if}
  </div>
</Modal>

<style>
  .wrap { display: flex; flex-direction: column; gap: 16px; }
  .intro { margin: 0; line-height: 1.5; color: var(--text-2); }
  .steps { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 10px; }
  .steps li { display: flex; gap: 12px; padding: 12px 14px; border-radius: 12px; border: 1px solid var(--line); opacity: .6; transition: opacity .2s, border-color .2s; }
  .steps li.on, .steps li.done { opacity: 1; }
  .steps li.on { border-color: color-mix(in srgb, var(--accent) 55%, transparent); background: var(--accent-soft); }
  .dot { flex: none; width: 24px; height: 24px; border-radius: 12px; display: flex; align-items: center; justify-content: center; font-size: 12px; font-weight: 700; background: var(--surface-2); color: var(--text-2); }
  .done .dot { background: color-mix(in srgb, var(--success) 25%, transparent); color: var(--success-text); }
  .on .dot { background: var(--accent); color: #fff; }
  .body { flex: 1; min-width: 0; display: flex; flex-direction: column; align-items: flex-start; gap: 6px; }
  .live { color: var(--accent-line); }
  .ready { display: flex; flex-direction: column; gap: 8px; padding: 14px; border-radius: 12px; background: var(--surface-2); }
  .label { font-size: 12px; color: var(--muted); font-weight: 600; }
  .addr-row { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .addr { flex: 1; min-width: 0; font-family: var(--mono); font-size: 15px; color: var(--text); user-select: text; overflow-wrap: anywhere; }
  .ready p { margin: 0; }
  .manage { display: flex; gap: 10px; justify-content: flex-end; flex-wrap: wrap; }
</style>
