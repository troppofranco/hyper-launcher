<script lang="ts">
  import { api, on, type DeviceCodeEvent } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import Modal from './Modal.svelte';

  // Senza client ID il login Microsoft non può funzionare: si parte da Offline.
  const msaReady = $derived(!!app.settings?.msaClientId.trim());
  let tab = $state<'microsoft' | 'offline'>(app.settings?.msaClientId.trim() ? 'microsoft' : 'offline');
  let offlineName = $state('');
  let busy = $state(false);
  let code = $state<DeviceCodeEvent | null>(null);
  let error = $state<string | null>(null);

  const validName = $derived(/^[A-Za-z0-9_]{3,16}$/.test(offlineName));

  function close() {
    app.addAccountOpen = false;
  }

  async function addOffline(e: SubmitEvent) {
    e.preventDefault();
    if (!validName) return;
    busy = true;
    error = null;
    try {
      app.accounts = await api.addOfflineAccount(offlineName);
      close();
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function loginMicrosoft() {
    busy = true;
    error = null;
    code = null;
    const unlisten = await on('msa-code', (c) => (code = c));
    try {
      app.accounts = await api.addMicrosoftAccount();
      close();
    } catch (err) {
      error = String(err);
    } finally {
      unlisten();
      busy = false;
      code = null;
    }
  }
</script>

<Modal title="Aggiungi account" onclose={close} width={520}>
  <div class="tabs" role="tablist">
    <button role="tab" aria-selected={tab === 'microsoft'} class:on={tab === 'microsoft'} disabled={busy} onclick={() => (tab = 'microsoft')}>Microsoft</button>
    <button role="tab" aria-selected={tab === 'offline'} class:on={tab === 'offline'} disabled={busy} onclick={() => (tab = 'offline')}>Offline</button>
  </div>

  {#if tab === 'microsoft'}
    <p class="muted">Accedi con l'account Microsoft che possiede Minecraft: Java Edition. I token restano solo nel portachiavi di Windows.</p>
    {#if code}
      <div class="code-box">
        <span class="label">Codice</span>
        <span class="code">{code.userCode}</span>
        <span class="muted">Si è aperta la pagina <button class="link" onclick={() => code && api.openUrl(code.verificationUri)}>{code.verificationUri}</button>: inserisci il codice e conferma. In attesa…</span>
      </div>
    {/if}
    {#if !msaReady}
      <p class="notice">Il login Microsoft non è ancora disponibile in questa versione. Per ora usa un account <button class="link" onclick={() => (tab = 'offline')}>offline</button>.</p>
    {/if}
    <button class="btn primary" disabled={busy || !msaReady} onclick={loginMicrosoft}>{busy ? 'In attesa della conferma…' : 'Accedi con Microsoft'}</button>
  {:else}
    <form onsubmit={addOffline} class="offline">
      <label class="field">Nome giocatore
        <input class="input" bind:value={offlineName} maxlength="16" placeholder="Steve" autocomplete="off" />
      </label>
      <span class="muted small">3–16 caratteri: lettere, numeri e _. Gli account offline funzionano solo in singolo giocatore e sui server in modalità offline.</span>
      <button class="btn primary" type="submit" disabled={busy || !validName}>Aggiungi</button>
    </form>
  {/if}

  {#if error}<p class="error-box" role="alert">{error}</p>{/if}
</Modal>

<style>
  .tabs { display: flex; gap: 6px; background: var(--panel); padding: 4px; border-radius: 12px; }
  .tabs button { flex: 1; height: 38px; border: 0; border-radius: 9px; background: transparent; color: var(--muted); font-weight: 600; }
  .tabs button.on { background: var(--surface-2); color: var(--text); }
  p { margin: 0; }
  .offline { display: flex; flex-direction: column; gap: 12px; }
  .code-box { display: flex; flex-direction: column; gap: 6px; padding: 16px; border-radius: 12px; background: var(--panel); border: 1px solid var(--line-strong); }
  .code { font-family: var(--mono); font-size: 28px; letter-spacing: .12em; font-weight: 600; user-select: text; }
  .notice { padding: 12px 14px; border-radius: 10px; background: var(--panel); border: 1px solid var(--line-strong); }
  .link { background: none; border: 0; padding: 0; color: var(--accent-line); text-decoration: underline; }
</style>
