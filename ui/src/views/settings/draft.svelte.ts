// Bozza delle impostazioni condivisa dalle sezioni: ogni modifica si vede
// subito (applySettings) e viene salvata da sola poco dopo.
import type { Settings } from '../../lib/api';
import { app } from '../../lib/store.svelte';

export const settings = $state<{ draft: Settings | null; status: 'idle' | 'saving' | 'saved' | 'error' }>({
  draft: null,
  status: 'idle',
});

let timer: ReturnType<typeof setTimeout> | undefined;
let saved = '';

/** Prende le impostazioni attuali come punto di partenza. */
export function startEditing() {
  if (!app.settings) return;
  settings.draft = $state.snapshot(app.settings);
  saved = JSON.stringify(settings.draft);
  settings.status = 'idle';
}

/** Da chiamare quando la bozza cambia: applica subito e salva dopo 400 ms. */
export function changed() {
  const draft = settings.draft;
  if (!draft) return;
  const json = JSON.stringify(draft);
  if (json === saved) return;
  app.applySettings($state.snapshot(draft));
  clearTimeout(timer);
  timer = setTimeout(async () => {
    settings.status = 'saving';
    try {
      await app.saveSettings(JSON.parse(json));
      saved = json;
      settings.status = 'saved';
    } catch (e) {
      // Valori rifiutati: si torna agli ultimi salvati, così ciò che si
      // vede è ciò che è davvero sul disco.
      settings.status = 'error';
      app.error(e);
      const last = JSON.parse(saved);
      app.applySettings(last);
      settings.draft = last;
    }
  }, 400);
}
