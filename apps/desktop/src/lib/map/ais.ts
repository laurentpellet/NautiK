/**
 * Couche AIS : symboles des cibles, vecteurs de déplacement (COG/SOG)
 * et historique des positions.
 *
 * Fonctions pures : ChartView se contente de les appliquer. Les
 * symboles sont des icônes SDF générées ici (aucune ressource externe,
 * aucun glyphe), colorées par la carte selon la classe et l'état de la
 * cible :
 * - triangle orienté (cap, sinon route fond) ; cercle si aucune
 *   direction n'est connue ;
 * - classe A : vert d'eau, grand ; classe B : ambre, plus petit ;
 * - cible obsolète : estompée ; perdue : grise et très estompée ;
 * - ligne de cap (cap vrai déclaré seulement) et nom de la cible.
 */
import type { Feature, FeatureCollection, LineString, Point, Position } from 'geojson';
import type { ExpressionSpecification, LayerSpecification } from 'maplibre-gl';
import { targetLabel, type AisSnapshot, type AisTarget } from '../services/ais';
import type { LatLon } from '../services/validate';
import { labelImageExpression, labelText } from './labels';
import type { Theme } from './style';

export const AIS_SOURCE = 'nautik-ais';
export const AIS_SYMBOL_LAYER = 'nautik-ais-symbols';
export const AIS_LABEL_LAYER = 'nautik-ais-labels';
export const AIS_VESSEL_ICON = 'nautik-ais-vessel';
export const AIS_ROUND_ICON = 'nautik-ais-round';
export const AIS_HEADING_ICON = 'nautik-ais-heading';

export const AIS_COLORS = { A: '#00897b', B: '#f9a825', lost: '#9e9e9e' } as const;

type Properties = Record<string, string | boolean | number>;
type Collection = FeatureCollection<LineString | Point, Properties>;

const toPosition = (p: LatLon): Position => [p.longitudeDeg, p.latitudeDeg];

const RAD_TO_DEG = 180 / Math.PI;

/** Direction du symbole (degrés) : cap, sinon route fond. */
export function targetRotation(target: AisTarget): number | null {
  const angle = target.headingRad ?? target.cogRad;
  return angle === null ? null : angle * RAD_TO_DEG;
}

/** Données GeoJSON de la couche AIS. */
export function aisData(snapshot: AisSnapshot | null): Collection {
  const features: Collection['features'] = [];
  for (const target of snapshot?.targets ?? []) {
    const position = target.position;
    if (!position) continue;
    const history = [...target.history, position];
    if (history.length >= 2) {
      features.push(line(history, { kind: 'history', mmsi: target.mmsi }));
    }
    if (target.vectorEnd) {
      features.push(
        line([position, target.vectorEnd], {
          kind: 'vector',
          mmsi: target.mmsi,
          aisClass: target.class,
        }),
      );
    }
    const rotation = targetRotation(target);
    features.push({
      type: 'Feature',
      geometry: { type: 'Point', coordinates: toPosition(position) },
      properties: {
        kind: 'target',
        mmsi: target.mmsi,
        aisClass: target.class,
        state: target.state,
        directed: rotation !== null,
        rotation: rotation ?? 0,
        // Ligne de cap : seulement si le cap est déclaré (jamais déduit).
        hasHeading: target.headingRad !== null,
        heading: target.headingRad === null ? 0 : target.headingRad * RAD_TO_DEG,
        label: labelText(targetLabel(target)),
      },
    });
  }
  return { type: 'FeatureCollection', features };
}

function line(points: LatLon[], properties: Properties): Feature<LineString, Properties> {
  return {
    type: 'Feature',
    geometry: { type: 'LineString', coordinates: points.map(toPosition) },
    properties,
  };
}

const isTarget: ExpressionSpecification = ['==', ['get', 'kind'], 'target'];

const targetColor: ExpressionSpecification = [
  'case',
  ['==', ['get', 'state'], 'lost'],
  AIS_COLORS.lost,
  ['match', ['get', 'aisClass'], 'A', AIS_COLORS.A, AIS_COLORS.B],
];

const targetOpacity: ExpressionSpecification = [
  'match',
  ['get', 'state'],
  'active',
  1,
  'stale',
  0.55,
  0.4,
];

/** Couches AIS, dans l'ordre d'empilement ; noms au-dessus des symboles. */
export function aisLayers(theme: Theme = 'day'): LayerSpecification[] {
  return [
    {
      id: 'nautik-ais-history',
      type: 'line',
      source: AIS_SOURCE,
      filter: ['==', ['get', 'kind'], 'history'],
      layout: { 'line-cap': 'round', 'line-join': 'round' },
      paint: { 'line-color': '#78909c', 'line-width': 1.5, 'line-dasharray': [1, 2] },
    },
    {
      id: 'nautik-ais-vectors',
      type: 'line',
      source: AIS_SOURCE,
      filter: ['==', ['get', 'kind'], 'vector'],
      layout: { 'line-cap': 'round' },
      paint: {
        'line-color': ['match', ['get', 'aisClass'], 'A', AIS_COLORS.A, AIS_COLORS.B],
        'line-width': 2,
      },
    },
    {
      id: 'nautik-ais-heading',
      type: 'symbol',
      source: AIS_SOURCE,
      filter: ['all', isTarget, ['get', 'hasHeading']],
      layout: {
        'icon-image': AIS_HEADING_ICON,
        'icon-rotate': ['get', 'heading'],
        'icon-rotation-alignment': 'map',
        'icon-allow-overlap': true,
        'icon-ignore-placement': true,
      },
      paint: { 'icon-color': targetColor, 'icon-opacity': targetOpacity },
    },
    {
      id: AIS_SYMBOL_LAYER,
      type: 'symbol',
      source: AIS_SOURCE,
      filter: isTarget,
      layout: {
        'icon-image': ['case', ['get', 'directed'], AIS_VESSEL_ICON, AIS_ROUND_ICON],
        'icon-size': ['match', ['get', 'aisClass'], 'A', 1, 0.8],
        'icon-rotate': ['get', 'rotation'],
        'icon-rotation-alignment': 'map',
        'icon-allow-overlap': true,
        'icon-ignore-placement': true,
      },
      paint: {
        'icon-color': targetColor,
        'icon-opacity': targetOpacity,
        'icon-halo-color': '#212121',
        'icon-halo-width': 1.2,
      },
    },
    {
      // Nom (ou MMSI) à droite de la cible ; masqué s'il en chevauche un autre.
      id: AIS_LABEL_LAYER,
      type: 'symbol',
      source: AIS_SOURCE,
      filter: isTarget,
      minzoom: 8,
      layout: {
        'icon-image': labelImageExpression(theme, 'ais'),
        'icon-anchor': 'left',
        'icon-offset': [14, 0],
        'icon-optional': true,
        'icon-allow-overlap': false,
      },
      paint: { 'icon-opacity': targetOpacity },
    },
  ];
}

/** Images d'étiquettes AIS à changer avec le thème. */
export function aisThemeChanges(theme: Theme): [string, ExpressionSpecification][] {
  return [[AIS_LABEL_LAYER, labelImageExpression(theme, 'ais')]];
}

/** Image brute acceptée par `map.addImage`. */
export interface IconImage {
  width: number;
  height: number;
  data: Uint8Array;
}

/** Côté des icônes (pixels), marge du champ de distance comprise. */
export const ICON_SIZE = 48;

/** Côté de l'icône de la ligne de cap (pixels) : ligne du centre au bord. */
export const HEADING_ICON_SIZE = 96;

/** Rayon du champ de distance (convention MapLibre / TinySDF). */
const SDF_RADIUS = 8;

/**
 * Icône SDF : l'alpha encode la distance signée au contour
 * (`192` sur le contour, décroissant vers l'extérieur).
 */
export function sdfIcon(
  distance: (x: number, y: number) => number,
  size: number = ICON_SIZE,
): IconImage {
  const data = new Uint8Array(size * size * 4);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const d = distance(x + 0.5, y + 0.5);
      const alpha = Math.round(255 * Math.min(1, Math.max(0, 0.75 - d / SDF_RADIUS)));
      const i = (y * size + x) * 4;
      data[i] = 0;
      data[i + 1] = 0;
      data[i + 2] = 0;
      data[i + 3] = alpha;
    }
  }
  return { width: size, height: size, data };
}

export type Point2 = [number, number];

function segmentDistance(p: Point2, a: Point2, b: Point2): number {
  const [px, py] = p;
  const [ax, ay] = a;
  const [bx, by] = b;
  const dx = bx - ax;
  const dy = by - ay;
  const t = Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)));
  return Math.hypot(px - (ax + t * dx), py - (ay + t * dy));
}

function inside(p: Point2, polygon: Point2[]): boolean {
  let result = false;
  for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i++) {
    const [xi, yi] = polygon[i] as Point2;
    const [xj, yj] = polygon[j] as Point2;
    if (yi > p[1] !== yj > p[1] && p[0] < ((xj - xi) * (p[1] - yi)) / (yj - yi) + xi) {
      result = !result;
    }
  }
  return result;
}

/** Distance signée à un polygone (négative à l'intérieur). */
export function polygonDistance(polygon: Point2[]): (x: number, y: number) => number {
  return (x, y) => {
    const p: Point2 = [x, y];
    let d = Infinity;
    for (let i = 0; i < polygon.length; i++) {
      const a = polygon[i] as Point2;
      const b = polygon[(i + 1) % polygon.length] as Point2;
      d = Math.min(d, segmentDistance(p, a, b));
    }
    return inside(p, polygon) ? -d : d;
  };
}

/** Icônes AIS à enregistrer dans la carte (`addImage(id, image, { sdf: true })`). */
export function aisIcons(): { id: string; image: IconImage }[] {
  const c = ICON_SIZE / 2;
  // Triangle effilé pointant vers le nord (rotation 0), lisible sur la carte.
  const vessel: Point2[] = [
    [c, c - 15],
    [c + 9, c + 13],
    [c - 9, c + 13],
  ];
  // Ligne de cap : trait fin du centre vers le nord, au-delà de la proue.
  const h = HEADING_ICON_SIZE / 2;
  const heading = (x: number, y: number) => segmentDistance([x, y], [h, h], [h, SDF_RADIUS]) - 0.75;
  return [
    { id: AIS_VESSEL_ICON, image: sdfIcon(polygonDistance(vessel)) },
    { id: AIS_ROUND_ICON, image: sdfIcon((x, y) => Math.hypot(x - c, y - c) - 8) },
    { id: AIS_HEADING_ICON, image: sdfIcon(heading, HEADING_ICON_SIZE) },
  ];
}
