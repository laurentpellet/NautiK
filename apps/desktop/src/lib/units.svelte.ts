/**
 * Unités d'affichage choisies par l'utilisateur (onglet Paramètres).
 *
 * Le moteur travaille toujours en SI ; seule la présentation change. Le
 * choix est mémorisé localement (préférence d'interface, comme le thème).
 */

export type SpeedUnit = 'kn' | 'kmh' | 'ms';
export type DistanceUnit = 'nm' | 'km';
export type DepthUnit = 'm' | 'ft';

export interface UnitPrefs {
  speed: SpeedUnit;
  distance: DistanceUnit;
  depth: DepthUnit;
}

export const DEFAULT_UNITS: UnitPrefs = { speed: 'kn', distance: 'nm', depth: 'm' };

export const SPEED_UNITS: Record<SpeedUnit, { label: string; perMps: number }> = {
  kn: { label: 'kn', perMps: 3600 / 1852 },
  kmh: { label: 'km/h', perMps: 3.6 },
  ms: { label: 'm/s', perMps: 1 },
};

export const DISTANCE_UNITS: Record<DistanceUnit, { label: string; meters: number }> = {
  nm: { label: 'NM', meters: 1852 },
  km: { label: 'km', meters: 1000 },
};

export const DEPTH_UNITS: Record<DepthUnit, { label: string; meters: number }> = {
  m: { label: 'm', meters: 1 },
  ft: { label: 'ft', meters: 0.3048 },
};

const STORAGE_KEY = 'nautik.units';

function isKey<T extends string>(table: Record<T, unknown>, value: unknown): value is T {
  return typeof value === 'string' && Object.hasOwn(table, value);
}

/** Unités mémorisées ; toute valeur inconnue revient à la valeur par défaut. */
export function loadUnits(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): UnitPrefs {
  try {
    const raw: unknown = JSON.parse(storage?.getItem(STORAGE_KEY) ?? 'null');
    if (typeof raw !== 'object' || raw === null) return { ...DEFAULT_UNITS };
    const r = raw as Record<string, unknown>;
    return {
      speed: isKey(SPEED_UNITS, r.speed) ? r.speed : DEFAULT_UNITS.speed,
      distance: isKey(DISTANCE_UNITS, r.distance) ? r.distance : DEFAULT_UNITS.distance,
      depth: isKey(DEPTH_UNITS, r.depth) ? r.depth : DEFAULT_UNITS.depth,
    };
  } catch {
    return { ...DEFAULT_UNITS };
  }
}

export function saveUnits(
  prefs: UnitPrefs,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): void {
  try {
    storage?.setItem(STORAGE_KEY, JSON.stringify(prefs));
  } catch {
    // Préférence non mémorisée : sans conséquence.
  }
}

/** Unités courantes (réactives). */
export const units: UnitPrefs = $state(loadUnits());

/** Change les unités et les mémorise. */
export function setUnits(prefs: Partial<UnitPrefs>): void {
  Object.assign(units, prefs);
  saveUnits({ ...units });
}

/** Vitesse SI convertie dans l'unité courante. */
export function speedIn(mps: number, unit: SpeedUnit = units.speed): number {
  return mps * SPEED_UNITS[unit].perMps;
}

/** Vitesse dans l'unité courante convertie en SI. */
export function speedToMps(value: number, unit: SpeedUnit = units.speed): number {
  return value / SPEED_UNITS[unit].perMps;
}

export function distanceIn(meters: number, unit: DistanceUnit = units.distance): number {
  return meters / DISTANCE_UNITS[unit].meters;
}

export function depthIn(meters: number, unit: DepthUnit = units.depth): number {
  return meters / DEPTH_UNITS[unit].meters;
}
