<script lang="ts">
  import { api, type SearchHit } from '../lib/api';
  import { t } from '../lib/i18n.svelte';
  import { app } from '../lib/store.svelte';
  import { compact, isoDate, loaderLabel } from '../lib/format';
  import Icon from '../components/Icon.svelte';
  import InstanceIcon from '../components/InstanceIcon.svelte';
  import ProjectIcon from '../components/ProjectIcon.svelte';
  import PlaytimeWeek from '../components/PlaytimeWeek.svelte';
  import ServerList from '../components/ServerList.svelte';

  const GREETINGS = ['Pronto a costruire qualcosa di grande?', 'Il mondo ti aspetta.', 'Si gioca?', 'Un altro blocco, un’altra avventura.'];
  const greetingIndex = new Date().getDate() % GREETINGS.length;

  const quick = $derived(app.recent.slice(0, 8));
  const featuredNews = $derived(app.news[0] ?? null);
  let featured = $state<SearchHit[] | null>(null);

  // I modpack più scaricati: una volta per sessione, poi dalla memoria.
  $effect(() => {
    if (app.featuredPacks) {
      featured = app.featuredPacks;
      return;
    }
    api.search({ text: '', kind: 'modpack', instance: null, gameVersion: null, loaders: [], categories: [], sort: 'downloads', offset: 0 })
      .then((r) => (featured = app.featuredPacks = r.hits.slice(0, 3)))
      .catch(() => (featured = []));
  });

  function cover(hit: SearchHit): string {
    const hex = `#${(hit.color ?? 0x2a2f3a).toString(16).padStart(6, '0')}`;
    return `linear-gradient(135deg, color-mix(in srgb, ${hex} 55%, var(--panel)), var(--panel))`;
  }

  function openPack(hit: SearchHit) {
    app.explore.kind = 'modpack';
    app.openProject(hit.id, 'modpack');
  }
</script>

<div class="page home">
  <header class="head enter">
    <div>
      <h1 class="page-title">{app.activeAccount ? t('Bentornato, {nome}', { nome: app.activeAccount.name }) : t('Benvenuto in Hyper Launcher')}</h1>
      <p class="muted">{app.instances.length ? t(GREETINGS[greetingIndex]) : t('Crea la tua prima istanza per iniziare a giocare.')}</p>
    </div>
    <button class="card lib-btn" onclick={() => app.go({ view: 'library' })}>
      <Icon name="library" size={20} />
      <span><span class="strong">{t('Libreria')}</span><span class="muted small">{t('Tutte le tue istanze')}</span></span>
    </button>
  </header>

  <section class="card quick enter" style:--i={1} aria-label={t('Gioca subito')}>
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

  <section aria-labelledby="hub-title" class="enter" style:--i={2}>
    <div class="section-head">
      <h2 id="hub-title" class="section-title">{t('Scopri nel catalogo')}</h2>
      <button class="link" onclick={() => app.browse(null, 'modpack')}>{t('Vedi tutti')} <Icon name="back" size={14} /></button>
    </div>
    <div class="packs">
      {#if featured === null}
        {#each [0, 1, 2] as n (n)}<div class="card pack skeleton"></div>{/each}
      {:else}
        {#each featured as hit, i (hit.id)}
          <button class="card pack enter" style:--i={i + 3} onclick={() => openPack(hit)}>
            <span class="pack-cover" style:background={cover(hit)}>
              {#if hit.image}<img src={hit.image} alt="" loading="lazy" />{/if}
              <span class="pack-icon"><ProjectIcon url={hit.iconUrl} name={hit.title} size={46} /></span>
            </span>
            <span class="pack-body">
              <span class="pack-title">{hit.title}</span>
              <span class="pack-desc">{hit.description}</span>
              <span class="pack-foot"><span>{t('di {autore}', { autore: hit.author })}</span><span><Icon name="download" size={12} /> {compact(hit.downloads)}</span></span>
            </span>
          </button>
        {/each}
      {/if}
    </div>
  </section>

  <div class="two enter" style:--i={4}>
    <ServerList />
    <section aria-labelledby="week-title-home">
      <h2 id="week-title-home" class="section-title">{t('Il tuo tempo di gioco')}</h2>
      <PlaytimeWeek showTitle={false} />
    </section>
  </div>

  {#if featuredNews}
    <section aria-labelledby="news-title" class="enter" style:--i={5}>
      <h2 id="news-title" class="section-title">{t('Notizie')}</h2>
      <button class="card news" onclick={() => api.openUrl(featuredNews.url)}>
        <span class="news-img">{#if featuredNews.image}<img src={featuredNews.image} alt="" />{/if}</span>
        <span class="news-body">
          <span class="news-meta"><span class="tag">{t('NOTIZIA')}</span><span class="muted small">{isoDate(featuredNews.date)}</span></span>
          <span class="news-title">{featuredNews.title}</span>
          {#if featuredNews.text}<span class="muted">{featuredNews.text}</span>{/if}
          <span class="read">{t('Leggi di più')} <Icon name="external" size={14} /></span>
        </span>
      </button>
    </section>
  {/if}
</div>

<style>
  .home { gap: 26px; }
  .head { display: flex; justify-content: space-between; align-items: center; gap: 16px; flex-wrap: wrap; }
  .head p { margin: 6px 0 0; }
  .lib-btn { display: flex; align-items: center; gap: 12px; padding: 12px 18px; text-align: left; color: var(--text); }
  .lib-btn > span { display: flex; flex-direction: column; }
  .strong { font-weight: 600; }
  .quick { display: flex; align-items: center; gap: 14px; padding: 12px 16px; flex-wrap: wrap; }
  .quick-label { display: flex; align-items: center; gap: 8px; color: var(--muted); font-weight: 600; }
  .quick-sep { width: 1px; height: 30px; background: var(--line-strong); }
  .quick-icons { display: flex; gap: 10px; flex-wrap: wrap; }
  .quick-btn { position: relative; padding: 0; border: 0; background: none; border-radius: 12px; transition: transform .18s cubic-bezier(.2,.8,.2,1); }
  .quick-btn:hover:not(:disabled) { transform: translateY(-3px) scale(1.06); }
  .quick-btn:active:not(:disabled) { transform: scale(.94); }
  .play-badge { position: absolute; right: -4px; bottom: -4px; width: 20px; height: 20px; border-radius: 10px; background: var(--accent); color: #fff; display: flex; align-items: center; justify-content: center; opacity: 0; transform: scale(.6); transition: opacity .18s, transform .18s; border: 2px solid var(--surface); }
  .quick-btn:hover .play-badge { opacity: 1; transform: scale(1); }
  .quick-btn.running::after { content: ''; position: absolute; inset: -3px; border-radius: 14px; border: 2px solid var(--success); animation: ring 1.4s ease-in-out infinite; }
  .quick-btn.add { width: 40px; height: 40px; border: 1px dashed var(--line-strong); color: var(--muted); display: flex; align-items: center; justify-content: center; }
  .quick-btn.add:hover { color: var(--text); border-color: var(--accent-line); }
  .section-head { display: flex; justify-content: space-between; align-items: baseline; }
  .section-title { font-size: 16px; font-weight: 600; margin-bottom: 12px; }
  .link { display: inline-flex; align-items: center; gap: 4px; background: none; border: 0; color: var(--accent-line); font-size: 13px; padding: 0; }
  .link :global(svg) { transform: rotate(180deg); }
  .packs { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 14px; }
  .pack { padding: 0; overflow: hidden; display: flex; flex-direction: column; text-align: left; color: var(--text); }
  .pack.skeleton { height: 230px; }
  .pack-cover { position: relative; height: 120px; display: block; }
  .pack-cover img { width: 100%; height: 100%; object-fit: cover; display: block; transition: transform .5s cubic-bezier(.2,.8,.2,1); }
  .pack:hover .pack-cover img { transform: scale(1.06); }
  .pack-icon { position: absolute; left: 12px; bottom: -20px; border-radius: 13px; padding: 3px; background: var(--surface); display: flex; }
  .pack-body { padding: 26px 14px 14px; display: flex; flex-direction: column; gap: 6px; }
  .pack-title { font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .pack-desc { color: var(--muted); font-size: 13px; line-height: 1.45; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .pack-foot { display: flex; justify-content: space-between; font-size: 12px; color: var(--faint); margin-top: 4px; }
  .pack-foot span { display: inline-flex; align-items: center; gap: 4px; }
  .two { display: grid; grid-template-columns: repeat(auto-fit, minmax(320px, 1fr)); gap: 18px; align-items: start; }
  .news { padding: 0; display: grid; grid-template-columns: minmax(0, 320px) 1fr; overflow: hidden; text-align: left; color: var(--text); width: 100%; }
  .news-img { background: var(--bg-deep); min-height: 170px; display: block; overflow: hidden; }
  .news-img img { width: 100%; height: 100%; object-fit: cover; display: block; transition: transform .5s cubic-bezier(.2,.8,.2,1); }
  .news:hover .news-img img { transform: scale(1.05); }
  .news-body { padding: 22px 24px; display: flex; flex-direction: column; gap: 10px; justify-content: center; }
  .news-meta { display: flex; align-items: center; gap: 10px; }
  .tag { font-size: 10px; font-weight: 700; letter-spacing: .06em; padding: 4px 9px; border-radius: 12px; background: var(--accent-soft); color: var(--accent-line); }
  .news-title { font-family: var(--display); font-size: 22px; font-weight: 600; }
  .read { display: inline-flex; align-items: center; gap: 6px; font-weight: 600; margin-top: 4px; }
  @keyframes ring { 0%, 100% { opacity: .4; } 50% { opacity: 1; } }
  @media (max-width: 900px) {
    .packs { grid-template-columns: 1fr; }
    .news { grid-template-columns: 1fr; }
  }
</style>
