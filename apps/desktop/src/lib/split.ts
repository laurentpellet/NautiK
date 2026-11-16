/**
 * Carte partagée en deux volets côte à côte. Verrouillées, les deux cartes
 * suivent la même vue (centre, zoom, orientation) : le volet que
 * l'utilisateur manipule mène, l'autre suit.
 *
 * Le volet de droite sert à comparer : il affiche une seule carte, choisie
 * parmi celles qui ne sont pas déjà affichées à gauche.
 */
import type { VectorCharts } from './layers';

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
  /**
   * Carte choisie pour le volet de droite (voir {@link rightChartOptions}) ;
   * `null` : pas encore choisie.
   */
  chart: string | null;
}

/** Longueur maximale d'un choix mémorisé. */
const MAX_CHOICE_CHARS = 300;
const STORAGE_KEY = 'nautik.split';

export function defaultSplit(): SplitState {
  return { enabled: false, locked: true, chart: null };
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
    if (typeof r.chart === 'string' && r.chart.length > 0 && r.chart.length <= MAX_CHOICE_CHARS) {
      state.chart = r.chart;
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

/** Choix d'une carte MBTiles pour le volet de droite. */
export function chartChoice(id: string): string {
  return `chart:${id}`;
}

/**
 * Cartes proposées au volet de droite : les cartes MBTiles masquées à
 * gauche, dans l'ordre donné, puis les cartes vectorielles disponibles
 * non affichées à gauche. Comparer une carte à elle-même n'apprend rien.
 */
export function rightChartOptions(
  chartIds: string[],
  leftHidden: string[],
  vectorAvailable: (keyof VectorCharts)[],
  leftVector: VectorCharts,
): string[] {
  return [
    ...chartIds.filter((id) => leftHidden.includes(id)).map(chartChoice),
    ...vectorAvailable.filter((key) => !leftVector[key]),
  ];
}

/**
 * Carte affichée à droite : le choix mémorisé s'il est proposé, sinon la
 * première carte proposée ; `null` si rien n'est proposé.
 */
export function effectiveRightChart(chosen: string | null, options: string[]): string | null {
  return chosen !== null && options.includes(chosen) ? chosen : (options[0] ?? null);
}

/** Cartes MBTiles masquées et cartes vectorielles du volet de droite. */
export function rightPaneCharts(
  choice: string,
  chartIds: string[],
): { hiddenCharts: string[]; vector: VectorCharts } {
  return {
    hiddenCharts: chartIds.filter((id) => chartChoice(id) !== choice),
    vector: { enc: choice === 'enc', cm93: choice === 'cm93' },
  };
}
