<script lang="ts">
  // Immagine del profilo: la testa della skin scelta, altrimenti l'iniziale.
  import type { Account } from '../lib/api';
  import { initial } from '../lib/format';
  import { skinHead } from '../lib/image';

  let { account, size = 40 }: { account: Account; size?: number } = $props();

  let head = $state<string | null>(null);
  $effect(() => {
    const skin = account.skin;
    head = null;
    if (!skin) return;
    let live = true;
    skinHead(skin, 64).then((h) => live && (head = h)).catch(() => {});
    return () => (live = false);
  });
</script>

{#if head}
  <img class="head" src={head} alt="" width={size} height={size} />
{:else}
  <span class="letter" style:width="{size}px" style:height="{size}px">{initial(account.name)}</span>
{/if}

<style>
  .head { display: block; border-radius: 10px; image-rendering: pixelated; animation: pop .3s var(--ease) backwards; }
  .letter { display: flex; align-items: center; justify-content: center; border-radius: 10px; background: var(--accent-soft); font-weight: 700; }
  @keyframes pop { from { opacity: 0; transform: scale(.8); } to { opacity: 1; transform: none; } }
</style>
