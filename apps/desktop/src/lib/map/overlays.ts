/**
 * Couches de surimpression : routes, route en cours de tracé, traces.
 *
 * Fonctions pures (GeoJSON et spécifications de couches) : ChartView se
 * contente de les appliquer. Le style étant entièrement local (pas de
 * glyphes), les waypoints sont des cercles ; leurs noms figurent dans le
 * panneau des routes.
 */
import type { Feature, FeatureCollection, LineString, Point, Position } from 'geojson';
import type { LayerSpecification } from 'maplibre-gl';
import type { LatLon, Route } from '../services/routing';

export const ROUTES_SOURCE = 'nautik-routes';
export const DRAFT_SOURCE = 'nautik-draft';
export const TRACKS_SOURCE = 'nautik-tracks';

type Properties = Record<string, string | boolean | number>;
type Collection = FeatureCollection<LineString | Point, Properties>;

const toPosition = (p: LatLon): Position => [p.longitudeDeg, p.latitudeDeg];

function line(points: LatLon[], properties: Properties): Feature<LineString, Properties> {
  return {
    type: 'Feature',
    geometry: { type: 'LineString', coordinates: points.map(toPosition) },
    properties,
  };
}

function point(p: LatLon, properties: Properties): Feature<Point, Properties> {
  return { type: 'Feature', geometry: { type: 'Point', coordinates: toPosition(p) }, properties };
}

export function emptyCollection(): Collection {
  return { type: 'FeatureCollection', features: [] };
}

/** Routes enregistrées : un segment par route et un point par waypoint. */
export function routesData(routes: Route[], targetIndex: number | null = null): Collection {
  const features: Collection['features'] = [];
  for (const route of routes) {
    const positions = route.waypoints.map((w) => w.position);
    features.push(line(positions, { id: route.id, active: route.active }));
    route.waypoints.forEach((w, index) => {
      features.push(
        point(w.position, {
          id: w.id,
          routeId: route.id,
          active: route.active,
          target: route.active && index === targetIndex,
        }),
      );
    });
  }
  return { type: 'FeatureCollection', features };
}

/** Route en cours de tracé. */
export function draftData(points: LatLon[]): Collection {
  const features: Collection['features'] = points.map((p, index) => point(p, { index }));
  if (points.length >= 2) {
    features.unshift(line(points, {}));
  }
  return { type: 'FeatureCollection', features };
}

/** Traces affichées. */
export function tracksData(tracks: { id: string; points: LatLon[]; recording: boolean }[]) {
  const features: Collection['features'] = tracks
    .filter((t) => t.points.length >= 2)
    .map((t) => line(t.points, { id: t.id, recording: t.recording }));
  return { type: 'FeatureCollection', features } satisfies Collection;
}

/** Couches de surimpression, dans l'ordre d'empilement. */
export function overlayLayers(): LayerSpecification[] {
  return [
    {
      id: 'nautik-tracks-line',
      type: 'line',
      source: TRACKS_SOURCE,
      layout: { 'line-cap': 'round', 'line-join': 'round' },
      paint: {
        'line-color': ['case', ['get', 'recording'], '#2e7d32', '#1565c0'],
        'line-width': 2.5,
        'line-opacity': 0.85,
      },
    },
    {
      id: 'nautik-routes-line',
      type: 'line',
      source: ROUTES_SOURCE,
      filter: ['==', ['geometry-type'], 'LineString'],
      layout: { 'line-cap': 'round', 'line-join': 'round' },
      paint: {
        'line-color': ['case', ['get', 'active'], '#d81b60', '#7b1fa2'],
        'line-width': ['case', ['get', 'active'], 4, 2.5],
      },
    },
    {
      id: 'nautik-routes-points',
      type: 'circle',
      source: ROUTES_SOURCE,
      filter: ['==', ['geometry-type'], 'Point'],
      paint: {
        'circle-radius': ['case', ['get', 'target'], 8, 5],
        'circle-color': ['case', ['get', 'target'], '#ffeb3b', '#ffffff'],
        'circle-stroke-color': ['case', ['get', 'active'], '#d81b60', '#7b1fa2'],
        'circle-stroke-width': 2.5,
      },
    },
    {
      id: 'nautik-draft-line',
      type: 'line',
      source: DRAFT_SOURCE,
      filter: ['==', ['geometry-type'], 'LineString'],
      paint: { 'line-color': '#ff6f00', 'line-width': 3, 'line-dasharray': [2, 1.5] },
    },
    {
      id: 'nautik-draft-points',
      type: 'circle',
      source: DRAFT_SOURCE,
      filter: ['==', ['geometry-type'], 'Point'],
      paint: {
        'circle-radius': 5,
        'circle-color': '#ffffff',
        'circle-stroke-color': '#ff6f00',
        'circle-stroke-width': 2.5,
      },
    },
  ];
}
