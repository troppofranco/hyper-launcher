// Etichette e regole condivise dalle schermate del catalogo.

import type { ContentDirName, ContentKind, Instance, LoaderKind } from './api';

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
  if (!instance) return null;
  if (kind === 'mod' && !instance.loader) return 'Questa istanza è Vanilla: per le mod serve un loader (Fabric, Quilt, NeoForge o Forge).';
  if (kind === 'shader' && (!instance.loader || instance.loader.kind === 'forge'))
    return 'Gli shader funzionano con Iris, disponibile per Fabric, Quilt e NeoForge.';
  return null;
}

/** Mod loader supportati, con il nome da mostrare (id = nome Modrinth). */
export const LOADERS: [LoaderKind, string][] = [
  ['fabric', 'Fabric'],
  ['quilt', 'Quilt'],
  ['neoforge', 'NeoForge'],
  ['forge', 'Forge'],
];

/** Iris: il launcher lo aggiunge da solo quando serve per uno shader. */
export const IRIS_PROJECT_ID = 'YL57xq9U';
