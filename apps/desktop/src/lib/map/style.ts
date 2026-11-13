/**
 * Construction du style MapLibre à partir du catalogue de cartes.
 *
 * Le style est entièrement local : aucune police, aucun sprite ni
 * aucune source distante. Seules les tuiles servies par le moteur via
 * le protocole `nautik-tiles` sont utilisées.
 */
import type {
  LngLatBoundsLike,
  RasterLayerSpecification,
  RasterSourceSpecification,
  StyleSpecification,
} from 'maplibre-gl';
import type { ChartCatalog, ChartInfo } from '../services/charts';

export type Theme = 'day' | 'night';

export const BACKGROUND_LAYER_ID = 'background';

/** Couleur de fond (zones sans carte) par thème. */
export const BACKGROUND_COLOR: Record<Theme, string> = {
  day: '#dfe9ee',
  night: '#05080c',
};

/** Zoom maximal d'affichage sans carte. */
const DEFAULT_MAX_ZOOM = 18;
/** Sur-zoom autorisé au-delà du zoom maximal des cartes. */
const OVERZOOM = 3;
/** Zoom absolu maximal. */
const ABSOLUTE_MAX_ZOOM = 24;

type RasterPaint = NonNullable<RasterLayerSpecification['paint']>;

/** Échappe le HTML : MapLibre insère les attributions en HTML. */
export function escapeHtml(text: string): string {
  return text
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

export function chartSourceId(chart: ChartInfo): string {
  return `chart-${chart.id}`;
}

export function chartLayerId(chart: ChartInfo): string {
  return `chart-${chart.id}`;
}

export function tileUrlTemplate(tileUrlBase: string, chart: ChartInfo): string {
  return `${tileUrlBase}/${chart.id}/{z}/{x}/{y}`;
}

/** Réglages des couches raster par thème (mode nuit : luminosité réduite). */
export function rasterPaint(theme: Theme): RasterPaint {
  return theme === 'night'
    ? { 'raster-brightness-max': 0.35, 'raster-saturation': -0.4, 'raster-contrast': 0.1 }
    : { 'raster-brightness-max': 1, 'raster-saturation': 0, 'raster-contrast': 0 };
}

export function chartSource(tileUrlBase: string, chart: ChartInfo): RasterSourceSpecification {
  const source: RasterSourceSpecification = {
    type: 'raster',
    tiles: [tileUrlTemplate(tileUrlBase, chart)],
    tileSize: 256,
    minzoom: chart.minZoom,
    maxzoom: chart.maxZoom,
  };
  if (chart.bounds) {
    const { west, south, east, north } = chart.bounds;
    source.bounds = [west, south, east, north];
  }
  if (chart.attribution) {
    source.attribution = escapeHtml(chart.attribution);
  }
  return source;
}

/** Style complet : fond + une couche raster par carte, dans l'ordre du catalogue. */
export function buildStyle(catalog: ChartCatalog, theme: Theme): StyleSpecification {
  const sources: StyleSpecification['sources'] = {};
  const layers: StyleSpecification['layers'] = [
    {
      id: BACKGROUND_LAYER_ID,
      type: 'background',
      paint: { 'background-color': BACKGROUND_COLOR[theme] },
    },
  ];
  for (const chart of catalog.charts) {
    sources[chartSourceId(chart)] = chartSource(catalog.tileUrlBase, chart);
    layers.push({
      id: chartLayerId(chart),
      type: 'raster',
      source: chartSourceId(chart),
      paint: rasterPaint(theme),
    });
  }
  return { version: 8, sources, layers };
}

export function maxMapZoom(catalog: ChartCatalog): number {
  if (catalog.charts.length === 0) {
    return DEFAULT_MAX_ZOOM;
  }
  const max = Math.max(...catalog.charts.map((c) => c.maxZoom));
  return Math.min(ABSOLUTE_MAX_ZOOM, max + OVERZOOM);
}

/** Zoom minimal au démarrage (échelle utile à la navigation et à l'AIS). */
export const MIN_INITIAL_ZOOM = 10;

/** Zoom de démarrage : jamais en deçà de [`MIN_INITIAL_ZOOM`]. */
export function startZoom(zoom: number): number {
  return Math.max(zoom, MIN_INITIAL_ZOOM);
}

export type InitialView =
  | { kind: 'bounds'; bounds: LngLatBoundsLike }
  | { kind: 'center'; center: [number, number]; zoom: number };

/**
 * Vue initiale : carte la plus détaillée (dernière du catalogue), sinon
 * l'origine. Le zoom est ensuite porté à [`MIN_INITIAL_ZOOM`] au moins
 * (voir [`startZoom`]) et la carte se centre sur la première position.
 */
export function initialView(catalog: ChartCatalog): InitialView {
  const chart = catalog.charts.at(-1);
  if (chart?.bounds) {
    const { west, south, east, north } = chart.bounds;
    return {
      kind: 'bounds',
      bounds: [
        [west, south],
        [east, north],
      ],
    };
  }
  if (chart?.center) {
    return {
      kind: 'center',
      center: [chart.center.longitude, chart.center.latitude],
      zoom: startZoom(chart.center.zoom ?? chart.minZoom),
    };
  }
  return { kind: 'center', center: [0, 0], zoom: MIN_INITIAL_ZOOM };
}
