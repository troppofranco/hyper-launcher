<script lang="ts">
  import { initial, instanceColors } from '../lib/format';

  let { name, size = 48 }: { name: string; size?: number } = $props();
  const colors = $derived(instanceColors(name));
</script>

<div
  class="icon"
  style:width="{size}px"
  style:height="{size}px"
  style:font-size="{Math.round(size * 0.42)}px"
  style:border-radius="{Math.round(size * 0.27)}px"
  style:background={colors.bg}
  style:color={colors.fg}
  aria-hidden="true"
>
  {initial(name)}
</div>

<style>
  .icon { flex: 0 0 auto; display: flex; align-items: center; justify-content: center; font-weight: 700; font-family: var(--display); }
</style>
