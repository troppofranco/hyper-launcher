<script lang="ts">
  // Automatismi del server: riavvio ogni giorno a un'ora (con avvisi in
  // chat) e backup automatici. Li esegue l'host, anche a launcher chiuso;
  // ogni modifica si salva da sola.
  import { untrack } from 'svelte';
  import { api, type ServerInfo, type ServerSchedule } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import { t } from '../../lib/i18n.svelte';

  let { server, onchanged }: { server: ServerInfo; onchanged: (s: ServerInfo) => void } = $props();

  const HOURS = [1, 2, 3, 6, 12, 24];

  // Bozza locale: parte dai valori salvati del server aperto.
  let restartOn = $state(untrack(() => !!server.schedule.restart_at));
  let restartAt = $state(untrack(() => server.schedule.restart_at ?? '05:00'));
  let backupOn = $state(untrack(() => server.schedule.backup_hours !== null));
  let backupHours = $state(untrack(() => server.schedule.backup_hours ?? 6));
  let keep = $state(untrack(() => server.schedule.keep_backups));
  let saving = $state(false);

  const schedule = $derived<ServerSchedule>({
    restart_at: restartOn ? restartAt : null,
    backup_hours: backupOn ? backupHours : null,
    keep_backups: Math.max(1, Math.min(100, Math.round(keep) || 10)),
  });
  const changed = $derived(JSON.stringify(schedule) !== JSON.stringify(server.schedule));

  // Salvataggio automatico, con una breve pausa mentre si scrive.
  $effect(() => {
    if (!changed) return;
    const next = schedule;
    const timer = setTimeout(async () => {
      saving = true;
      try {
        onchanged(await api.serverSetSchedule(server.name, next));
      } catch (e) {
        app.error(e);
      } finally {
        saving = false;
      }
    }, 500);
    return () => clearTimeout(timer);
  });
</script>

<section class="card panel" aria-labelledby="auto-{server.name}">
  <header class="head">
    <h2 id="auto-{server.name}">{t('Automatismi')}</h2>
    <span class="muted small">{saving ? t('Salvataggio…') : t('Funzionano anche a launcher chiuso, se il server resta acceso.')}</span>
  </header>

  <div class="row">
    <div class="txt">
      <span class="strong">{t('Riavvio ogni giorno')}</span>
      <span class="muted small">{t('Avvisa in chat 5 minuti e 1 minuto prima, poi riavvia.')}</span>
    </div>
    <input class="input time" type="time" bind:value={restartAt} disabled={!restartOn} aria-label={t('Ora del riavvio')} />
    <button class="switch" role="switch" aria-checked={restartOn} aria-label={t('Riavvio ogni giorno')} onclick={() => (restartOn = !restartOn)}></button>
  </div>

  <div class="row">
    <div class="txt">
      <span class="strong">{t('Backup automatico')}</span>
      <span class="muted small">{t('Anche a server acceso, senza interrompere il gioco.')}</span>
    </div>
    <select class="input every" bind:value={backupHours} disabled={!backupOn} aria-label={t('Ogni quante ore')}>
      {#each HOURS as h (h)}<option value={h}>{h === 1 ? t('ogni ora') : t('ogni {n} ore', { n: h })}</option>{/each}
    </select>
    <button class="switch" role="switch" aria-checked={backupOn} aria-label={t('Backup automatico')} onclick={() => (backupOn = !backupOn)}></button>
  </div>

  <div class="row">
    <div class="txt">
      <span class="strong">{t('Backup da tenere')}</span>
      <span class="muted small">{t('I più vecchi si cancellano da soli (anche quelli fatti a mano).')}</span>
    </div>
    <input class="input keep" type="number" min="1" max="100" bind:value={keep} aria-label={t('Backup da tenere')} />
  </div>
</section>

<style>
  .panel { padding: 16px 18px; display: flex; flex-direction: column; gap: 14px; }
  .head { display: flex; flex-direction: column; gap: 2px; }
  h2 { margin: 0; font-size: 15px; font-weight: 600; }
  .row { display: flex; align-items: center; gap: 12px; }
  .txt { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .strong { font-weight: 600; }
  .time { width: 110px; font-family: var(--mono); }
  .every { width: 140px; }
  .keep { width: 80px; font-family: var(--mono); }
</style>
