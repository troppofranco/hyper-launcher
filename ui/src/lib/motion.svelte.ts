// Livello di animazione scelto nelle Impostazioni, combinato con "riduci
// animazioni" del sistema operativo: tutte le transizioni passano da qui.
import { prefersReducedMotion } from 'svelte/motion';

export type MotionLevel = 'full' | 'reduced' | 'off';

let level = $state<MotionLevel>('full');

export function setMotion(next: MotionLevel) {
  level = next;
}

/** Le animazioni sono attive (non spente né ridotte dal sistema). */
export function moving(): boolean {
  return level !== 'off' && !prefersReducedMotion.current;
}

/** Durata di una transizione: 0 senza animazioni, metà se ridotte. */
export function ms(full: number): number {
  if (!moving()) return 0;
  return level === 'reduced' ? Math.round(full / 2) : full;
}

/** Livello scelto (le animazioni decorative, come il tremolio, solo se "full"). */
export function motionLevel(): MotionLevel {
  return moving() ? level : 'off';
}
