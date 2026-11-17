<script lang="ts">
  import { app } from '../lib/store.svelte';
  import { megabytes } from '../lib/format';
  import Icon from './Icon.svelte';

  let { name, size = 'normal' }: { name: string; size?: 'normal' | 'large' } = $props();

  const launch = $derived(app.launches[name]);
  const stageLabel: Record<string, string> = { libraries: 'Librerie', assets: 'Asset', java: 'Java' };
  const percent = $derived(launch && launch.total > 0 ? Math.round((launch.done / launch.total) * 100) : 0);
</script>

{#if !launch}
  <button class="btn primary play {size}" onclick={() => app.play(name)}><Icon name="play" size={16} /> Gioca</button>
{:else if launch.phase === 'running'}
  <span class="running {size}" role="status"><span class="dot"></span> In gioco</span>
{:else}
  <div class="progress {size}" role="status" aria-live="polite">
    <div class="line">
      <span>{launch.total ? `${stageLabel[launch.stage] ?? 'Preparazione'} · ${launch.done}/${launch.total}` : 'Preparazione…'}</span>
      <span>{megabytes(launch.downloadedBytes)}</span>
    </div>
    <div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={percent}><div style:width="{percent}%"></div></div>
  </div>
{/if}

<style>
  .play.large { height: 52px; min-width: 180px; font-size: 16px; border-radius: 12px; }
  .running { display: inline-flex; align-items: center; gap: 8px; height: 44px; padding: 0 16px; border-radius: 10px; background: color-mix(in srgb, #3ecf8e 16%, transparent); color: #7ee2c0; font-weight: 600; }
  .running.large { height: 52px; min-width: 180px; justify-content: center; border-radius: 12px; }
  .dot { width: 8px; height: 8px; border-radius: 4px; background: #3ecf8e; }
  .progress { min-width: 220px; display: flex; flex-direction: column; gap: 8px; }
  .progress.large { min-width: 280px; }
  .line { display: flex; justify-content: space-between; gap: 12px; font-size: 12px; color: var(--text-2); }
  .bar { height: 8px; border-radius: 4px; background: #262a34; overflow: hidden; }
  .bar div { height: 100%; background: var(--accent); transition: width .2s; }
</style>
