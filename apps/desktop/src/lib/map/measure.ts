/**
 * Couche de l'outil de mesure : segments en tirets et points cliqués.
 * Les distances et relèvements affichés viennent du moteur ; cette couche
 * ne dessine que la géométrie.
 */
import type { FeatureCollection, LineString, Point } from 'geojson';
import type { LayerSpecification } from 'maplibre-gl';
import type { LatLon } from '../services/routing';

export const MEASURE_SOURCE = 'nautik-measure';

type Properties = Record<string, number>;
export type MeasureFeatures = FeatureCollection<LineString | Point, Properties>;

const toPosition = (p: LatLon): [number, number] => [p.longitudeDeg, p.latitudeDeg];

export function measureData(points: LatLon[]): MeasureFeatures {
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
  return { type: 'FeatureCollection', features };
}

/** Couches de la mesure, au-dessus des routes. */
export function measureLayers(): LayerSpecification[] {
  return [
    {
      id: 'nautik-measure-casing',
      type: 'line',
      source: MEASURE_SOURCE,
      filter: ['==', ['geometry-type'], 'LineString'],
      paint: { 'line-color': '#ffffff', 'line-width': 5, 'line-opacity': 0.8 },
    },
    {
      id: 'nautik-measure-line',
      type: 'line',
      source: MEASURE_SOURCE,
      filter: ['==', ['geometry-type'], 'LineString'],
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
  ];
}
