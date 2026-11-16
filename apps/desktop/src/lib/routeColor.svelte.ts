/**
 * Couleurs de la route active et des traces sur la carte (onglet
 * Paramètres).
 *
 * Préférences d'interface mémorisées localement, comme les unités ; toute
 * valeur invalide revient à la couleur par défaut (jaune pour la route
 * active, rose pour les traces).
 */
import { DEFAULT_ACTIVE_ROUTE_COLOR, DEFAULT_TRACK_COLOR } from './map/overlays';

const STORAGE_KEY = 'nautik.activeRouteColor';
const TRACK_STORAGE_KEY = 'nautik.trackColor';
const HEX_COLOR = /^#[0-9a-f]{6}$/;

/** Couleur `#rrggbb` normalisée en minuscules, ou `null` si invalide. */
export function parseColor(value: unknown): string | null {
  if (typeof value !== 'string') return null;
  const color = value.trim().toLowerCase();
  return HEX_COLOR.test(color) ? color : null;
}

function loadColor(
  key: string,
  fallback: string,
  storage: Pick<Storage, 'getItem'> | undefined,
): string {
  try {
    return parseColor(storage?.getItem(key)) ?? fallback;
  } catch {
    return fallback;
  }
}

function saveColor(key: string, color: string, storage: Pick<Storage, 'setItem'> | undefined) {
  try {
    storage?.setItem(key, color);
  } catch {
    // Préférence non mémorisée : sans conséquence.
  }
}

export function loadActiveRouteColor(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): string {
  return loadColor(STORAGE_KEY, DEFAULT_ACTIVE_ROUTE_COLOR, storage);
}

export function loadTrackColor(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): string {
  return loadColor(TRACK_STORAGE_KEY, DEFAULT_TRACK_COLOR, storage);
}

/** Couleurs courantes (réactives). */
export const routeStyle: { activeColor: string; trackColor: string } = $state({
  activeColor: loadActiveRouteColor(),
  trackColor: loadTrackColor(),
});

/** Change la couleur de la route active et la mémorise ; refuse une couleur invalide. */
export function setActiveRouteColor(
  value: string,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): boolean {
  const color = parseColor(value);
  if (color === null) return false;
  routeStyle.activeColor = color;
  saveColor(STORAGE_KEY, color, storage);
  return true;
}

/** Change la couleur des traces (en cours comprise) et la mémorise ; refuse une couleur invalide. */
export function setTrackColor(
  value: string,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): boolean {
  const color = parseColor(value);
  if (color === null) return false;
  routeStyle.trackColor = color;
  saveColor(TRACK_STORAGE_KEY, color, storage);
  return true;
}
