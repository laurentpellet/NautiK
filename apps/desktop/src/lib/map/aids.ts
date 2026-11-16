/**
 * Aides à la navigation sur la carte : vecteur de route et ligne de cap
 * du bateau, laylines du waypoint actif, ligne vers l'homme à la mer,
 * cercle de distance (VRM) et ligne de relèvement (EBL), lignes de
 * dérive (route fond et ligne de foi) graduées chaque minute.
 *
 * Fonctions pures, tracé seulement : vecteurs, laylines et distances
 * sont calculés par le moteur ; le VRM et l'EBL sont des repères choisis
 * par l'utilisateur, dessinés autour du bateau.
 */
import type { Feature, FeatureCollection, LineString, Point, Polygon, Position } from 'geojson';
import type { LayerSpecification } from 'maplibre-gl';
import type { OwnShip } from '../services/ownShip';
import type { LatLon } from '../services/validate';
import { circle, destination } from './anchor';
import { ICON_SIZE, polygonDistance, sdfIcon, type IconImage } from './ais';

export const AIDS_SOURCE = 'nautik-aids';

export type AidKind =
  | 'vector'
  | 'heading'
  | 'layline'
  | 'mobLine'
  | 'vrm'
  | 'ebl'
  | 'driftCog'
  | 'driftHeading'
  | 'driftTick';
type Properties = {
  kind: AidKind;
  upwind?: boolean;
  /** Graduation : ligne portée (route fond ou ligne de foi). */
  line?: 'cog' | 'heading';
  /** Graduation : orientation de la ligne (degrés vrais). */
  bearing?: number;
  /** Graduation : minutes de projection. */
  minute?: number;
  /** Graduation de bout de ligne. */
  last?: boolean;
};
type Collection = FeatureCollection<LineString | Polygon | Point, Properties>;

/** Graduation des lignes de dérive (icône SDF, trait perpendiculaire). */
export const DRIFT_TICK_ICON = 'nautik-drift-tick';

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
  /** Durée de projection des lignes de dérive (min) ; `null` : masquées. */
  driftMinutes?: number | null;
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
  const drift = own?.drift ?? null;
  const minutes = input.driftMinutes ?? null;
  if (boat && drift && minutes !== null && minutes >= 1) {
    features.push(...driftFeatures(boat, drift, Math.floor(minutes)));
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

/**
 * Route fond et ligne de foi, de la longueur parcourue à la vitesse fond
 * pendant `minutes`, avec une graduation par minute.
 */
function driftFeatures(
  boat: LatLon,
  drift: NonNullable<OwnShip['drift']>,
  minutes: number,
): Collection['features'] {
  const { latitudeDeg: lat, longitudeDeg: lon } = boat;
  const start: Position = [lon, lat];
  const perMinute = drift.sogMps * 60;
  const features: Collection['features'] = [];
  const lines = [
    { line: 'heading', kind: 'driftHeading', bearing: drift.headingRad },
    { line: 'cog', kind: 'driftCog', bearing: drift.cogRad },
  ] as const;
  for (const { line, kind, bearing } of lines) {
    const end = near(start, destination(lat, lon, bearing, perMinute * minutes));
    features.push({
      type: 'Feature',
      geometry: { type: 'LineString', coordinates: [start, end] },
      properties: { kind },
    });
    const degrees = (bearing * 180) / Math.PI;
    for (let minute = 1; minute <= minutes; minute++) {
      features.push({
        type: 'Feature',
        geometry: {
          type: 'Point',
          coordinates: near(start, destination(lat, lon, bearing, perMinute * minute)),
        },
        properties: { kind: 'driftTick', line, bearing: degrees, minute, last: minute === minutes },
      });
    }
  }
  return features;
}

/** Couleurs des lignes de dérive : route fond et ligne de foi. */
export const DRIFT_COG_COLOR = '#d81b60';
export const DRIFT_HEADING_COLOR = '#00897b';

/** Icônes des aides (`addImage(id, image, { sdf: true })`). */
export function aidsIcons(): { id: string; image: IconImage }[] {
  const c = ICON_SIZE / 2;
  // Trait horizontal : perpendiculaire à une ligne orientée au nord.
  const bar = polygonDistance([
    [c - 9, c - 1.25],
    [c + 9, c - 1.25],
    [c + 9, c + 1.25],
    [c - 9, c + 1.25],
  ]);
  return [{ id: DRIFT_TICK_ICON, image: sdfIcon(bar) }];
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
      id: 'nautik-aids-drift-heading',
      type: 'line',
      source: AIDS_SOURCE,
      filter: is('driftHeading'),
      layout: { 'line-cap': 'round' },
      paint: { 'line-color': DRIFT_HEADING_COLOR, 'line-width': 2 },
    },
    {
      id: 'nautik-aids-drift-cog',
      type: 'line',
      source: AIDS_SOURCE,
      filter: is('driftCog'),
      layout: { 'line-cap': 'round' },
      paint: { 'line-color': DRIFT_COG_COLOR, 'line-width': 2.5 },
    },
    {
      id: 'nautik-aids-drift-ticks',
      type: 'symbol',
      source: AIDS_SOURCE,
      filter: is('driftTick'),
      layout: {
        'icon-image': DRIFT_TICK_ICON,
        'icon-rotate': ['get', 'bearing'] as never,
        'icon-rotation-alignment': 'map',
        'icon-size': ['case', ['get', 'last'], 1, 0.6] as never,
        'icon-allow-overlap': true,
        'icon-ignore-placement': true,
      },
      paint: {
        'icon-color': [
          'case',
          ['==', ['get', 'line'], 'cog'],
          DRIFT_COG_COLOR,
          DRIFT_HEADING_COLOR,
        ] as never,
      },
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
