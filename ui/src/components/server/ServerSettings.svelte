<script lang="ts">
  // Editor grafico di server.properties più le risorse del server (memoria,
  // avvio in background). Le modifiche valgono dal prossimo avvio.
  import { untrack } from 'svelte';
  import { slide } from 'svelte/transition';
  import { api, type ServerInfo, type ServerProperties } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import { t } from '../../lib/i18n.svelte';
  import { ms } from '../../lib/motion.svelte';
  import Toggle from '../controls/Toggle.svelte';
  import Segmented from '../controls/Segmented.svelte';
  import Range from '../controls/Range.svelte';

  let { server, onchanged, onrestart }: { server: ServerInfo; onchanged: () => Promise<void>; onrestart: () => Promise<void> } = $props();

  const DIFFICULTIES: [ServerProperties['difficulty'], string][] = [['peaceful', 'Pacifica'], ['easy', 'Facile'], ['normal', 'Normale'], ['hard', 'Difficile']];
  const GAMEMODES: [ServerProperties['gamemode'], string][] = [['survival', 'Sopravvivenza'], ['creative', 'Creativa'], ['adventure', 'Avventura'], ['spectator', 'Spettatore']];

  interface Resources { memoryGb: number; keepRunning: boolean; autostart: boolean }

  let draft = $state<ServerProperties | null>(null);
  let saved = $state('');
  let res = $state<Resources>({ memoryGb: 4, keepRunning: false, autostart: false });
  let savedRes = $state('');
  let saving = $state(false);
  /** Salvato a server acceso: serve un riavvio. */
  let pending = $state(false);

  const resourcesOf = (s: ServerInfo): Resources => ({ memoryGb: Math.max(1, Math.round(s.memoryMb / 1024)), keepRunning: s.keepRunning, autostart: s.autostart });

  // Si carica una volta per server: il polling della pagina non deve
  // cancellare ciò che l'utente sta modificando.
  // (`server` è un oggetto nuovo a ogni lettura dell'elenco: si guardano
  // solo i campi che servono, così gli effetti non ripartono ogni 3 s.)
  const serverName = $derived(server.name);
  const startedAt = $derived(server.startedAt);

  $effect(() => {
    const name = serverName;
    untrack(() => {
      res = resourcesOf(server);
      savedRes = JSON.stringify(res);
    });
    api.serverProperties(name)
      .then((p) => { draft = p; saved = JSON.stringify(p); })
      .catch((e) => app.error(e));
  });

  // Un riavvio (nuovo orario di avvio) applica le modifiche in sospeso.
  $effect(() => {
    void startedAt;
    untrack(() => (pending = false));
  });

  $effect(() => {
    if (!res.keepRunning) res.autostart = false;
  });

  const dirtyProps = $derived(!!draft && JSON.stringify(draft) !== saved);
  const dirtyRes = $derived(JSON.stringify(res) !== savedRes);
  const dirty = $derived(dirtyProps || dirtyRes);

  const account = {
    get: () => (draft?.onlineMode ? 'premium' : 'offline') as 'premium' | 'offline',
    set: (v: 'premium' | 'offline') => { if (draft) draft.onlineMode = v === 'premium'; },
  };
  const int = (v: number, min: number, max: number, fallback: number) => (Number.isFinite(v) ? Math.min(max, Math.max(min, Math.round(v))) : fallback);

  async function save() {
    if (!draft || !dirty) return;
    saving = true;
    try {
      const p = $state.snapshot(draft);
      p.maxPlayers = int(p.maxPlayers, 1, 1000, 20);
      p.spawnProtection = int(p.spawnProtection, 0, 1000, 16);
      p.port = int(p.port, 1024, 65535, 25565);
      p.motd = p.motd.replace(/[\r\n]+/g, ' ');
      if (dirtyProps) await api.serverSaveProperties(server.name, p);
      draft = p;
      saved = JSON.stringify(p);
      if (dirtyRes) {
        await api.serverUpdate(server.name, { memoryMb: res.memoryGb * 1024, keepRunning: res.keepRunning, autostart: res.autostart });
        savedRes = JSON.stringify(res);
      }
      if (server.running) pending = true;
      else app.info_(t('Impostazioni del server salvate.'));
      await onchanged();
    } catch (e) {
      app.error(e);
    } finally {
      saving = false;
    }
  }

  async function restart() {
    await onrestart();
    pending = false;
  }
</script>

<section class="card panel" aria-labelledby="settings-{server.name}">
  <div class="head">
    <h2 id="settings-{server.name}">{t('Impostazioni')}</h2>
    {#if pending}
      <span class="pending" transition:slide={{ axis: 'x', duration: ms(200) }}>
        <span>{t('Valgono dal prossimo riavvio')}</span>
        <button class="btn small" onclick={restart}>{t('Riavvia ora')}</button>
      </span>
    {/if}
  </div>

  {#if !draft}
    <div class="skeleton skel"></div>
  {:else}
    <div class="grid">
      <label class="field">{t('Giocatori massimi')}
        <input class="input" type="number" min="1" max="1000" bind:value={draft.maxPlayers} />
      </label>
      <Segmented bind:value={account.get, account.set} label="Account" options={[['premium', 'Solo premium'], ['offline', 'Anche offline']]} />
      <label class="field">{t('Difficoltà')}
        <select class="input" bind:value={draft.difficulty}>
          {#each DIFFICULTIES as [id, label] (id)}<option value={id}>{t(label)}</option>{/each}
        </select>
      </label>
      <label class="field">{t('Modalità di gioco')}
        <select class="input" bind:value={draft.gamemode}>
          {#each GAMEMODES as [id, label] (id)}<option value={id}>{t(label)}</option>{/each}
        </select>
      </label>
    </div>
    {#if !draft.onlineMode}<p class="note small">{t('Con “Anche offline” chiunque può entrare con qualsiasi nome: tieni attiva la whitelist.')}</p>{/if}

    <label class="field">{t('Messaggio del server (MOTD)')}
      <input class="input" bind:value={draft.motd} maxlength="120" />
    </label>

    <div class="toggles">
      <Toggle bind:checked={draft.pvp} label="PvP" />
      <Toggle bind:checked={draft.whitelist} label="Whitelist" />
      <Toggle bind:checked={draft.hardcore} label="Hardcore" />
      <Toggle bind:checked={draft.allowFlight} label="Volo consentito" />
    </div>

    <div class="grid">
      <Range bind:value={draft.viewDistance} label="Distanza di visuale" min={2} max={32} unit=" chunk" />
      <Range bind:value={draft.simulationDistance} label="Distanza di simulazione" min={2} max={32} unit=" chunk" />
      <label class="field">{t('Protezione dello spawn')}
        <input class="input" type="number" min="0" max="1000" bind:value={draft.spawnProtection} />
      </label>
      <label class="field">{t('Porta')}
        <input class="input mono" type="number" min="1024" max="65535" bind:value={draft.port} />
      </label>
    </div>
    <label class="field">{t('Seed del mondo')}
      <input class="input mono" bind:value={draft.seed} placeholder={t('Vuoto = casuale')} spellcheck="false" />
      <span class="muted small">{t('Vale solo quando si genera un mondo nuovo.')}</span>
    </label>

    <h3>{t('Risorse')}</h3>
    <Range bind:value={res.memoryGb} label="Memoria" min={1} max={16} unit=" GB" />
    <Toggle bind:checked={res.keepRunning} label="Resta acceso anche a launcher chiuso" hint="Il server gira in un processo in background, senza finestre." />
    <div class="row" class:disabled={!res.keepRunning}>
      <div class="txt">
        <span class="strong">{t('Avvia con il computer')}</span>
        <span class="muted small">{res.keepRunning ? t('Parte da solo quando accedi a Windows.') : t('Disponibile solo se il server resta acceso a launcher chiuso.')}</span>
      </div>
      <button class="switch" role="switch" aria-checked={res.autostart} aria-label={t('Avvia con il computer')} disabled={!res.keepRunning} onclick={() => (res.autostart = !res.autostart)}></button>
    </div>

    <div class="foot">
      <span class="muted small">{dirty ? t('Modifiche non salvate') : ''}</span>
      <button class="btn primary" disabled={!dirty || saving} onclick={save}>{saving ? t('Salvataggio…') : t('Salva')}</button>
    </div>
  {/if}
</section>

<style>
  .panel { padding: 16px; display: flex; flex-direction: column; gap: 14px; min-width: 0; }
  .head { display: flex; justify-content: space-between; align-items: center; gap: 10px; min-height: 36px; flex-wrap: wrap; }
  h2 { font-size: 16px; font-weight: 600; }
  h3 { font-size: 14px; font-weight: 600; padding-top: 10px; border-top: 1px solid var(--line); }
  .pending { display: flex; align-items: center; gap: 10px; font-size: 12px; color: var(--warn-text); white-space: nowrap; }
  .skel { height: 320px; border-radius: var(--radius-sm); }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 12px; align-items: end; }
  .toggles { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 4px 18px; }
  .note { margin: 0; color: var(--warn-text); }
  .mono { font-family: var(--mono); }
  .row { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 4px 0; }
  .row.disabled .txt { opacity: .7; }
  .txt { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .strong { font-weight: 600; }
  .foot { display: flex; justify-content: space-between; align-items: center; gap: 10px; }
</style>
