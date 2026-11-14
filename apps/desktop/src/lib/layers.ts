/** Calques de la carte affichables ou masquables par l'utilisateur. */

export interface MapLayers {
  routes: boolean;
  tracks: boolean;
  ais: boolean;
}

export const DEFAULT_LAYERS: MapLayers = { routes: true, tracks: true, ais: true };

export const LAYER_LABELS: Record<keyof MapLayers, string> = {
  routes: 'Routes et waypoints',
  tracks: 'Traces',
  ais: 'Cibles AIS',
};

const STORAGE_KEY = 'nautik.layers';

/** Choix mémorisé ; valeurs absentes ou invalides : affichées. */
export function loadLayers(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): MapLayers {
  try {
    const raw = storage?.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_LAYERS };
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== 'object' || parsed === null) return { ...DEFAULT_LAYERS };
    const record = parsed as Record<string, unknown>;
    const flag = (key: keyof MapLayers) =>
      typeof record[key] === 'boolean' ? (record[key] as boolean) : DEFAULT_LAYERS[key];
    return { routes: flag('routes'), tracks: flag('tracks'), ais: flag('ais') };
  } catch {
    return { ...DEFAULT_LAYERS };
  }
}

export function saveLayers(
  layers: MapLayers,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): void {
  try {
    storage?.setItem(STORAGE_KEY, JSON.stringify(layers));
  } catch {
    // Choix non mémorisé : sans conséquence.
  }
}

/** Calques masqués, pour le rappel affiché sur la carte. */
export function hiddenLayers(layers: MapLayers): string[] {
  return (Object.keys(LAYER_LABELS) as (keyof MapLayers)[])
    .filter((k) => !layers[k])
    .map((k) => LAYER_LABELS[k]);
}

const HIDDEN_CHARTS_KEY = 'nautik.hiddenCharts';

/** Nombre maximal d'identifiants de cartes masquées mémorisés. */
export const MAX_HIDDEN_CHARTS = 256;

/**
 * Cartes masquées (identifiants du catalogue) ; valeurs absentes ou
 * invalides : toutes les cartes sont affichées.
 */
export function loadHiddenCharts(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): string[] {
  try {
    const raw = storage?.getItem(HIDDEN_CHARTS_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    const ids = parsed.filter((v): v is string => typeof v === 'string' && v.length <= 64);
    return [...new Set(ids)].slice(0, MAX_HIDDEN_CHARTS);
  } catch {
    return [];
  }
}

export function saveHiddenCharts(
  ids: string[],
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): void {
  try {
    storage?.setItem(HIDDEN_CHARTS_KEY, JSON.stringify(ids.slice(0, MAX_HIDDEN_CHARTS)));
  } catch {
    // Choix non mémorisé : sans conséquence.
  }
}

/** Bascule l'affichage d'une carte. */
export function toggleChart(hidden: string[], id: string, visible: boolean): string[] {
  const rest = hidden.filter((h) => h !== id);
  return visible ? rest : [...rest, id];
}

/** Noms des cartes du catalogue masquées, pour le rappel sur la carte. */
export function hiddenChartNames(
  charts: { id: string; name: string }[],
  hidden: string[],
): string[] {
  return charts.filter((c) => hidden.includes(c.id)).map((c) => c.name);
}
