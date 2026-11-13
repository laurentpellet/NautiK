/**
 * Couche vent (prévision GRIB) : flèches orientées dans le sens où
 * souffle le vent, colorées par la force. Fonctions pures.
 */
import type { FeatureCollection, Point } from 'geojson';
import type { LayerSpecification } from 'maplibre-gl';
import type { Bounds, WindVector } from '../services/weather';
import { polygonDistance, sdfIcon, ICON_SIZE, type IconImage, type Point2 } from './ais';

export const WIND_SOURCE = 'nautik-wind';
export const WIND_LAYER = 'nautik-wind-arrows';
export const WIND_ICON = 'nautik-wind-arrow';

const KNOTS_PER_MPS = 3600 / 1852;

/** Échelle de couleurs (nœuds), aussi utilisée par la légende. */
export const WIND_SCALE: readonly [number, string][] = [
  [0, '#4fc3f7'],
  [10, '#66bb6a'],
  [17, '#fdd835'],
  [25, '#fb8c00'],
  [34, '#e53935'],
  [48, '#ab47bc'],
];

type Collection = FeatureCollection<Point, { speedKn: number; rotation: number }>;

/** Données GeoJSON : une flèche par point (vers où souffle le vent). */
export function windData(points: WindVector[] | null): Collection {
  return {
    type: 'FeatureCollection',
    features: (points ?? []).map((p) => ({
      type: 'Feature',
      geometry: { type: 'Point', coordinates: [p.lon, p.lat] },
      properties: {
        speedKn: Math.round(p.speedMps * KNOTS_PER_MPS * 10) / 10,
        rotation: ((p.directionRad * 180) / Math.PI + 180) % 360,
      },
    })),
  };
}

/** Couche des flèches. */
export function windLayers(): LayerSpecification[] {
  const color: unknown[] = ['interpolate', ['linear'], ['get', 'speedKn']];
  for (const [kn, c] of WIND_SCALE) color.push(kn, c);
  return [
    {
      id: WIND_LAYER,
      type: 'symbol',
      source: WIND_SOURCE,
      layout: {
        'icon-image': WIND_ICON,
        'icon-rotate': ['get', 'rotation'],
        'icon-rotation-alignment': 'map',
        'icon-size': ['interpolate', ['linear'], ['get', 'speedKn'], 0, 0.55, 30, 1.05],
        'icon-allow-overlap': true,
        'icon-ignore-placement': true,
      },
      paint: {
        'icon-color': color as never,
        'icon-opacity': 0.9,
        'icon-halo-color': '#0b1424',
        'icon-halo-width': 1,
      },
    },
  ];
}

/** Icône flèche pointant vers le nord. */
export function windIcon(): { id: string; image: IconImage } {
  const c = ICON_SIZE / 2;
  const arrow: Point2[] = [
    [c, c - 12],
    [c + 6, c - 3],
    [c + 2, c - 3],
    [c + 2, c + 12],
    [c - 2, c + 12],
    [c - 2, c - 3],
    [c - 6, c - 3],
  ];
  return { id: WIND_ICON, image: sdfIcon(polygonDistance(arrow)) };
}

const wrap = (lon: number) => ((((lon + 180) % 360) + 360) % 360) - 180;

/**
 * Emprise affichée normalisée : longitudes dans [-180, 180] (ouest >
 * est si l'antiméridien est traversé), latitudes bornées.
 */
export function viewBounds(west: number, south: number, east: number, north: number): Bounds {
  const s = Math.max(-90, Math.min(90, south));
  const n = Math.max(-90, Math.min(90, north));
  if (east - west >= 360) return { west: -180, south: s, east: 180, north: n };
  let w = wrap(west);
  let e = wrap(east);
  if (w === e) {
    w = -180;
    e = 180;
  }
  return { west: w, south: s, east: e, north: n };
}
