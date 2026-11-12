/** Formatage pour l'affichage (aucun calcul de navigation). */

function formatDegreesMinutes(value: number, positive: string, negative: string, pad: number) {
  const hemisphere = value < 0 ? negative : positive;
  const abs = Math.abs(value);
  let degrees = Math.floor(abs);
  let minutes = Number(((abs - degrees) * 60).toFixed(3));
  if (minutes >= 60) {
    degrees += 1;
    minutes = 0;
  }
  return `${String(degrees).padStart(pad, '0')}°${minutes.toFixed(3).padStart(6, '0')}′ ${hemisphere}`;
}

/** Latitude au format marin : `17°32.100′ S`. */
export function formatLatitude(latitudeDeg: number): string {
  return formatDegreesMinutes(latitudeDeg, 'N', 'S', 2);
}

/** Longitude au format marin : `149°34.140′ W`. */
export function formatLongitude(longitudeDeg: number): string {
  return formatDegreesMinutes(longitudeDeg, 'E', 'W', 3);
}

/** Âge lisible d'une donnée. */
export function formatAge(ageMs: number): string {
  const seconds = Math.max(0, Math.floor(ageMs / 1000));
  if (seconds < 60) return `${seconds} s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} min`;
  return `${Math.floor(minutes / 60)} h`;
}

/**
 * Seuil d'affichage « position obsolète » côté interface. La détection
 * d'obsolescence des sources sera portée par le moteur (phase 3) ; ce
 * contrôle garantit qu'un arrêt du flux reste visible.
 */
export const STALE_POSITION_MS = 5_000;

export function isStale(measuredAt: number, now: number): boolean {
  return now - measuredAt > STALE_POSITION_MS;
}
