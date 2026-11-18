<script lang="ts">
  import { app } from '../lib/store.svelte';
  import Icon from './Icon.svelte';
  import { fly } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { prefersReducedMotion } from 'svelte/motion';

  // Le notifiche informative spariscono da sole; gli errori restano finché
  // non vengono chiusi, così si possono leggere (e copiare) con calma.
  $effect(() => {
    if (app.toast?.kind !== 'info') return;
    const t = setTimeout(() => (app.toast = null), 4000);
    return () => clearTimeout(t);
  });
</script>

{#if app.toast}
  <div class="toast" transition:fly={{ y: 24, duration: prefersReducedMotion.current ? 0 : 260, easing: cubicOut }} class:error={app.toast.kind === 'error'} role={app.toast.kind === 'error' ? 'alert' : 'status'}>
    <span class="text">{app.toast.text}</span>
    <button aria-label="Chiudi" onclick={() => (app.toast = null)}><Icon name="close" size={16} /></button>
  </div>
{/if}

<style>
  .toast { position: fixed; right: 20px; bottom: 20px; z-index: 40; max-width: 460px; display: flex; gap: 12px; align-items: flex-start; padding: 14px 14px 14px 16px; border-radius: 12px; background: var(--surface-2); border: 1px solid var(--line-strong); box-shadow: 0 16px 40px rgba(0,0,0,.45); }
  .toast.error { border-color: color-mix(in srgb, var(--danger) 60%, transparent); }
  .text { flex: 1; line-height: 1.45; user-select: text; white-space: pre-wrap; word-break: break-word; }
  button { border: 0; background: transparent; color: var(--muted); padding: 2px; border-radius: 6px; }
  button:hover { color: var(--text); }
</style>
