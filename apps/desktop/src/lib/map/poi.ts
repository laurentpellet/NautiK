/**
 * Couche des îles et amers (OpenStreetMap) : noms des archipels, îles et
 * îlots selon le zoom, passes, balises et bouées colorées selon leur
 * catégorie (bâbord rouge, tribord vert, cardinales jaunes…), feux,
 * mouillages et ports.
 *
 * Symbologie simplifiée et non officielle : ces données complètent les
 * cartes marines sans les remplacer. Fonctions pures ; ChartView les
 * applique.
 */
import type { Feature, FeatureCollection, Point } from 'geojson';
import type {
  DataDrivenPropertyValueSpecification,
  ExpressionSpecification,
  LayerSpecification,
} from 'maplibre-gl';
import type { Poi, PoiKind } from '../services/poi';
import { labelImageExpression, labelText, type LabelStyle } from './labels';
import type { Theme } from './style';

export const POI_SOURCE = 'nautik-poi';

type Properties = { kind: PoiKind; category: string; label: string; osmRef: string };
export type PoiFeatures = FeatureCollection<Point, Properties>;

export function emptyPoiCollection(): PoiFeatures {
  return { type: 'FeatureCollection', features: [] };
}

/** Texte affiché : le nom, sinon (feux, balises) leurs caractéristiques. */
function poiLabel(poi: Poi): string {
  return labelText(poi.name ?? poi.details ?? '');
}

export function poiData(pois: Poi[]): PoiFeatures {
  return {
    type: 'FeatureCollection',
    features: pois.map((p): Feature<Point, Properties> => ({
      type: 'Feature',
      geometry: { type: 'Point', coordinates: [p.position.longitudeDeg, p.position.latitudeDeg] },
      properties: {
        kind: p.kind,
        category: p.category ?? '',
        label: poiLabel(p),
        osmRef: p.osmRef,
      },
    })),
  };
}

const kindIs = (...kinds: PoiKind[]): ExpressionSpecification => [
  'in',
  ['get', 'kind'],
  ['literal', kinds],
];
const hasLabel: ExpressionSpecification = ['!=', ['get', 'label'], ''];

/** Couleur d'une balise ou bouée selon sa catégorie (IALA, simplifié). */
const markColor: DataDrivenPropertyValueSpecification<string> = [
  'case',
  ['in', 'port', ['get', 'category']],
  '#d32f2f',
  ['in', 'starboard', ['get', 'category']],
  '#2e7d32',
  [
    'any',
    ['in', 'cardinal', ['get', 'category']],
    ['in', 'north', ['get', 'category']],
    ['in', 'south', ['get', 'category']],
    ['in', 'east', ['get', 'category']],
    ['in', 'west', ['get', 'category']],
    ['in', 'special', ['get', 'category']],
  ],
  '#fbc02d',
  ['in', 'isolated', ['get', 'category']],
  '#212121',
  ['in', 'safe', ['get', 'category']],
  '#e53935',
  '#78909c',
];

/** Couches d'étiquettes : identifiant, style, natures et zooms. */
const LABEL_LAYERS: {
  id: string;
  style: LabelStyle;
  kinds: PoiKind[];
  min: number;
  max?: number;
}[] = [
  { id: 'nautik-poi-archipelago-labels', style: 'island', kinds: ['archipelago'], min: 4, max: 9 },
  { id: 'nautik-poi-island-labels', style: 'island', kinds: ['island'], min: 7 },
  { id: 'nautik-poi-islet-labels', style: 'island', kinds: ['islet'], min: 11 },
  { id: 'nautik-poi-pass-labels', style: 'mark', kinds: ['pass', 'harbour'], min: 10 },
  {
    id: 'nautik-poi-mark-labels',
    style: 'mark',
    kinds: ['light', 'beacon', 'buoy', 'landmark', 'anchorage'],
    min: 13,
  },
];

/** Changements d'images d'étiquettes lors d'un changement de thème. */
export function poiThemeChanges(theme: Theme): [string, ExpressionSpecification][] {
  return LABEL_LAYERS.map((l) => [l.id, labelImageExpression(theme, l.style)]);
}

/** Couches des îles et amers, de bas en haut. */
export function poiLayers(theme: Theme = 'day'): LayerSpecification[] {
  const source = POI_SOURCE;
  const symbols: LayerSpecification[] = [
    {
      id: 'nautik-poi-anchorages',
      type: 'circle',
      source,
      minzoom: 10,
      filter: kindIs('anchorage', 'harbour'),
      paint: {
        'circle-radius': 5,
        'circle-color': '#1565c0',
        'circle-stroke-color': '#ffffff',
        'circle-stroke-width': 1.5,
      },
    },
    {
      id: 'nautik-poi-passes',
      type: 'circle',
      source,
      minzoom: 9,
      filter: kindIs('pass'),
      paint: {
        'circle-radius': 4,
        'circle-color': '#c2187a',
        'circle-stroke-color': '#ffffff',
        'circle-stroke-width': 1.5,
      },
    },
    {
      id: 'nautik-poi-marks',
      type: 'circle',
      source,
      minzoom: 11,
      filter: kindIs('beacon', 'buoy', 'landmark'),
      paint: {
        'circle-radius': ['match', ['get', 'kind'], 'buoy', 5, 4],
        'circle-color': markColor,
        'circle-stroke-color': ['match', ['get', 'kind'], 'landmark', '#212121', '#ffffff'],
        'circle-stroke-width': 1.5,
      },
    },
    {
      id: 'nautik-poi-lights',
      type: 'circle',
      source,
      minzoom: 9,
      filter: kindIs('light'),
      paint: {
        'circle-radius': 4,
        'circle-color': '#ffeb3b',
        'circle-stroke-color': '#c2187a',
        'circle-stroke-width': 2,
      },
    },
  ];
  const labels: LayerSpecification[] = LABEL_LAYERS.map((l) => ({
    id: l.id,
    type: 'symbol',
    source,
    minzoom: l.min,
    ...(l.max === undefined ? {} : { maxzoom: l.max }),
    filter: ['all', kindIs(...l.kinds), hasLabel],
    layout: {
      'icon-image': labelImageExpression(theme, l.style),
      // Îles centrées sur leur position, amers à droite du symbole.
      'icon-anchor': l.style === 'island' ? 'center' : 'left',
      'icon-offset': l.style === 'island' ? [0, 0] : [7, 0],
      'icon-allow-overlap': false,
      'symbol-sort-key': ['match', ['get', 'kind'], 'archipelago', 0, 'island', 1, 'pass', 2, 3],
    },
  }));
  return [...symbols, ...labels];
}
