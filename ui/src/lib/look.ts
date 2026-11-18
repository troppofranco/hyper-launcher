import { inTauri, type Appearance } from './api';
import { setMotion } from './motion.svelte';

// Scelte d'aspetto condivise da Impostazioni e configurazione iniziale.

export const ACCENTS: [string, string][] = [
  ['#6e5ae6', 'Viola'],
  ['#2f6fe4', 'Blu'],
  ['#12856f', 'Verde acqua'],
  ['#c2571a', 'Arancio'],
  ['#c0306a', 'Lampone'],
];

export const THEMES: ['dark' | 'light', string][] = [
  ['dark', 'Scuro'],
  ['light', 'Chiaro'],
];


/**
 * Applica aspetto e layout alla pagina: attributi e variabili su `:root`
 * letti da app.css (sfondo, vetro, angoli, scala, animazioni, larghezza).
 */
export function applyAppearance(a: Appearance, windowEffect: boolean) {
  const root = document.documentElement;
  // Senza effetto della finestra la trasparenza mostrerebbe il desktop nitido.
  root.dataset.background = a.background === 'transparent' && !windowEffect ? 'solid' : a.background;
  root.dataset.radius = a.radius;
  root.dataset.buttons = a.buttons;
  // Nell'app la scala è lo zoom nativo della WebView (applicato dal backend);
  // nel browser di sviluppo si simula con lo zoom CSS.
  root.toggleAttribute('data-css-zoom', !inTauri);
  root.dataset.animations = a.animations;
  root.toggleAttribute('data-wide', a.wideContent);
  root.style.setProperty('--alpha', String(a.panelOpacity / 100));
  root.style.setProperty('--ui-scale', String(a.uiScale / 100));
  root.style.setProperty('--bg-blur', `${a.blur}px`);
  root.style.setProperty('--bg-dim', String(a.dim / 100));
  setMotion(a.animations);
}
