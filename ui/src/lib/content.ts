// Etichette e regole condivise dalle schermate del catalogo.

import type { ContentDirName, ContentKind, Instance } from './api';

export const KINDS: { kind: ContentKind; label: string }[] = [
  { kind: 'mod', label: 'Mod' },
  { kind: 'resourcepack', label: 'Resource pack' },
  { kind: 'shader', label: 'Shader' },
  { kind: 'datapack', label: 'Data pack' },
  { kind: 'modpack', label: 'Modpack' },
];

export function kindLabel(kind: ContentKind | string): string {
  return KINDS.find((k) => k.kind === kind)?.label ?? kind;
}

/** Categorie Modrinth più usate per le mod, in italiano. */
export const MOD_CATEGORIES: [string, string][] = [
  ['optimization', 'Ottimizzazione'],
  ['utility', 'Utilità'],
  ['decoration', 'Decorazione'],
  ['adventure', 'Avventura'],
  ['technology', 'Tecnologia'],
  ['worldgen', 'Generazione mondi'],
  ['magic', 'Magia'],
  ['storage', 'Inventario'],
  ['library', 'Librerie'],
];

/** Cartella dell'istanza per un tipo di contenuto (data pack e modpack no). */
export function dirOf(kind: ContentKind): ContentDirName | null {
  return kind === 'mod' ? 'mods' : kind === 'resourcepack' ? 'resourcepacks' : kind === 'shader' ? 'shaderpacks' : null;
}

/** Perché questo contenuto non si può installare nell'istanza (o null). */
export function incompatibility(kind: ContentKind, instance: Instance | null): string | null {
  if (!instance || instance.loader) return null;
  if (kind === 'mod') return 'Questa istanza è Vanilla: le mod funzionano solo con Fabric. Crea un’istanza Fabric per installarle.';
  if (kind === 'shader') return 'Questa istanza è Vanilla: gli shader funzionano con Iris, che richiede Fabric.';
  return null;
}

/** Iris: il launcher lo aggiunge da solo quando serve per uno shader. */
export const IRIS_PROJECT_ID = 'YL57xq9U';
