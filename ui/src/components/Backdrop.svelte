<script lang="ts">
  // Immagine di sfondo personalizzata dietro tutta l'interfaccia, sfocata e
  // oscurata secondo le Impostazioni (variabili --bg-blur e --bg-dim).
  import { app } from '../lib/store.svelte';
  import { fade } from 'svelte/transition';
  import { ms } from '../lib/motion.svelte';

  const active = $derived(app.settings?.appearance.background === 'image');
</script>

<!-- Il fondo opaco c'è sempre in modalità immagine: anche mentre l'immagine
     carica, o se manca, la finestra non diventa trasparente. -->
{#if active}
  <div class="backdrop" aria-hidden="true">
    {#if app.backgroundImage}
      <div class="img" style:background-image="url({app.backgroundImage})" transition:fade={{ duration: ms(400) }}></div>
    {/if}
    <div class="dim"></div>
  </div>
{/if}

<style>
  :global(:root[data-css-zoom]) .backdrop { zoom: calc(1 / var(--ui-scale, 1)); }
  .backdrop { position: fixed; inset: 0; z-index: 0; overflow: hidden; background: var(--c-bg); pointer-events: none; }
  /* Bordi allargati: la sfocatura non lascia aloni chiari ai margini. */
  .img { position: absolute; inset: calc(var(--bg-blur) * -2); background-size: cover; background-position: center; filter: blur(var(--bg-blur)); transform: translateZ(0); }
  .dim { position: absolute; inset: 0; background: color-mix(in srgb, var(--c-bg) calc(var(--bg-dim) * 100%), transparent); }
</style>
