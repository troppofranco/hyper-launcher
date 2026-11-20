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
  /** Ultimo errore, mostrato nel passo con "Riprova" (oltre al messaggio). */
  let failure = $state<{ step: 1 | 2; message: string } | null>(null);
  let guide = $state(false);

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
    failure = null;
    try {
      await api.playitConnect();
      connected = true;
    } catch (e) {
      failure = { step: 1, message: String(e) };
    } finally {
      connecting = false;
    }
  }

  async function enable() {
    enabling = true;
    stage = null;
    failure = null;
    try {
      onchanged(await api.serverPlayitEnable(server.name));
    } catch (e) {
      failure = { step: 2, message: String(e) };
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
            {:else if failure?.step === 1}
              <span class="small warn">{failure.message}</span>
              <button class="btn primary small" onclick={connect}>{t('Riprova')}</button>
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
            {:else if failure?.step === 2}
              <span class="small warn">{failure.message}</span>
              <span class="muted small">{t('Il lavoro già fatto non si perde: riprovando si riparte da dove si era fermato.')}</span>
              <button class="btn primary small" onclick={enable}>{t('Riprova')}</button>
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

    <section class="guide">
      <button type="button" class="guide-head" aria-expanded={guide} onclick={() => (guide = !guide)}>
        <span class="chev" class:open={guide}><Icon name="chevron" size={14} /></span> {t('Guida passo passo')}
      </button>
      {#if guide}
        <ol class="howto">
          <li><strong>{t('Crea un account playit.gg')}</strong> <span>{t('È gratis: basta un’email (o l’accesso con Google). Puoi farlo anche durante il collegamento, nella pagina che si apre.')}</span></li>
          <li><strong>{t('Collega il launcher')}</strong> <span>{t('Premi “Collega”: nel browser accedi e premi “Approva”. Il launcher salva la chiave solo nel portachiavi di Windows di questo PC.')}</span></li>
          <li><strong>{t('Attiva l’accesso da internet')}</strong> <span>{t('Premi “Attiva”: il launcher scarica il programma ufficiale di playit (verificato), crea il tunnel e aspetta l’indirizzo. Può volerci un paio di minuti la prima volta.')}</span></li>
          <li><strong>{t('Avvia il server')}</strong> <span>{t('Il tunnel funziona solo a server acceso. Se il server resta acceso a launcher chiuso, anche il tunnel resta attivo.')}</span></li>
          <li><strong>{t('Condividi l’indirizzo')}</strong> <span>{t('Gli amici aprono Minecraft → Multigiocatore → Aggiungi server e incollano l’indirizzo (es. nome.joinmc.link). Devono avere la stessa versione del gioco.')}</span></li>
        </ol>
        <h4>{t('Se qualcosa non va')}</h4>
        <ul class="howto tips">
          <li>{t('Si è fermato con un errore? Premi “Riprova”: il launcher riprova già da solo gli intoppi di rete, e il tunnel creato non va perso.')}</li>
          <li>{t('Gli amici non entrano? Controlla che il server sia acceso e aspetta un minuto dopo l’avvio: il tunnel si collega poco dopo il server.')}</li>
          <li>{t('“Collegamento non più valido”? L’agent è stato eliminato dal sito di playit: premi di nuovo “Collega”.')}</li>
          <li>{t('L’antivirus blocca playit? Consenti “playitd.exe” nella cartella del launcher.')}</li>
          <li>{t('I tunnel e gli agent si vedono (e si possono eliminare) anche dal sito playit.gg, nella sezione Agents.')}</li>
        </ul>
        <button type="button" class="btn small link-btn" onclick={() => api.openUrl('https://playit.gg/account/agents')}><Icon name="external" size={14} /> {t('Apri playit.gg')}</button>
      {/if}
    </section>
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
  .warn { color: var(--warn-text); overflow-wrap: anywhere; }
  .guide { display: flex; flex-direction: column; gap: 10px; border-top: 1px solid var(--line); padding-top: 12px; }
  .guide-head { display: flex; align-items: center; gap: 6px; align-self: flex-start; background: none; border: 0; padding: 0; color: var(--text-2); font-weight: 600; font-size: 13px; }
  .guide-head:hover { color: var(--text); }
  .chev { display: flex; transition: transform .2s ease; }
  .chev.open { transform: rotate(90deg); }
  .howto { margin: 0; padding-left: 20px; display: flex; flex-direction: column; gap: 8px; font-size: 13px; line-height: 1.5; color: var(--text-2); }
  .howto strong { color: var(--text); }
  .tips { list-style: disc; }
  h4 { margin: 4px 0 0; font-size: 13px; font-weight: 600; }
  .link-btn { align-self: flex-start; }
  .manage { display: flex; gap: 10px; justify-content: flex-end; flex-wrap: wrap; }
</style>
