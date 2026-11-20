<script lang="ts" generics="T extends string">
  import { t } from '../../lib/i18n.svelte';
  let { value = $bindable(), options, label }: { value: T; options: [T, string][]; label: string } = $props();
</script>

<div class="field">
  <span class="strong">{t(label)}</span>
  <div class="seg" role="radiogroup" aria-label={t(label)}>
    {#each options as [id, text] (id)}
      <button type="button" role="radio" aria-checked={value === id} class:on={value === id} onclick={() => (value = id)}>{t(text)}</button>
    {/each}
  </div>
</div>

<style>
  .field { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  .strong { font-weight: 600; }
  .seg { display: flex; border: 1px solid var(--line-strong); border-radius: var(--radius-sm); overflow: hidden; padding: 3px; gap: 3px; background: var(--panel); }
  button { flex: 1; height: 34px; border: 0; border-radius: calc(var(--radius-sm) - 3px); background: transparent; color: var(--text-2); font-weight: 600; font-size: 13px; transition: background .2s ease, color .2s ease; white-space: nowrap; }
  button:hover { color: var(--text); }
  button.on { background: var(--accent); color: #fff; }
</style>
