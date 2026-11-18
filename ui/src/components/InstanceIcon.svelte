<script lang="ts">
  import { initial, instanceColors } from '../lib/format';

  // Immagine scelta dall'utente oppure iniziale su un colore stabile.
  let { name, icon = null, size = 48 }: { name: string; icon?: string | null; size?: number } = $props();
  const colors = $derived(instanceColors(name));
  const radius = $derived(Math.round(size * 0.27));
</script>

{#if icon}
  <img class="icon" src={icon} alt="" width={size} height={size} style:border-radius="{radius}px" />
{:else}
  <div
    class="icon"
    style:width="{size}px"
    style:height="{size}px"
    style:font-size="{Math.round(size * 0.42)}px"
    style:border-radius="{radius}px"
    style:background={colors.bg}
    style:color={colors.fg}
    aria-hidden="true"
  >
    {initial(name)}
  </div>
{/if}

<style>
  .icon { flex: 0 0 auto; display: flex; align-items: center; justify-content: center; font-weight: 700; font-family: var(--display); object-fit: cover; }
</style>
