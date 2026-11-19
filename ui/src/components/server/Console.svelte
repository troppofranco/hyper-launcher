<script lang="ts">
  // Terminale del server: righe nuove ogni secondo mentre è acceso, comandi
  // con cronologia (↑/↓) e scorrimento automatico finché si è in fondo.
  import { api } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import { t } from '../../lib/i18n.svelte';
  import { parseLine, plain } from './console';
  import { copyText, poll } from './util';

  let { name, running }: { name: string; running: boolean } = $props();

  const MAX_LINES = 3000;
  const TRIM_TO = 2500;

  let lines = $state<{ id: number; text: string }[]>([]);
  let input = $state('');
  let sending = $state(false);
  let box = $state<HTMLDivElement>();
  /** Si segue il fondo finché l'utente non scorre in su. */
  let stick = true;
  // Prossima riga da chiedere al server (non reattiva: non deve far ripartire il polling).
  let next = 0;
  let nextId = 0;
  const history: string[] = [];
  let historyAt = 0;

  async function fetchLines() {
    const chunk = await api.serverConsole(name, next);
    // Numerazione ripartita (es. dopo un riavvio del backend): si rilegge da capo.
    if (chunk.next < next) {
      next = 0;
      lines = [];
      return fetchLines();
    }
    next = chunk.next;
    if (!chunk.lines.length) return;
    lines.push(...chunk.lines.map((text) => ({ id: nextId++, text })));
    // Si accorcia a blocchi, non una riga alla volta.
    if (lines.length > MAX_LINES) lines.splice(0, lines.length - TRIM_TO);
  }

  $effect(() => {
    if (running) return poll(fetchLines, 1000);
    // Spento: si mostra comunque l'ultimo log, una volta.
    fetchLines().catch(() => {});
  });

  // Dopo ogni aggiunta (il DOM è già aggiornato) si torna in fondo.
  $effect(() => {
    void lines.length;
    if (stick && box) box.scrollTop = box.scrollHeight;
  });

  function onscroll() {
    if (!box) return;
    stick = box.scrollHeight - box.scrollTop - box.clientHeight < 24;
  }

  async function send(e: SubmitEvent) {
    e.preventDefault();
    const line = input.trim();
    if (!line || !running) return;
    sending = true;
    try {
      await api.serverCommand(name, line);
      if (history.at(-1) !== line) history.push(line);
      historyAt = history.length;
      input = '';
      stick = true;
      await fetchLines();
    } catch (err) {
      app.error(err);
    } finally {
      sending = false;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key !== 'ArrowUp' && e.key !== 'ArrowDown') return;
    if (!history.length) return;
    e.preventDefault();
    historyAt = Math.max(0, Math.min(history.length, historyAt + (e.key === 'ArrowUp' ? -1 : 1)));
    input = history[historyAt] ?? '';
  }

  function copyAll() {
    void copyText(lines.map((l) => plain(l.text)).join('\n'), t('Console copiata.'));
  }

  function clear() {
    lines = [];
    stick = true;
  }
</script>

<section class="term" aria-label={t('Console del server')}>
  <div class="bar">
    <span class="title">{t('Console')}</span>
    <span class="tools">
      <button class="tool" disabled={!lines.length} onclick={copyAll}>{t('Copia tutto')}</button>
      <button class="tool" disabled={!lines.length} onclick={clear}>{t('Pulisci')}</button>
    </span>
  </div>
  <div class="out" bind:this={box} {onscroll} role="log" aria-live="off">
    {#each lines as line (line.id)}
      {@const p = parseLine(line.text)}
      <div class="line {p.level}">{#if p.prefix}<span class="prefix">{p.prefix}</span>{' '}{/if}{#each p.pieces as piece, i (i)}<span class={piece.color ? `c${piece.color}` : undefined} class:b={piece.bold}>{piece.text}</span>{/each}</div>
    {/each}
    {#if !running}
      <div class="off">{lines.length ? t('Il server è spento. Avvialo per vedere la console dal vivo.') : t('Il server è spento: la console si riempie quando lo avvii.')}</div>
    {/if}
  </div>
  <form class="cmd" onsubmit={send}>
    <span class="prompt" aria-hidden="true">&gt;</span>
    <input
      bind:value={input}
      {onkeydown}
      disabled={!running || sending}
      placeholder={running ? t('Scrivi un comando (es. say ciao, time set day) e premi Invio') : t('Avvia il server per inviare comandi')}
      aria-label={t('Comando della console')}
      autocomplete="off"
      spellcheck="false"
    />
  </form>
</section>

<style>
  .term { border-radius: var(--radius); background: var(--bg-deep); border: 1px solid var(--line); display: flex; flex-direction: column; height: 330px; overflow: hidden; }
  .bar { display: flex; justify-content: space-between; align-items: center; padding: 8px 10px 8px 16px; border-bottom: 1px solid var(--line); }
  .title { font-weight: 600; font-size: 13px; color: var(--text-2); }
  .tools { display: flex; gap: 4px; }
  .tool { height: 28px; padding: 0 10px; border-radius: 8px; border: 0; background: transparent; color: var(--muted); font-size: 12px; font-weight: 600; }
  .tool:hover:not(:disabled) { background: var(--surface-2); color: var(--text); }
  .out { flex: 1; min-height: 0; overflow-y: auto; padding: 12px 16px; font-family: var(--mono); font-size: 12.5px; line-height: 1.6; color: var(--text-2); white-space: pre-wrap; word-break: break-word; user-select: text; }
  .prefix { color: var(--faint); }
  .line.warn { color: var(--warn-text); }
  .line.error { color: var(--danger-text); }
  .b { font-weight: 700; }
  .off { color: var(--muted); font-family: var(--font); padding-top: 6px; }
  .cmd { display: flex; align-items: center; gap: 10px; padding: 6px 14px; border-top: 1px solid var(--line); }
  .prompt { font-family: var(--mono); color: var(--accent-line); font-weight: 600; }
  .cmd input { flex: 1; height: 34px; border: 0; background: transparent; font-family: var(--mono); font-size: 13px; outline: none; }
  .cmd input:disabled { opacity: .6; }
  .out::-webkit-scrollbar { width: 10px; }
  .out::-webkit-scrollbar-thumb { background: var(--line-strong); border-radius: 5px; border: 2px solid transparent; background-clip: padding-box; }

  /* Colori § di Minecraft; nel tema chiaro i più tenui si scuriscono. */
  .c0 { color: #000000; } .c1 { color: #3b3bd6; } .c2 { color: #1f9e1f; } .c3 { color: #1fa3a3; }
  .c4 { color: #c43030; } .c5 { color: #a23ea2; } .c6 { color: #e0a32e; } .c7 { color: #aaaaaa; }
  .c8 { color: #6b6b6b; } .c9 { color: #6f6fff; } .ca { color: #55ff55; } .cb { color: #55ffff; }
  .cc { color: #ff5555; } .cd { color: #ff66ff; } .ce { color: #ffff55; } .cf { color: #ffffff; }
  :global(:root:not([data-theme='light'])) .c0 { color: #555555; }
  :global(:root[data-theme='light']) .c6 { color: #9a6a00; }
  :global(:root[data-theme='light']) .c7 { color: #6b6b6b; }
  :global(:root[data-theme='light']) .ca { color: #178a17; }
  :global(:root[data-theme='light']) .cb { color: #0f8a8a; }
  :global(:root[data-theme='light']) .cc { color: #c62b30; }
  :global(:root[data-theme='light']) .cd { color: #b0329e; }
  :global(:root[data-theme='light']) .ce { color: #8a7a00; }
  :global(:root[data-theme='light']) .cf { color: #15171c; }
</style>
