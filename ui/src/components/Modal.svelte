<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  let { title, onclose, children, width = 560 }: { title: string; onclose: () => void; children: Snippet; width?: number } = $props();
  const id = `modal-${Math.random().toString(36).slice(2)}`;
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />
<div class="overlay">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby={id} style:width="{width}px">
    <div class="head">
      <h2 {id}>{title}</h2>
      <button class="x" aria-label="Chiudi" onclick={onclose}><Icon name="close" size={18} /></button>
    </div>
    {@render children()}
  </div>
</div>

<style>
  .overlay { position: fixed; inset: 0; z-index: 30; background: rgba(5, 6, 8, .72); display: flex; align-items: center; justify-content: center; padding: 24px; }
  .dialog { max-width: 100%; max-height: 100%; overflow-y: auto; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 16px; padding: 24px; display: flex; flex-direction: column; gap: 18px; box-shadow: 0 24px 60px rgba(0,0,0,.5); }
  .head { display: flex; justify-content: space-between; align-items: center; }
  h2 { font-size: 18px; font-weight: 600; }
  .x { width: 36px; height: 36px; border-radius: 8px; border: 0; background: transparent; color: var(--muted); display: flex; align-items: center; justify-content: center; }
  .x:hover { background: var(--surface-2); color: var(--text); }
</style>
