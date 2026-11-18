// Widget della Home: cosa esiste, le larghezze ammesse, le opzioni e la
// disposizione predefinita. I componenti sono in components/widgets.
import type { WidgetConfig, WidgetSize } from './api';

export interface WidgetOption { key: string; label: string; min: number; max: number; default: number }
export interface WidgetDef {
  kind: string;
  /** Titolo mostrato sopra il widget (null = nessun titolo, es. il saluto). */
  title: string | null;
  /** Nome nel catalogo e nel menu. */
  name: string;
  description: string;
  size: WidgetSize;
  options: WidgetOption[];
}

export const WIDGETS: WidgetDef[] = [
  { kind: 'greeting', title: null, name: 'Saluto', description: 'Il benvenuto con il tuo nome e la Libreria a portata di mano.', size: 'large', options: [] },
  { kind: 'quick', title: null, name: 'Gioca subito', description: 'Le istanze più recenti: un clic e si parte.', size: 'large', options: [{ key: 'count', label: 'Istanze mostrate', min: 1, max: 12, default: 8 }] },
  { kind: 'catalog', title: 'Scopri nel catalogo', name: 'Scopri nel catalogo', description: 'I modpack più scaricati su Modrinth.', size: 'large', options: [{ key: 'count', label: 'Modpack mostrati', min: 1, max: 6, default: 3 }] },
  { kind: 'servers', title: 'I tuoi server', name: 'I tuoi server', description: 'I server salvati, con giocatori online e latenza.', size: 'medium', options: [{ key: 'count', label: 'Server mostrati', min: 1, max: 10, default: 6 }] },
  { kind: 'playtime', title: 'Il tuo tempo di gioco', name: 'Tempo di gioco', description: 'Le ore giocate negli ultimi sette giorni.', size: 'medium', options: [] },
  { kind: 'news', title: 'Notizie', name: 'Notizie', description: 'Le ultime notizie di Minecraft.', size: 'large', options: [{ key: 'count', label: 'Notizie mostrate', min: 1, max: 4, default: 1 }] },
  { kind: 'last', title: 'Ultima partita', name: 'Ultima istanza giocata', description: 'Riprendi da dove avevi lasciato.', size: 'small', options: [] },
  { kind: 'skin', title: 'La tua skin', name: 'Skin attuale', description: 'Anteprima 3D della skin scelta.', size: 'small', options: [] },
  { kind: 'stats', title: 'Statistiche', name: 'Statistiche', description: 'Istanze, ore totali e l’istanza più giocata.', size: 'small', options: [] },
];

export const SIZES: [WidgetSize, string][] = [
  ['small', 'Piccolo'],
  ['medium', 'Medio'],
  ['large', 'Grande'],
];

export function widgetDef(kind: string): WidgetDef | undefined {
  return WIDGETS.find((w) => w.kind === kind);
}

const make = (kind: string, size?: WidgetSize): WidgetConfig => {
  const def = widgetDef(kind)!;
  return { kind, size: size ?? def.size, options: Object.fromEntries(def.options.map((o) => [o.key, o.default])) };
};

/** La Home com'era prima dei widget, più le statistiche. */
export function defaultLayout(): WidgetConfig[] {
  return [make('greeting'), make('quick'), make('catalog'), make('servers'), make('playtime'), make('news')];
}

/** Un widget nuovo con le impostazioni predefinite. */
export function newWidget(kind: string): WidgetConfig {
  return make(kind);
}

/** Valore di un'opzione (con il predefinito se manca). */
export function option(w: WidgetConfig, key: string): number {
  return w.options[key] ?? widgetDef(w.kind)?.options.find((o) => o.key === key)?.default ?? 0;
}
