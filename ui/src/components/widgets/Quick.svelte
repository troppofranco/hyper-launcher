<script lang="ts">
  import { t } from '../../lib/i18n.svelte';
  import { app } from '../../lib/store.svelte';
  import { loaderLabel } from '../../lib/format';
  import Icon from '../Icon.svelte';
  import InstanceIcon from '../InstanceIcon.svelte';

  let { count = 8 }: { count?: number } = $props();
  const quick = $derived(app.recent.slice(0, count));
</script>

<section class="card quick" aria-label={t('Gioca subito')}>
  <span class="quick-label"><Icon name="play" size={16} /> {t('Gioca subito')}</span>
  <span class="quick-sep"></span>
  <div class="quick-icons">
    {#each quick as inst (inst.name)}
      <button
        class="quick-btn"
        class:running={!!app.launches[inst.name]}
        title={t('Gioca a {nome} · {loader} {versione}', { nome: inst.name, loader: loaderLabel(inst), versione: inst.version })}
        aria-label={t('Gioca a {nome}', { nome: inst.name })}
        disabled={!!app.launches[inst.name]}
        onclick={() => app.play(inst.name)}
      >
        <InstanceIcon name={inst.name} icon={inst.icon} size={40} />
        <span class="play-badge"><Icon name="play" size={12} /></span>
      </button>
    {/each}
    <button class="quick-btn add" aria-label={t('Nuova istanza')} title={t('Nuova istanza')} onclick={() => (app.newInstanceOpen = true)}><Icon name="plus" size={18} /></button>
  </div>
</section>

<style>
  .quick { display: flex; align-items: center; align-content: center; gap: 14px; padding: 12px 16px; flex-wrap: wrap; }
  .quick-label { display: flex; align-items: center; gap: 8px; color: var(--muted); font-weight: 600; }
  .quick-sep { width: 1px; height: 30px; background: var(--line-strong); }
  .quick-icons { display: flex; gap: 10px; flex-wrap: wrap; }
  .quick-btn { position: relative; padding: 0; border: 0; background: none; border-radius: 12px; transition: transform .18s var(--ease); }
  .quick-btn:hover:not(:disabled) { transform: translateY(-3px) scale(1.06); }
  .quick-btn:active:not(:disabled) { transform: scale(.94); }
  .play-badge { position: absolute; right: -4px; bottom: -4px; width: 20px; height: 20px; border-radius: 10px; background: var(--accent); color: #fff; display: flex; align-items: center; justify-content: center; opacity: 0; transform: scale(.6); transition: opacity .18s, transform .18s; border: 2px solid var(--c-surface); }
  .quick-btn:hover .play-badge { opacity: 1; transform: scale(1); }
  .quick-btn.running::after { content: ''; position: absolute; inset: -3px; border-radius: 14px; border: 2px solid var(--success); animation: ring 1.4s ease-in-out infinite; }
  .quick-btn.add { width: 40px; height: 40px; border: 1px dashed var(--line-strong); color: var(--muted); display: flex; align-items: center; justify-content: center; }
  .quick-btn.add:hover { color: var(--text); border-color: var(--accent-line); }
  @keyframes ring { 0%, 100% { opacity: .4; } 50% { opacity: 1; } }
</style>
