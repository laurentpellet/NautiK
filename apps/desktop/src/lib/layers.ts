/** Calques de la carte affichables ou masquables par l'utilisateur. */

export interface MapLayers {
  routes: boolean;
  tracks: boolean;
  ais: boolean;
  /** Cartes ENC S-57 du dossier choisi. */
  enc: boolean;
  /** Cartes CM93 de la base trouvée dans le dossier ENC. */
  cm93: boolean;
  /** Îles et amers téléchargés (OpenStreetMap). */
  poi: boolean;
  /** Marques personnelles. */
  marks: boolean;
  /** Vecteur de route (COG/SOG) et ligne de cap du bateau. */
  vectors: boolean;
  /** Laylines du waypoint actif. */
  laylines: boolean;
  /** Dérive : route fond et ligne de foi graduées à l'avant du bateau. */
  drift: boolean;
}

export const DEFAULT_LAYERS: MapLayers = {
  routes: true,
  tracks: true,
  ais: true,
  enc: true,
  cm93: true,
  poi: true,
  marks: true,
  vectors: true,
  laylines: true,
  drift: false,
};

export const LAYER_LABELS: Record<keyof MapLayers, string> = {
  routes: 'Routes et waypoints',
  tracks: 'Traces',
  ais: 'Cibles AIS',
  enc: 'Cartes ENC (S-57)',
  cm93: 'Cartes CM93',
  poi: 'Îles et amers (OSM)',
  marks: 'Marques',
  vectors: 'Vecteur et ligne de cap',
  laylines: 'Laylines',
  drift: 'Dérive (COG et ligne de foi)',
};

/** Cartes vectorielles d'un volet : ENC officielles et base CM93. */
export type VectorCharts = Pick<MapLayers, 'enc' | 'cm93'>;

/** Calques des cartes vectorielles (groupe « Cartes » du menu). */
export const VECTOR_KEYS = ['enc', 'cm93'] as const;

/** Calques d'objets (routes, traces, marques, AIS, îles et amers, aides). */
export const OBJECT_KEYS = [
  'routes',
  'tracks',
  'marks',
  'ais',
  'poi',
  'vectors',
  'drift',
  'laylines',
] as const;

/**
 * Cartes vectorielles proposées : ENC si le dossier contient des cellules
 * S-57, CM93 si une base est trouvée.
 */
export function vectorKeys(cm93Available: boolean, encAvailable = true): (keyof VectorCharts)[] {
  return VECTOR_KEYS.filter((k) => (k === 'enc' ? encAvailable : cm93Available));
}

/** Libellé du calque : nom choisi pour la CM93, sinon libellé par défaut. */
export function layerLabel(key: keyof MapLayers, cm93Name: string | null = null): string {
  return key === 'cm93' && cm93Name ? cm93Name : LAYER_LABELS[key];
}

/**
 * Libellé court des cartes vectorielles affichées (« ENC, CM93 »), avec le
 * nom choisi pour la CM93.
 */
export function vectorLabel(
  charts: VectorCharts,
  cm93Available: boolean,
  cm93Name: string | null = null,
  encAvailable = true,
): string[] {
  const names: string[] = [];
  if (encAvailable && charts.enc) names.push('ENC');
  if (cm93Available && charts.cm93) names.push(cm93Name ?? 'CM93');
  return names;
}

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
    return {
      routes: flag('routes'),
      tracks: flag('tracks'),
      ais: flag('ais'),
      enc: flag('enc'),
      cm93: flag('cm93'),
      poi: flag('poi'),
      marks: flag('marks'),
      vectors: flag('vectors'),
      laylines: flag('laylines'),
      drift: flag('drift'),
    };
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

/**
 * Calques masqués, pour le rappel affiché sur la carte (CM93 seulement
 * si une base est trouvée, ENC seulement s'il y a des cellules).
 */
export function hiddenLayers(
  layers: MapLayers,
  cm93Available = false,
  cm93Name: string | null = null,
  encAvailable = true,
): string[] {
  return (
    (Object.keys(LAYER_LABELS) as (keyof MapLayers)[])
      // Dérive : aide facultative, masquée par défaut, jamais rappelée.
      .filter((k) => k !== 'drift')
      .filter((k) => !layers[k] && (k !== 'cm93' || cm93Available) && (k !== 'enc' || encAvailable))
      .map((k) => layerLabel(k, cm93Name))
  );
}

/**
 * Routes dessinées sur la carte. Le calque Routes masque les routes
 * enregistrées, jamais la route suivie en navigation : la perdre de vue
 * sans s'en rendre compte serait dangereux.
 */
export function visibleRoutes<R extends { active: boolean }>(
  routes: R[],
  layers: Pick<MapLayers, 'routes'>,
): R[] {
  return layers.routes ? routes : routes.filter((r) => r.active);
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
