/**
 * Mouillage sur la carte : cercle d'évitage, limite de retour
 * (hystérésis) et position de l'ancre. Fonctions pures, tracé seulement :
 * la distance et l'alarme sont calculées par le moteur.
 */
import type { Feature, FeatureCollection, Point, Polygon, Position } from 'geojson';
import type { LayerSpecification } from 'maplibre-gl';
import type { AnchorView } from '../services/alarms';

export const ANCHOR_SOURCE = 'nautik-anchor';

/** Rayon terrestre moyen (m), suffisant pour le tracé d'un cercle. */
const EARTH_RADIUS_M = 6_371_008.8;
/** Nombre de sommets du cercle tracé. */
export const CIRCLE_STEPS = 72;

type Properties = { role: 'circle' | 'limit' | 'anchor'; alerting: boolean };
type Collection = FeatureCollection<Polygon | Point, Properties>;

/** Point à `distance` m dans la direction `bearing` (rad), sphère. */
function destination(lat: number, lon: number, bearing: number, distance: number): Position {
  const lat1 = (lat * Math.PI) / 180;
  const lon1 = (lon * Math.PI) / 180;
  const angle = distance / EARTH_RADIUS_M;
  const lat2 = Math.asin(
    Math.sin(lat1) * Math.cos(angle) + Math.cos(lat1) * Math.sin(angle) * Math.cos(bearing),
  );
  const lon2 =
    lon1 +
    Math.atan2(
      Math.sin(bearing) * Math.sin(angle) * Math.cos(lat1),
      Math.cos(angle) - Math.sin(lat1) * Math.sin(lat2),
    );
  // Longitude continue autour du centre (antiméridien sans saut).
  return [(lon2 * 180) / Math.PI, (lat2 * 180) / Math.PI];
}

/** Cercle (polygone fermé) de rayon `radiusM` autour de (lat, lon). */
export function circle(lat: number, lon: number, radiusM: number): Position[] {
  const ring: Position[] = [];
  for (let i = 0; i < CIRCLE_STEPS; i++) {
    ring.push(destination(lat, lon, (2 * Math.PI * i) / CIRCLE_STEPS, radiusM));
  }
  const first = ring[0];
  if (first) ring.push(first);
  return ring;
}

/** Données GeoJSON du mouillage surveillé (vide sans mouillage). */
export function anchorData(anchor: AnchorView | null, alerting: boolean): Collection {
  if (!anchor) return { type: 'FeatureCollection', features: [] };
  const { latitudeDeg: lat, longitudeDeg: lon } = anchor.position;
  const polygon = (radius: number, role: Properties['role']): Feature<Polygon, Properties> => ({
    type: 'Feature',
    geometry: { type: 'Polygon', coordinates: [circle(lat, lon, radius)] },
    properties: { role, alerting },
  });
  return {
    type: 'FeatureCollection',
    features: [
      polygon(anchor.radiusM, 'circle'),
      polygon(anchor.radiusM - anchor.hysteresisM, 'limit'),
      {
        type: 'Feature',
        geometry: { type: 'Point', coordinates: [lon, lat] },
        properties: { role: 'anchor', alerting },
      },
    ],
  };
}

const ALERT = '#e53935';
const NORMAL = '#26a69a';

/** Couches : disque, contour, limite de retour en pointillés, ancre. */
export function anchorLayers(): LayerSpecification[] {
  const color = ['case', ['get', 'alerting'], ALERT, NORMAL] as never;
  return [
    {
      id: 'nautik-anchor-fill',
      type: 'fill',
      source: ANCHOR_SOURCE,
      filter: ['==', ['get', 'role'], 'circle'],
      paint: { 'fill-color': color, 'fill-opacity': 0.12 },
    },
    {
      id: 'nautik-anchor-circle',
      type: 'line',
      source: ANCHOR_SOURCE,
      filter: ['==', ['get', 'role'], 'circle'],
      paint: { 'line-color': color, 'line-width': 2.5 },
    },
    {
      id: 'nautik-anchor-limit',
      type: 'line',
      source: ANCHOR_SOURCE,
      filter: ['==', ['get', 'role'], 'limit'],
      paint: { 'line-color': color, 'line-width': 1, 'line-dasharray': [2, 2] },
    },
    {
      id: 'nautik-anchor-point',
      type: 'circle',
      source: ANCHOR_SOURCE,
      filter: ['==', ['get', 'role'], 'anchor'],
      paint: {
        'circle-radius': 6,
        'circle-color': '#ffffff',
        'circle-stroke-color': color,
        'circle-stroke-width': 3,
      },
    },
  ];
}
