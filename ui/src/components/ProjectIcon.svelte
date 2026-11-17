<script lang="ts">
  import { initial, instanceColors } from '../lib/format';

  // Icona di un progetto Modrinth; senza immagine (o se non carica) mostra
  // l'iniziale su un colore stabile ricavato dal nome.
  let { url, name, size = 48 }: { url: string | null; name: string; size?: number } = $props();
  let broken = $state(false);
  const colors = $derived(instanceColors(name));
</script>

{#if url && !broken}
  <img src={url} alt="" width={size} height={size} style:border-radius="{Math.round(size * 0.22)}px" loading="lazy" onerror={() => (broken = true)} />
{:else}
  <span class="fallback" style:width="{size}px" style:height="{size}px" style:border-radius="{Math.round(size * 0.22)}px" style:background={colors.bg} style:color={colors.fg} style:font-size="{Math.round(size * 0.4)}px">{initial(name)}</span>
{/if}

<style>
  img { flex: none; object-fit: cover; background: var(--surface-2); }
  .fallback { flex: none; display: flex; align-items: center; justify-content: center; font-family: var(--display); font-weight: 600; }
</style>
