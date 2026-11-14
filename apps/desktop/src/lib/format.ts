/** Formatage pour l'affichage (aucun calcul de navigation). */
import type { ConnectionState } from './services/instruments';
import {
  DEPTH_UNITS,
  DISTANCE_UNITS,
  SPEED_UNITS,
  depthIn,
  distanceIn,
  speedIn,
  units,
} from './units.svelte';

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
 * Silence maximal du moteur avant alerte. Le moteur diffuse un instantané
 * chaque seconde (avec la fraîcheur de chaque donnée) ; au-delà de ce
 * délai, les valeurs affichées ne sont plus garanties et l'interface le
 * signale.
 */
export const ENGINE_SILENCE_MS = 3_000;

/** Le moteur n'a rien diffusé depuis trop longtemps. */
export function isEngineSilent(lastSnapshotAt: number | null, now: number): boolean {
  return lastSnapshotAt === null || now - lastSnapshotAt > ENGINE_SILENCE_MS;
}

/** Nœuds par mètre/seconde. */
export const KNOTS_PER_MPS = 3600 / 1852;

/** Valeur et unité séparées (affichage en grands caractères). */
export interface Parts {
  value: string;
  unit: string;
}

/** Vitesse dans l'unité choisie : `6.4` `kn`. */
export function speedParts(mps: number): Parts {
  return { value: speedIn(mps).toFixed(1), unit: SPEED_UNITS[units.speed].label };
}

/** Profondeur dans l'unité choisie (sans décimale à partir de 100). */
export function depthParts(meters: number): Parts {
  const v = depthIn(meters);
  return { value: v < 100 ? v.toFixed(1) : v.toFixed(0), unit: DEPTH_UNITS[units.depth].label };
}

/** Distance dans l'unité choisie (2 décimales sous 1). */
export function distanceParts(meters: number): Parts {
  const v = distanceIn(meters);
  return {
    value: v < 1 ? v.toFixed(2) : v.toFixed(1),
    unit: DISTANCE_UNITS[units.distance].label,
  };
}

const joined = (p: Parts) => `${p.value} ${p.unit}`;

/** Vitesse : `6.4 kn` (unité choisie). */
export function formatSpeed(mps: number | null): string {
  return mps === null ? '—' : joined(speedParts(mps));
}

/** Profondeur : `12.5 m` (`125 m` à partir de 100 ; unité choisie). */
export function formatDepth(meters: number | null): string {
  return meters === null ? '—' : joined(depthParts(meters));
}

/** Angle relatif (vent apparent) : `045° Td` (tribord) ou `120° Bd` (bâbord). */
export function formatRelativeAngle(radians: number | null): string {
  if (radians === null) return '—';
  const degrees = Math.round((Math.abs(radians) * 180) / Math.PI);
  const text = `${String(degrees).padStart(3, '0')}°`;
  if (degrees === 0 || degrees === 180) return text;
  return `${text} ${radians > 0 ? 'Td' : 'Bd'}`;
}

/** Mètres par mille marin. */
export const METERS_PER_NM = 1852;

/** Distance : `12.3 NM` (`0.05 NM` sous 1 ; unité choisie). */
export function formatDistance(meters: number): string {
  return joined(distanceParts(meters));
}

/** Relèvement vrai en degrés : `036°`. */
export function formatBearing(radians: number | null): string {
  if (radians === null) return '—';
  const degrees = Math.round((((radians * 180) / Math.PI) % 360) + 360) % 360;
  return `${String(degrees).padStart(3, '0')}°`;
}

/** Écart de route : `0.05 NM D` (à droite) ou `G` (à gauche). */
export function formatCrossTrack(meters: number | null): string {
  if (meters === null) return '—';
  const side = meters > 0 ? ' D' : meters < 0 ? ' G' : '';
  return `${formatDistance(Math.abs(meters))}${side}`;
}

/** Heure UTC `HH:MM` (sans le suffixe UTC). */
export function formatUtcClock(time: number): string {
  const d = new Date(time);
  return `${String(d.getUTCHours()).padStart(2, '0')}:${String(d.getUTCMinutes()).padStart(2, '0')}`;
}

/**
 * Durée arrondie à la minute : `45 min`, `1 h 05`, `2 j 03 h` (au-delà de
 * 48 h). Négative : 0 min.
 */
export function formatDuration(ms: number): string {
  const minutes = Math.max(0, Math.round(ms / 60_000));
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  if (hours < 48) return `${hours} h ${String(minutes % 60).padStart(2, '0')}`;
  return `${Math.floor(hours / 24)} j ${String(hours % 24).padStart(2, '0')} h`;
}

/** Heure UTC `HH:MM` et durée restante : `14:32 UTC (dans 1 h 05)`. */
export function formatEta(eta: number | null, now: number): string {
  if (eta === null) return '—';
  return `${formatUtcClock(eta)} UTC (dans ${formatDuration(eta - now)})`;
}

/** Gravité d'un état de connexion, pour l'affichage. */
export type ConnectionLevel = 'ok' | 'pending' | 'alert';

/** Libellé d'un état de liaison (`null` : pas encore d'état). */
export function describeConnection(state: ConnectionState | null): {
  label: string;
  level: ConnectionLevel;
} {
  if (state === null) return { label: 'En attente de la liaison', level: 'pending' };
  switch (state.state) {
    case 'connecting':
      return { label: `Connexion en cours (tentative ${state.attempt})`, level: 'pending' };
    case 'connected': {
      const server = [state.serverName, state.serverVersion].filter((s) => s !== null).join(' ');
      return { label: server ? `Connecté — ${server}` : 'Connecté', level: 'ok' };
    }
    case 'disconnected':
      return {
        label: `Déconnecté : ${state.reason} — nouvel essai dans ${formatAge(state.retryInMs)}`,
        level: 'alert',
      };
    case 'unauthorized':
      return {
        label: `Accès refusé : ${state.reason} — vérifiez le jeton (nouvel essai dans ${formatAge(state.retryInMs)})`,
        level: 'alert',
      };
    case 'stopped':
      return { label: 'Liaison arrêtée', level: 'alert' };
  }
}
