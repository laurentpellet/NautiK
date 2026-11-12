/** Formatage pour l'affichage (aucun calcul de navigation). */
import type { ConnectionState } from './services/instruments';

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

/** Vitesse en nœuds : `6.4 kn`. */
export function formatSpeed(mps: number | null): string {
  if (mps === null) return '—';
  return `${(mps * KNOTS_PER_MPS).toFixed(1)} kn`;
}

/** Profondeur en mètres : `12.5 m` (`125 m` à partir de 100 m). */
export function formatDepth(meters: number | null): string {
  if (meters === null) return '—';
  return `${meters < 100 ? meters.toFixed(1) : meters.toFixed(0)} m`;
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

/** Distance en milles marins : `12.3 NM` (`0.05 NM` sous 1 NM). */
export function formatDistance(meters: number): string {
  const nm = meters / METERS_PER_NM;
  return `${nm < 1 ? nm.toFixed(2) : nm.toFixed(1)} NM`;
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

/** Heure UTC `HH:MM` et durée restante : `14:32 UTC (dans 1 h 05)`. */
export function formatEta(eta: number | null, now: number): string {
  if (eta === null) return '—';
  const d = new Date(eta);
  const clock = `${String(d.getUTCHours()).padStart(2, '0')}:${String(d.getUTCMinutes()).padStart(2, '0')} UTC`;
  const minutes = Math.max(0, Math.round((eta - now) / 60_000));
  const remaining =
    minutes < 60
      ? `${minutes} min`
      : `${Math.floor(minutes / 60)} h ${String(minutes % 60).padStart(2, '0')}`;
  return `${clock} (dans ${remaining})`;
}

/** Gravité d'un état de connexion, pour l'affichage. */
export type ConnectionLevel = 'ok' | 'pending' | 'alert';

/** Libellé d'un état de connexion Signal K (`null` : pas encore d'état). */
export function describeConnection(state: ConnectionState | null): {
  label: string;
  level: ConnectionLevel;
} {
  if (state === null) return { label: 'En attente du client Signal K', level: 'pending' };
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
      return { label: 'Client Signal K arrêté', level: 'alert' };
  }
}
