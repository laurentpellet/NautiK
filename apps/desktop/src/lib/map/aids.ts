/**
 * Aides à la navigation sur la carte : vecteur de route et ligne de cap
 * du bateau, laylines du waypoint actif, ligne vers l'homme à la mer,
 * cercle de distance (VRM) et ligne de relèvement (EBL).
 *
 * Fonctions pures, tracé seulement : vecteurs, laylines et distances
 * sont calculés par le moteur ; le VRM et l'EBL sont des repères choisis
 * par l'utilisateur, dessinés autour du bateau.
 */
import type { Feature, FeatureCollection, LineString, Polygon, Position } from 'geojson';
import type { LayerSpecification } from 'maplibre-gl';
import type { OwnShip } from '../services/ownShip';
import type { LatLon } from '../services/validate';
import { circle, destination } from './anchor';

export const AIDS_SOURCE = 'nautik-aids';

export type AidKind = 'vector' | 'heading' | 'layline' | 'mobLine' | 'vrm' | 'ebl';
type Properties = { kind: AidKind; upwind?: boolean };
type Collection = FeatureCollection<LineString | Polygon, Properties>;

/** Repère VRM/EBL autour du bateau. */
export interface RangeBearing {
  /** Rayon du cercle de distance (m) ; `null` : pas de cercle. */
  radiusM: number | null;
  /** Relèvement vrai de la ligne (rad) ; `null` : pas de ligne. */
  bearingRad: number | null;
}

/** Longueur de la ligne de relèvement sans cercle (m) : 12 milles. */
export const EBL_LENGTH_M = 12 * 1852;

export interface AidsInput {
  own: OwnShip | null;
  vectors: boolean;
  laylines: boolean;
  /** Bateau (position fraîche) et point MOB : ligne de ralliement. */
  mob: LatLon | null;
  /** Repère VRM/EBL, centré sur la position fraîche du bateau. */
  rangeBearing: RangeBearing | null;
}

const toPosition = (p: LatLon): Position => [p.longitudeDeg, p.latitudeDeg];

/** Longitude de `b` déroulée près de celle de `a` (antiméridien). */
function near(a: Position, b: Position): Position {
  const [ax] = a;
  let [bx] = b;
  const by = b[1] ?? 0;
  if (ax === undefined || bx === undefined) return b;
  while (bx - ax > 180) bx -= 360;
  while (ax - bx > 180) bx += 360;
  return [bx, by];
}

function line(a: LatLon, b: LatLon, properties: Properties): Feature<LineString, Properties> {
  const start = toPosition(a);
  return {
    type: 'Feature',
    geometry: { type: 'LineString', coordinates: [start, near(start, toPosition(b))] },
    properties,
  };
}

/** Données GeoJSON des aides à la navigation. */
export function aidsData(input: AidsInput): Collection {
  const features: Collection['features'] = [];
  const own = input.own;
  const boat = own?.position ?? null;
  if (own && boat && input.vectors) {
    if (own.headingEnd) features.push(line(boat, own.headingEnd, { kind: 'heading' }));
    if (own.vectorEnd) features.push(line(boat, own.vectorEnd, { kind: 'vector' }));
  }
  if (own?.laylines && input.laylines) {
    const l = own.laylines;
    features.push(line(l.mark, l.starboardEnd, { kind: 'layline', upwind: l.upwind }));
    features.push(line(l.mark, l.portEnd, { kind: 'layline', upwind: l.upwind }));
  }
  if (boat && input.mob) features.push(line(boat, input.mob, { kind: 'mobLine' }));
  const rb = input.rangeBearing;
  if (boat && rb) {
    const { latitudeDeg: lat, longitudeDeg: lon } = boat;
    if (rb.radiusM !== null && rb.radiusM > 0) {
      features.push({
        type: 'Feature',
        geometry: { type: 'Polygon', coordinates: [circle(lat, lon, rb.radiusM)] },
        properties: { kind: 'vrm' },
      });
    }
    if (rb.bearingRad !== null) {
      const length = rb.radiusM !== null && rb.radiusM > 0 ? rb.radiusM * 1.5 : EBL_LENGTH_M;
      const end = destination(lat, lon, rb.bearingRad, length);
      features.push({
        type: 'Feature',
        geometry: { type: 'LineString', coordinates: [[lon, lat], end] },
        properties: { kind: 'ebl' },
      });
    }
  }
  return { type: 'FeatureCollection', features };
}

const is = (kind: AidKind) => ['==', ['get', 'kind'], kind] as never;

/** Couches des aides, dans l'ordre d'empilement. */
export function aidsLayers(): LayerSpecification[] {
  return [
    {
      id: 'nautik-aids-vrm',
      type: 'line',
      source: AIDS_SOURCE,
      filter: is('vrm'),
      paint: { 'line-color': '#ff6f00', 'line-width': 2, 'line-dasharray': [4, 2] },
    },
    {
      id: 'nautik-aids-ebl',
      type: 'line',
      source: AIDS_SOURCE,
      filter: is('ebl'),
      paint: { 'line-color': '#ff6f00', 'line-width': 2 },
    },
    {
      id: 'nautik-aids-laylines',
      type: 'line',
      source: AIDS_SOURCE,
      filter: is('layline'),
      paint: {
        'line-color': ['case', ['get', 'upwind'], '#2e7d32', '#1565c0'] as never,
        'line-width': 2,
        'line-dasharray': [3, 2],
      },
    },
    {
      id: 'nautik-aids-heading',
      type: 'line',
      source: AIDS_SOURCE,
      filter: is('heading'),
      paint: { 'line-color': '#d81b60', 'line-width': 1.5, 'line-dasharray': [1, 1.5] },
    },
    {
      id: 'nautik-aids-vector',
      type: 'line',
      source: AIDS_SOURCE,
      filter: is('vector'),
      layout: { 'line-cap': 'round' },
      paint: { 'line-color': '#d81b60', 'line-width': 3 },
    },
    {
      id: 'nautik-aids-mob',
      type: 'line',
      source: AIDS_SOURCE,
      filter: is('mobLine'),
      paint: { 'line-color': '#e53935', 'line-width': 3, 'line-dasharray': [2, 1] },
    },
  ];
}
