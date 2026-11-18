<script lang="ts">
  import { t } from '../../lib/i18n.svelte';
  let { checked = $bindable(), label, hint }: { checked: boolean; label: string; hint?: string } = $props();
</script>

<div class="row">
  <div class="txt"><span class="strong">{t(label)}</span>{#if hint}<span class="muted small">{t(hint)}</span>{/if}</div>
  <button class="switch" role="switch" aria-checked={checked} aria-label={t(label)} onclick={() => (checked = !checked)}></button>
</div>

<style>
  .row { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 4px 0; }
  .txt { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .strong { font-weight: 600; }
</style>
