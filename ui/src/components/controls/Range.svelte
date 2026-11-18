<script lang="ts">
  import { t } from '../../lib/i18n.svelte';
  let { value = $bindable(), label, min, max, step = 1, unit = '' }: { value: number; label: string; min: number; max: number; step?: number; unit?: string } = $props();
</script>

<label class="field">
  <span class="head"><span class="strong">{t(label)}</span><span class="val">{value}{unit}</span></span>
  <input type="range" {min} {max} {step} bind:value />
</label>

<style>
  .field { display: flex; flex-direction: column; gap: 6px; }
  .head { display: flex; justify-content: space-between; align-items: baseline; }
  .strong { font-weight: 600; }
  .val { font-family: var(--mono); font-size: 12px; color: var(--muted); font-variant-numeric: tabular-nums; }
  input { width: 100%; }
</style>
