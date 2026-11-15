/**
 * Noms des cartes MBTiles choisis dans Paramètres → Cartes.
 *
 * Préférence d'interface mémorisée localement, par identifiant de carte
 * (tiré du nom du fichier, donc stable d'une relecture à l'autre). Sans
 * nom choisi, la carte garde le nom de ses métadonnées. Un nom vide
 * revient au nom d'origine ; une entrée illisible est ignorée.
 */
import type { ChartInfo } from './services/charts';

/** Longueur maximale d'un nom choisi. */
export const MAX_CHART_NAME_CHARS = 60;

const STORAGE_KEY = 'nautik.chartNames';

/** Nom nettoyé (espaces de bord et répétés), `null` s'il est vide. */
export function cleanChartName(value: string): string | null {
  const name = value.replace(/\s+/g, ' ').trim().slice(0, MAX_CHART_NAME_CHARS).trim();
  return name === '' ? null : name;
}

/** Noms mémorisés validés. */
export function parseChartNames(value: unknown): Record<string, string> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return {};
  const names: Record<string, string> = {};
  for (const [id, raw] of Object.entries(value as Record<string, unknown>)) {
    const name = typeof raw === 'string' ? cleanChartName(raw) : null;
    if (id !== '' && name !== null) names[id] = name;
  }
  return names;
}

export function loadChartNames(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): Record<string, string> {
  try {
    const raw = storage?.getItem(STORAGE_KEY);
    return raw ? parseChartNames(JSON.parse(raw)) : {};
  } catch {
    return {};
  }
}

/** Noms choisis (réactifs). */
export const chartNames: { names: Record<string, string> } = $state({ names: loadChartNames() });

/** Donne un nom à une carte ; vide : retour au nom d'origine. */
export function setChartName(
  id: string,
  value: string,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): void {
  const name = cleanChartName(value);
  const names = Object.fromEntries(Object.entries(chartNames.names).filter(([key]) => key !== id));
  if (name !== null) names[id] = name;
  chartNames.names = names;
  try {
    storage?.setItem(STORAGE_KEY, JSON.stringify(names));
  } catch {
    // Préférence non mémorisée : sans conséquence.
  }
}

/** Cartes avec le nom à afficher (choisi, sinon celui des métadonnées). */
export function withChartNames(charts: ChartInfo[], names: Record<string, string>): ChartInfo[] {
  return charts.map((c) => {
    const name = names[c.id];
    return name ? { ...c, name } : c;
  });
}

const CM93_STORAGE_KEY = 'nautik.cm93Name';

export function loadCm93Name(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): string | null {
  try {
    const raw = storage?.getItem(CM93_STORAGE_KEY);
    return raw ? cleanChartName(raw) : null;
  } catch {
    return null;
  }
}

/** Nom choisi pour la base CM93 (réactif), `null` : nom par défaut. */
export const cm93Name: { name: string | null } = $state({ name: loadCm93Name() });

/** Nomme la base CM93 ; vide : retour au nom par défaut. */
export function setCm93Name(
  value: string,
  storage: Pick<Storage, 'setItem' | 'removeItem'> | undefined = globalThis.localStorage,
): void {
  const name = cleanChartName(value);
  cm93Name.name = name;
  try {
    if (name === null) storage?.removeItem(CM93_STORAGE_KEY);
    else storage?.setItem(CM93_STORAGE_KEY, name);
  } catch {
    // Préférence non mémorisée : sans conséquence.
  }
}

/** Tri des listes de cartes (Paramètres → Cartes, menu Calques). */
export type ChartSort = 'detail' | 'name' | 'file';

export const CHART_SORTS: { id: ChartSort; label: string }[] = [
  { id: 'detail', label: 'Détail (la plus détaillée en tête)' },
  { id: 'name', label: 'Nom' },
  { id: 'file', label: 'Fichier' },
];

const SORT_STORAGE_KEY = 'nautik.chartSort';

const isChartSort = (v: unknown): v is ChartSort => CHART_SORTS.some((s) => s.id === v);

export function loadChartSort(
  storage: Pick<Storage, 'getItem'> | undefined = globalThis.localStorage,
): ChartSort {
  try {
    const raw = storage?.getItem(SORT_STORAGE_KEY);
    return isChartSort(raw) ? raw : 'detail';
  } catch {
    return 'detail';
  }
}

/** Tri choisi (réactif). */
export const chartSort: { value: ChartSort } = $state({ value: loadChartSort() });

export function setChartSort(
  value: string,
  storage: Pick<Storage, 'setItem'> | undefined = globalThis.localStorage,
): void {
  if (!isChartSort(value)) return;
  chartSort.value = value;
  try {
    storage?.setItem(SORT_STORAGE_KEY, value);
  } catch {
    // Préférence non mémorisée : sans conséquence.
  }
}

/**
 * Cartes triées pour l'affichage d'une liste (l'empilement sur la carte,
 * lui, reste de la moins à la plus détaillée). `charts` : ordre du
 * catalogue, de la moins à la plus détaillée ; `names` : noms choisis,
 * utilisés pour le tri par nom.
 */
export function sortCharts(
  charts: ChartInfo[],
  sort: ChartSort,
  names: Record<string, string> = {},
): ChartInfo[] {
  if (sort === 'detail') return [...charts].reverse();
  const key = (c: ChartInfo) => (sort === 'name' ? (names[c.id] ?? c.name) : c.fileName);
  return [...charts].sort((a, b) =>
    key(a).localeCompare(key(b), 'fr', { sensitivity: 'base', numeric: true }),
  );
}
