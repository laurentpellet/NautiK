/**
 * Durée de projection des lignes de dérive (route fond et ligne de foi)
 * choisie dans Paramètres → Instruments : les deux lignes partent du
 * bateau et portent une graduation par minute.
 *
 * Préférence d'interface mémorisée localement ; une valeur illisible ou
 * hors bornes revient à la valeur par défaut.
 */

/** Durée par défaut (min) : graduations à 1, 2, 3, 4 et 5 minutes. */
export const DEFAULT_DRIFT_MINUTES = 5;
export const MIN_DRIFT_MINUTES = 1;
export const MAX_DRIFT_MINUTES = 30;

const STORAGE_KEY = 'nautik.driftMinutes';

/** Durée valide (entier de 1 à 30), sinon `null`. */
export function parseDriftMinutes(value: unknown): number | null {
  const n = typeof value === 'string' && value.trim() !== '' ? Number(value) : value;
  if (typeof n !== 'number' || !Number.isInteger(n)) return null;
  return n >= MIN_DRIFT_MINUTES && n <= MAX_DRIFT_MINUTES ? n : null;
}

export function loadDriftMinutes(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): number {
  try {
    return parseDriftMinutes(storage?.getItem(STORAGE_KEY) ?? null) ?? DEFAULT_DRIFT_MINUTES;
  } catch {
    return DEFAULT_DRIFT_MINUTES;
  }
}

/** Durée choisie (réactive). */
export const driftProjection: { minutes: number } = $state({ minutes: loadDriftMinutes() });

/** Règle la durée ; refuse (et garde l'ancienne) une valeur invalide. */
export function setDriftMinutes(
  value: unknown,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): boolean {
  const minutes = parseDriftMinutes(value);
  if (minutes === null) return false;
  driftProjection.minutes = minutes;
  try {
    storage?.setItem(STORAGE_KEY, String(minutes));
  } catch {
    // Préférence non mémorisée : sans conséquence.
  }
  return true;
}
