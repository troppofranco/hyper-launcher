<script lang="ts">
  import { t } from '../../lib/i18n.svelte';
  import { app } from '../../lib/store.svelte';
  import { settings } from './draft.svelte';
  import Icon from '../../components/Icon.svelte';
  import Toggle from '../../components/controls/Toggle.svelte';
  import Range from '../../components/controls/Range.svelte';
  import Segmented from '../../components/controls/Segmented.svelte';

  const a = $derived(settings.draft!.appearance);
  const SIDES: ['left' | 'right', string][] = [
    ['left', 'A sinistra'],
    ['right', 'A destra'],
  ];

  function customizeHome() {
    app.homeEditing = true;
    app.go({ view: 'home' });
  }
</script>

<section class="group enter" style:--i={0}>
  <span class="strong">{t('Barra laterale')}</span>
  <div class="choices">
    {#each SIDES as [id, label] (id)}
      <button class="choice" class:on={a.sidebarSide === id} aria-pressed={a.sidebarSide === id} onclick={() => (a.sidebarSide = id)}>
        <span class="frame" class:right={id === 'right'}><span class="bar"></span></span>{t(label)}
      </button>
    {/each}
  </div>
</section>
<div class="enter" style:--i={1}><Toggle bind:checked={a.sidebarLabels} label="Etichette accanto alle icone" hint="Barra più larga, con il nome di ogni sezione e istanza." /></div>
<div class="enter" style:--i={2}><Range bind:value={a.sidebarInstances} label="Istanze nella barra laterale" min={0} max={10} /></div>
<div class="enter" style:--i={3}><Toggle bind:checked={a.showNews} label="Pannello notizie" hint="La colonna di notizie Minecraft accanto a Home, Libreria e istanze." /></div>
<div class="grid enter" style:--i={4}>
  <Range bind:value={a.uiScale} label="Scala dell’interfaccia" min={80} max={130} step={5} unit="%" />
  <Segmented bind:value={a.radius} label="Angoli" options={[['sharp', 'Netti'], ['medium', 'Medi'], ['round', 'Tondi']]} />
</div>
<div class="enter" style:--i={5}><Toggle bind:checked={a.wideContent} label="Contenuti a tutta larghezza" hint="Su schermi grandi le pagine usano tutto lo spazio invece di restare centrate." /></div>
<div class="box enter" style:--i={6}>
  <div><div class="strong">{t('Home')}</div><div class="muted small">{t('Sposta, ridimensiona, nascondi o aggiungi i widget della Home.')}</div></div>
  <button class="btn primary" onclick={customizeHome}><Icon name="home" size={16} /> {t('Personalizza la Home')}</button>
</div>

<style>
  .group { display: flex; flex-direction: column; gap: 10px; }
  .strong { font-weight: 600; }
  .choices { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
  .choice { display: flex; align-items: center; gap: 12px; height: 64px; padding: 0 14px; border-radius: var(--radius-sm); border: 1px solid var(--line-strong); background: var(--panel); font-weight: 600; transition: border-color .18s ease, background .18s ease, transform .15s ease; }
  .choice:hover { border-color: var(--accent-line); }
  .choice:active { transform: scale(.98); }
  .choice.on { border-color: var(--accent-line); background: var(--accent-soft); box-shadow: 0 0 0 1px var(--accent-line) inset; }
  .frame { width: 56px; height: 38px; border-radius: 6px; border: 1px solid var(--line-strong); display: flex; overflow: hidden; }
  .frame.right { justify-content: flex-end; }
  .bar { width: 12px; background: var(--muted); opacity: .6; transition: background .2s ease; }
  .choice.on .bar { background: var(--accent); opacity: 1; }
  .grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 18px; align-items: end; }
  .box { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 14px; border-radius: var(--radius-sm); background: var(--panel); flex-wrap: wrap; }
</style>
