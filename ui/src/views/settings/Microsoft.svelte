<script lang="ts">
  import { t } from '../../lib/i18n.svelte';
  import { settings } from './draft.svelte';

  const d = $derived(settings.draft!);
</script>

<p class="muted enter" style:--i={0}>{t('Per il login Microsoft serve il client ID di un’app Azure ("Personal Microsoft accounts only", "Allow public client flows" = Yes) approvata da Mojang per la Minecraft API. I token restano solo nel portachiavi di Windows.')}</p>
<label class="field enter" style:--i={1}>{t('Client ID Azure')}
  <input class="input mono" bind:value={d.msaClientId} placeholder="00000000-0000-0000-0000-000000000000" spellcheck="false" />
</label>

<style>
  p { margin: 0; line-height: 1.5; }
  .mono { font-family: var(--mono); }
</style>
