/**
 * Carte partagée en deux volets côte à côte. Verrouillées, les deux cartes
 * suivent la même vue (centre, zoom, orientation) : le volet que
 * l'utilisateur manipule mène, l'autre suit.
 */

export type Pane = 'left' | 'right';

/** Vue d'une carte : centre [longitude, latitude] en degrés WGS84. */
export interface MapCamera {
  center: [number, number];
  zoom: number;
  bearing: number;
}

export interface SplitState {
  enabled: boolean;
  locked: boolean;
  /** Cartes masquées dans le volet de droite. */
  hiddenCharts: string[];
}

/** Nombre maximal d'identifiants de cartes mémorisés. */
const MAX_HIDDEN = 256;
const STORAGE_KEY = 'nautik.split';

export function defaultSplit(): SplitState {
  return { enabled: false, locked: true, hiddenCharts: [] };
}

export function loadSplit(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): SplitState {
  const state = defaultSplit();
  try {
    const raw = storage?.getItem(STORAGE_KEY);
    if (!raw) return state;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== 'object' || parsed === null) return state;
    const r = parsed as Record<string, unknown>;
    if (typeof r.enabled === 'boolean') state.enabled = r.enabled;
    if (typeof r.locked === 'boolean') state.locked = r.locked;
    if (Array.isArray(r.hiddenCharts)) {
      state.hiddenCharts = r.hiddenCharts
        .filter((id): id is string => typeof id === 'string' && id.length > 0 && id.length <= 256)
        .slice(0, MAX_HIDDEN);
    }
    return state;
  } catch {
    return defaultSplit();
  }
}

export function saveSplit(
  state: SplitState,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): void {
  try {
    storage?.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // Choix non mémorisé : sans conséquence.
  }
}

/** Vue exploitable : nombres finis, latitude et zoom dans les bornes. */
export function isValidCamera(camera: MapCamera): boolean {
  const [lon, lat] = camera.center;
  return (
    [lon, lat, camera.zoom, camera.bearing].every(Number.isFinite) &&
    Math.abs(lat) <= 90 &&
    Math.abs(lon) <= 540 &&
    camera.zoom >= 0 &&
    camera.zoom <= 24
  );
}

/**
 * Vue que doit suivre un volet : celle du volet meneur, seulement si les
 * cartes sont partagées et verrouillées. `null` : le volet est libre.
 */
export function followedCamera(
  pane: Pane,
  state: Pick<SplitState, 'enabled' | 'locked'>,
  leader: Pane,
  cameras: Partial<Record<Pane, MapCamera | null>>,
): MapCamera | null {
  if (!state.enabled || !state.locked || leader === pane) return null;
  const camera = cameras[leader] ?? null;
  return camera && isValidCamera(camera) ? camera : null;
}
