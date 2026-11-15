/**
 * Couleur de la route active sur la carte (onglet Paramètres).
 *
 * Préférence d'interface mémorisée localement, comme les unités ; toute
 * valeur invalide revient au jaune par défaut.
 */
import { DEFAULT_ACTIVE_ROUTE_COLOR } from './map/overlays';

const STORAGE_KEY = 'nautik.activeRouteColor';
const HEX_COLOR = /^#[0-9a-f]{6}$/;

/** Couleur `#rrggbb` normalisée en minuscules, ou `null` si invalide. */
export function parseColor(value: unknown): string | null {
  if (typeof value !== 'string') return null;
  const color = value.trim().toLowerCase();
  return HEX_COLOR.test(color) ? color : null;
}

export function loadActiveRouteColor(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): string {
  try {
    return parseColor(storage?.getItem(STORAGE_KEY)) ?? DEFAULT_ACTIVE_ROUTE_COLOR;
  } catch {
    return DEFAULT_ACTIVE_ROUTE_COLOR;
  }
}

/** Couleur courante (réactive). */
export const routeStyle: { activeColor: string } = $state({
  activeColor: loadActiveRouteColor(),
});

/** Change la couleur de la route active et la mémorise ; refuse une couleur invalide. */
export function setActiveRouteColor(
  value: string,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): boolean {
  const color = parseColor(value);
  if (color === null) return false;
  routeStyle.activeColor = color;
  try {
    storage?.setItem(STORAGE_KEY, color);
  } catch {
    // Préférence non mémorisée : sans conséquence.
  }
  return true;
}
