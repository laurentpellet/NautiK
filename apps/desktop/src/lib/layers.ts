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
