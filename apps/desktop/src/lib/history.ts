/**
 * Historique en mémoire des mesures reçues depuis le lancement (aucune
 * persistance). Seules les mesures fraîches sont retenues : une donnée
 * obsolète ou absente laisse un trou, jamais une valeur répétée.
 */
import { appendSample, type Sample } from './dashboard';
import type { InstrumentsSnapshot, Quantity } from './services/instruments';
import { DEPTH_UNITS, SPEED_UNITS, depthIn, speedIn, units } from './units.svelte';

/** Durée conservée. */
export const HISTORY_WINDOW_MS = 6 * 60 * 60 * 1000;
/** Intervalle minimal entre deux points d'une même grandeur. */
export const HISTORY_MIN_INTERVAL_MS = 2_000;

/** Grandeurs historisées. */
export const HISTORY_QUANTITIES = [
  'speedOverGround',
  'speedThroughWater',
  'courseOverGroundTrue',
  'headingTrue',
  'headingMagnetic',
  'windSpeedApparent',
  'depthBelowTransducer',
  'engineSpeed',
  'batteryVoltage',
] as const satisfies readonly Quantity[];

export type HistoryQuantity = (typeof HISTORY_QUANTITIES)[number];
export type History = Partial<Record<HistoryQuantity, Sample[]>>;

const TRACKED: ReadonlySet<Quantity> = new Set(HISTORY_QUANTITIES);

/** Nature d'une courbe : détermine conversion, échelle et sens de l'axe. */
export type ChartKind = 'speed' | 'angle' | 'depth' | 'rpm' | 'voltage';

/** Valeur SI convertie dans l'unité d'affichage. */
export function chartValue(kind: ChartKind, si: number): number {
  switch (kind) {
    case 'speed':
      return speedIn(si);
    case 'depth':
      return depthIn(si);
    case 'angle': {
      const deg = (si * 180) / Math.PI;
      return ((deg % 360) + 360) % 360;
    }
    case 'rpm':
      return si * 60;
    case 'voltage':
      return si;
  }
}

/** Unité affichée. */
export function chartUnit(kind: ChartKind): string {
  switch (kind) {
    case 'speed':
      return SPEED_UNITS[units.speed].label;
    case 'depth':
      return DEPTH_UNITS[units.depth].label;
    case 'angle':
      return '°';
    case 'rpm':
      return 'tr/min';
    case 'voltage':
      return 'V';
  }
}

/** Échelle verticale `[bas, haut]` (profondeur : axe inversé ailleurs). */
export function chartRange(kind: ChartKind, max: number, min: number): [number, number] {
  switch (kind) {
    case 'angle':
      return [0, 360];
    case 'rpm':
      return [0, Math.max(1000, Math.ceil(max / 500) * 500)];
    case 'voltage':
      // Échelle resserrée autour des mesures (11 à 15 V typiquement).
      return [Math.floor(Math.min(min, 12)), Math.ceil(Math.max(max, 13))];
    case 'depth':
      return [0, Math.max(10, Math.ceil(max / 10) * 10)];
    case 'speed':
      return [0, Math.max(5, Math.ceil(max / 5) * 5)];
  }
}

/** Heure UTC « HH:MM » d'un horodatage. */
export function utcHourMinute(t: number): string {
  return new Date(t).toISOString().slice(11, 16);
}

/**
 * Ajoute les mesures fraîches d'un instantané. Retourne le même objet
 * si rien n'a changé (pas de rendu inutile).
 */
export function recordSnapshot(
  history: History,
  snapshot: InstrumentsSnapshot,
  windowMs: number = HISTORY_WINDOW_MS,
  minIntervalMs: number = HISTORY_MIN_INTERVAL_MS,
): History {
  let next: History | null = null;
  for (const r of snapshot.readings) {
    if (r.stale || !TRACKED.has(r.quantity) || !Number.isFinite(r.value)) continue;
    const q = r.quantity as HistoryQuantity;
    const samples = history[q] ?? [];
    const last = samples.at(-1);
    if (last && r.measuredAt - last.t < minIntervalMs) continue;
    const updated = appendSample(samples, { t: r.measuredAt, v: r.value }, windowMs);
    if (updated === samples) continue;
    next ??= { ...history };
    next[q] = updated;
  }
  return next ?? history;
}
