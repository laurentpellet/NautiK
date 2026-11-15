/**
 * Couche de l'outil de mesure : segments en tirets, points cliqués et
 * longueur de chaque segment au milieu du segment. Les distances
 * affichées viennent du moteur (déjà formatées) ; cette couche ne dessine
 * que la géométrie.
 */
import type { FeatureCollection, LineString, Point } from 'geojson';
import type { ExpressionSpecification, LayerSpecification } from 'maplibre-gl';
import type { LatLon } from '../services/routing';
import { labelImageExpression, labelText } from './labels';
import type { Theme } from './style';

export const MEASURE_SOURCE = 'nautik-measure';
export const MEASURE_LEG_LABELS_LAYER = 'nautik-measure-leg-labels';

type Properties = Record<string, number | string>;
export type MeasureFeatures = FeatureCollection<LineString | Point, Properties>;

const toPosition = (p: LatLon): [number, number] => [p.longitudeDeg, p.latitudeDeg];

/**
 * Données de la mesure. `legLabels` : longueurs formatées des segments,
 * ignorées si leur nombre ne correspond pas aux segments (calcul en cours).
 */
export function measureData(points: LatLon[], legLabels: string[] | null = null): MeasureFeatures {
  const features: MeasureFeatures['features'] = points.map((p, index) => ({
    type: 'Feature',
    geometry: { type: 'Point', coordinates: toPosition(p) },
    properties: { index },
  }));
  if (points.length >= 2) {
    features.unshift({
      type: 'Feature',
      geometry: { type: 'LineString', coordinates: points.map(toPosition) },
      properties: {},
    });
  }
  if (legLabels && legLabels.length === points.length - 1) {
    legLabels.forEach((text, leg) => {
      const from = points[leg];
      const to = points[leg + 1];
      if (!from || !to) return;
      features.push({
        type: 'Feature',
        geometry: { type: 'LineString', coordinates: [toPosition(from), toPosition(to)] },
        properties: { leg, label: labelText(text) },
      });
    });
  }
  return { type: 'FeatureCollection', features };
}

const isLine: ExpressionSpecification = ['==', ['geometry-type'], 'LineString'];
const isLeg: ExpressionSpecification = ['has', 'leg'];

/** Couches de la mesure, au-dessus des routes. */
export function measureLayers(theme: Theme = 'day'): LayerSpecification[] {
  return [
    {
      id: 'nautik-measure-casing',
      type: 'line',
      source: MEASURE_SOURCE,
      filter: ['all', isLine, ['!', isLeg]],
      paint: { 'line-color': '#ffffff', 'line-width': 5, 'line-opacity': 0.8 },
    },
    {
      id: 'nautik-measure-line',
      type: 'line',
      source: MEASURE_SOURCE,
      filter: ['all', isLine, ['!', isLeg]],
      paint: { 'line-color': '#00838f', 'line-width': 2.5, 'line-dasharray': [3, 2] },
    },
    {
      id: 'nautik-measure-points',
      type: 'circle',
      source: MEASURE_SOURCE,
      filter: ['==', ['geometry-type'], 'Point'],
      paint: {
        'circle-radius': 4.5,
        'circle-color': '#ffffff',
        'circle-stroke-color': '#00838f',
        'circle-stroke-width': 2.5,
      },
    },
    {
      // Longueur de chaque segment, en italique au milieu du segment.
      id: MEASURE_LEG_LABELS_LAYER,
      type: 'symbol',
      source: MEASURE_SOURCE,
      filter: ['all', isLine, isLeg],
      layout: {
        'symbol-placement': 'line-center',
        'icon-image': labelImageExpression(theme, 'leg'),
        'icon-rotation-alignment': 'viewport',
        'icon-allow-overlap': false,
      },
    },
  ];
}

/** Images d'étiquettes de la mesure à changer avec le thème. */
export function measureThemeChanges(theme: Theme): [string, ExpressionSpecification][] {
  return [[MEASURE_LEG_LABELS_LAYER, labelImageExpression(theme, 'leg')]];
}
