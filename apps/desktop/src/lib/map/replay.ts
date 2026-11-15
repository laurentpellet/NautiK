/**
 * Couche de relecture d'une trace : segments colorés selon la vitesse
 * enregistrée (moteur) et position du curseur. Un segment sans vitesse
 * connue est gris : aucune vitesse n'est inventée. Fonctions pures.
 */
import type { Feature, FeatureCollection, LineString, Point, Position } from 'geojson';
import type { DataDrivenPropertyValueSpecification, LayerSpecification } from 'maplibre-gl';
import type { ProfilePoint } from '../services/trackProfile';
import type { LatLon } from '../services/validate';

export const REPLAY_SOURCE = 'nautik-replay';

/** Paliers de couleur (nœuds) : arrêt, petit temps, croisière, rapide. */
export const SPEED_STOPS_KN: [number, string][] = [
  [0, '#1565c0'],
  [3, '#00acc1'],
  [5, '#43a047'],
  [7, '#fdd835'],
  [9, '#e53935'],
];
const UNKNOWN_SPEED_COLOR = '#9e9e9e';
const KN = 1852 / 3600;

type Properties = { kind: 'segment' | 'cursor'; speedKn: number };
export type ReplayFeatures = FeatureCollection<LineString | Point, Properties>;

const toPosition = (p: LatLon): Position => [p.longitudeDeg, p.latitudeDeg];

/** Longitude rapprochée de la précédente (trace qui franchit l'antiméridien). */
function unwrap(previous: Position, next: Position): Position {
  const lon0 = previous[0] ?? 0;
  let lon = next[0] ?? 0;
  while (lon - lon0 > 180) lon -= 360;
  while (lon - lon0 < -180) lon += 360;
  return [lon, next[1] ?? 0];
}

/**
 * Segments de la trace (vitesse du point d'arrivée, en nœuds ; −1 si
 * inconnue) et curseur au point `index`.
 */
export function replayData(points: ProfilePoint[], index: number | null): ReplayFeatures {
  const features: ReplayFeatures['features'] = [];
  for (let i = 1; i < points.length; i += 1) {
    const a = points[i - 1];
    const b = points[i];
    if (!a || !b) continue;
    const start = toPosition(a.position);
    features.push({
      type: 'Feature',
      geometry: { type: 'LineString', coordinates: [start, unwrap(start, toPosition(b.position))] },
      properties: { kind: 'segment', speedKn: b.speedMps === null ? -1 : b.speedMps / KN },
    } satisfies Feature<LineString, Properties>);
  }
  const cursor = index === null ? undefined : points[index];
  if (cursor) {
    features.push({
      type: 'Feature',
      geometry: { type: 'Point', coordinates: toPosition(cursor.position) },
      properties: { kind: 'cursor', speedKn: cursor.speedMps === null ? -1 : cursor.speedMps / KN },
    });
  }
  return { type: 'FeatureCollection', features };
}

const speedColor = [
  'case',
  ['<', ['get', 'speedKn'], 0],
  UNKNOWN_SPEED_COLOR,
  ['interpolate', ['linear'], ['get', 'speedKn'], ...SPEED_STOPS_KN.flat()],
] as unknown as DataDrivenPropertyValueSpecification<string>;

export function replayLayers(): LayerSpecification[] {
  return [
    {
      id: 'nautik-replay-segments',
      type: 'line',
      source: REPLAY_SOURCE,
      filter: ['==', ['get', 'kind'], 'segment'],
      layout: { 'line-cap': 'round', 'line-join': 'round' },
      paint: { 'line-color': speedColor, 'line-width': 4 },
    },
    {
      id: 'nautik-replay-cursor',
      type: 'circle',
      source: REPLAY_SOURCE,
      filter: ['==', ['get', 'kind'], 'cursor'],
      paint: {
        'circle-radius': 8,
        'circle-color': '#ffffff',
        'circle-stroke-color': '#212121',
        'circle-stroke-width': 3,
      },
    },
  ];
}
