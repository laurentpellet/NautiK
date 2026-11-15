/**
 * Couche des marques personnelles : pastille colorée selon l'icône et
 * nom à droite. Fonctions pures ; ChartView les applique.
 */
import type { Feature, FeatureCollection, Point } from 'geojson';
import type {
  DataDrivenPropertyValueSpecification,
  ExpressionSpecification,
  LayerSpecification,
} from 'maplibre-gl';
import { MARK_ICON_COLORS, MARK_ICONS, type Mark, type MarkIcon } from '../services/marks';
import { labelImageExpression, labelText } from './labels';
import type { Theme } from './style';

export const MARKS_SOURCE = 'nautik-marks';
export const MARKS_LAYER = 'nautik-marks';
export const MARKS_LABEL_LAYER = 'nautik-marks-labels';

type Properties = { id: string; icon: MarkIcon; label: string };
export type MarkFeatures = FeatureCollection<Point, Properties>;

export function marksData(marks: Mark[]): MarkFeatures {
  return {
    type: 'FeatureCollection',
    features: marks.map((m): Feature<Point, Properties> => ({
      type: 'Feature',
      geometry: {
        type: 'Point',
        coordinates: [m.position.longitudeDeg, m.position.latitudeDeg],
      },
      properties: { id: m.id, icon: m.icon, label: labelText(m.name) },
    })),
  };
}

const iconColor = [
  'match',
  ['get', 'icon'],
  ...MARK_ICONS.flatMap((icon) => [icon, MARK_ICON_COLORS[icon]]),
  MARK_ICON_COLORS.flag,
] as unknown as DataDrivenPropertyValueSpecification<string>;

/** Changement d'image d'étiquette lors d'un changement de thème. */
export function marksThemeChanges(theme: Theme): [string, ExpressionSpecification][] {
  return [[MARKS_LABEL_LAYER, labelImageExpression(theme, 'mark')]];
}

export function marksLayers(theme: Theme = 'day'): LayerSpecification[] {
  return [
    {
      id: MARKS_LAYER,
      type: 'circle',
      source: MARKS_SOURCE,
      paint: {
        'circle-radius': ['match', ['get', 'icon'], 'danger', 7, 6],
        'circle-color': iconColor,
        'circle-stroke-color': '#ffffff',
        'circle-stroke-width': 2,
      },
    },
    {
      id: MARKS_LABEL_LAYER,
      type: 'symbol',
      source: MARKS_SOURCE,
      minzoom: 9,
      filter: ['!=', ['get', 'label'], ''],
      layout: {
        'icon-image': labelImageExpression(theme, 'mark'),
        'icon-anchor': 'left',
        'icon-offset': [9, 0],
        'icon-allow-overlap': false,
      },
    },
  ];
}
