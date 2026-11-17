<script lang="ts">
  // Avanzamento di un download di contenuti ("File 2 di 5"); senza dati
  // mostra una barra quasi vuota con "Preparazione…".
  let { progress, label = true }: { progress: { done: number; total: number } | undefined; label?: boolean } = $props();
  const percent = $derived(progress && progress.total ? (progress.done / progress.total) * 100 : 8);
</script>

<span class="progress">
  {#if label}<span class="small muted">{progress && progress.total ? `File ${progress.done} di ${progress.total}` : 'Preparazione…'}</span>{/if}
  <span class="bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(percent)}><span style:width="{percent}%"></span></span>
</span>

<style>
  .progress { width: 100%; min-width: 120px; display: flex; flex-direction: column; gap: 6px; }
  .bar { display: block; height: 6px; border-radius: 3px; background: var(--surface-2); overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--accent); transition: width .2s; }
</style>
