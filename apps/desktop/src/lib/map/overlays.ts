/**
 * Couches de surimpression : routes, route en cours d'édition, traces.
 *
 * Fonctions pures (GeoJSON et spécifications de couches) : ChartView se
 * contente de les appliquer. Le style étant entièrement local (pas de
 * glyphes), les noms des waypoints sont des images d'étiquettes
 * (`labels.ts`).
 */
import type { Feature, FeatureCollection, LineString, Point, Position } from 'geojson';
import type { ExpressionSpecification, LayerSpecification } from 'maplibre-gl';
import type { LatLon, Route } from '../services/routing';
import { labelImageExpression, labelText } from './labels';
import type { Theme } from './style';

export const ROUTES_SOURCE = 'nautik-routes';
export const DRAFT_SOURCE = 'nautik-draft';
export const TRACKS_SOURCE = 'nautik-tracks';

/** Couche des points de la route en édition (déplaçables). */
export const DRAFT_POINTS_LAYER = 'nautik-draft-points';

/** Point d'une route en édition, tel qu'affiché. */
export interface DraftPoint {
  name: string;
  position: LatLon;
}

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

/** Nom affiché d'un point en édition : son nom, sinon `WPnnn` provisoire. */
export function draftPointName(name: string, index: number): string {
  return name.trim() || `WP${String(index + 1).padStart(3, '0')}`;
}

/**
 * Routes enregistrées : un segment par route et un point par waypoint.
 * `hiddenId` : route en cours d'édition, affichée par le brouillon.
 * `legLabels` : longueurs (déjà formatées, calculées par le moteur) des
 * segments de la route active, affichées au milieu de chaque segment ;
 * ignorées si leur nombre ne correspond pas aux segments.
 */
export function routesData(
  routes: Route[],
  targetIndex: number | null = null,
  hiddenId: string | null = null,
  legLabels: string[] | null = null,
): Collection {
  const features: Collection['features'] = [];
  for (const route of routes) {
    if (route.id === hiddenId) continue;
    const positions = route.waypoints.map((w) => w.position);
    features.push(line(positions, { id: route.id, active: route.active }));
    if (route.active && legLabels && legLabels.length === positions.length - 1) {
      legLabels.forEach((text, index) => {
        const from = positions[index];
        const to = positions[index + 1];
        if (from && to) features.push(line([from, to], { leg: index, label: labelText(text) }));
      });
    }
    route.waypoints.forEach((w, index) => {
      features.push(
        point(w.position, {
          id: w.id,
          routeId: route.id,
          active: route.active,
          target: route.active && index === targetIndex,
          label: labelText(w.name),
        }),
      );
    });
  }
  return { type: 'FeatureCollection', features };
}

/** Route en cours d'édition ; départ et arrivée sont distingués. */
export function draftData(points: DraftPoint[]): Collection {
  const last = points.length - 1;
  const features: Collection['features'] = points.map((p, index) =>
    point(p.position, {
      index,
      role: index === 0 ? 'start' : index === last ? 'end' : 'via',
      label: labelText(draftPointName(p.name, index)),
    }),
  );
  if (points.length >= 2) {
    features.unshift(
      line(
        points.map((p) => p.position),
        {},
      ),
    );
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

const isPoint: ExpressionSpecification = ['==', ['geometry-type'], 'Point'];
const isLine: ExpressionSpecification = ['==', ['geometry-type'], 'LineString'];

/** Étiquettes des waypoints, à droite du point ; masquées si elles se chevauchent. */
function labelLayer(id: string, source: string, theme: Theme, minzoom: number): LayerSpecification {
  return {
    id,
    type: 'symbol',
    source,
    filter: isPoint,
    minzoom,
    layout: {
      'icon-image': labelImageExpression(theme, 'wp'),
      'icon-anchor': 'left',
      'icon-offset': [8, 0],
      'icon-optional': true,
      'icon-allow-overlap': false,
    },
  };
}

/** Image de la flèche de sens des routes (pointe vers la droite). */
export const ROUTE_ARROW_IMAGE = 'nautik-route-arrow';

/**
 * Flèche de sens : chevron blanc cerné de sombre, lisible sur toutes les
 * couleurs de route. `null` si le canevas est indisponible.
 */
export function routeArrowImage(ratio = 1): ImageData | null {
  const size = Math.ceil(14 * ratio);
  const canvas = document.createElement('canvas');
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;
  const s = size / 14;
  ctx.beginPath();
  ctx.moveTo(3 * s, 2.5 * s);
  ctx.lineTo(10.5 * s, 7 * s);
  ctx.lineTo(3 * s, 11.5 * s);
  ctx.lineJoin = 'round';
  ctx.lineCap = 'round';
  ctx.strokeStyle = 'rgba(0, 0, 0, 0.75)';
  ctx.lineWidth = 4.5 * s;
  ctx.stroke();
  ctx.strokeStyle = '#ffffff';
  ctx.lineWidth = 2 * s;
  ctx.stroke();
  return ctx.getImageData(0, 0, size, size);
}

/** Flèches de sens le long des segments (routes enregistrées ou en édition). */
function arrowLayer(
  id: string,
  source: string,
  filter: ExpressionSpecification,
): LayerSpecification {
  return {
    id,
    type: 'symbol',
    source,
    filter,
    minzoom: 5,
    layout: {
      'symbol-placement': 'line',
      'symbol-spacing': 140,
      'icon-image': ROUTE_ARROW_IMAGE,
      'icon-rotation-alignment': 'map',
      'icon-allow-overlap': true,
      'icon-ignore-placement': true,
    },
  };
}

/** Segment de la route active portant sa longueur. */
const isLeg: ExpressionSpecification = ['has', 'leg'];

/** Longueurs des segments de la route active, en italique au milieu du segment. */
function legLabelLayer(theme: Theme): LayerSpecification {
  return {
    id: 'nautik-routes-leg-labels',
    type: 'symbol',
    source: ROUTES_SOURCE,
    filter: ['all', isLine, isLeg],
    minzoom: 6,
    layout: {
      'symbol-placement': 'line-center',
      'icon-image': labelImageExpression(theme, 'leg'),
      'icon-rotation-alignment': 'viewport',
      'icon-allow-overlap': false,
    },
  };
}

/** Couches d'étiquettes dont l'image dépend du thème, avec leur style. */
export const OVERLAY_LABEL_LAYERS = [
  ['nautik-routes-labels', 'wp'],
  ['nautik-routes-leg-labels', 'leg'],
  ['nautik-draft-labels', 'wp'],
] as const;

/** Changements à appliquer aux étiquettes lors d'un changement de thème. */
export function overlayThemeChanges(theme: Theme): [string, ExpressionSpecification][] {
  return OVERLAY_LABEL_LAYERS.map(([id, style]) => [id, labelImageExpression(theme, style)]);
}

/** Couleur par défaut de la route active (jaune, réglable dans les Paramètres). */
export const DEFAULT_ACTIVE_ROUTE_COLOR = '#ffd600';
/** Couleur des routes enregistrées non actives. */
const ROUTE_COLOR = '#7b1fa2';

const routeColor = (active: string): ExpressionSpecification => [
  'case',
  ['get', 'active'],
  active,
  ROUTE_COLOR,
];

/**
 * Couleur par défaut des traces, enregistrées ou en cours (rose, réglable
 * dans les Paramètres).
 */
export const DEFAULT_TRACK_COLOR = '#e91e63';

/** Propriété de peinture à changer quand la couleur des traces change. */
export function trackColorChanges(
  color: string,
): [layer: string, property: 'line-color', value: string][] {
  return [['nautik-tracks-line', 'line-color', color]];
}

/** Propriétés de peinture à changer quand la couleur de la route active change. */
export function activeRouteColorChanges(
  color: string,
): [
  layer: string,
  property: 'line-color' | 'circle-stroke-color',
  value: ExpressionSpecification,
][] {
  return [
    ['nautik-routes-line', 'line-color', routeColor(color)],
    ['nautik-routes-points', 'circle-stroke-color', routeColor(color)],
  ];
}

/** Couches de surimpression, dans l'ordre d'empilement. */
export function overlayLayers(
  theme: Theme = 'day',
  activeRouteColor: string = DEFAULT_ACTIVE_ROUTE_COLOR,
  trackColor: string = DEFAULT_TRACK_COLOR,
): LayerSpecification[] {
  return [
    {
      id: 'nautik-tracks-line',
      type: 'line',
      source: TRACKS_SOURCE,
      layout: { 'line-cap': 'round', 'line-join': 'round' },
      paint: {
        'line-color': trackColor,
        'line-width': 2.5,
        'line-opacity': 0.85,
      },
    },
    {
      // Liseré sombre sous la route active : reste lisible quelle que soit
      // sa couleur (jaune sur fond clair notamment).
      id: 'nautik-routes-casing',
      type: 'line',
      source: ROUTES_SOURCE,
      filter: ['all', isLine, ['==', ['get', 'active'], true]],
      layout: { 'line-cap': 'round', 'line-join': 'round' },
      paint: { 'line-color': '#212121', 'line-width': 7, 'line-opacity': 0.55 },
    },
    {
      id: 'nautik-routes-line',
      type: 'line',
      source: ROUTES_SOURCE,
      filter: ['all', isLine, ['!', isLeg]],
      layout: { 'line-cap': 'round', 'line-join': 'round' },
      paint: {
        'line-color': routeColor(activeRouteColor),
        'line-width': ['case', ['get', 'active'], 4, 2.5],
      },
    },
    {
      id: 'nautik-routes-points',
      type: 'circle',
      source: ROUTES_SOURCE,
      filter: isPoint,
      paint: {
        'circle-radius': ['case', ['get', 'target'], 8, 5],
        'circle-color': ['case', ['get', 'target'], '#ffeb3b', '#ffffff'],
        'circle-stroke-color': routeColor(activeRouteColor),
        'circle-stroke-width': 2.5,
      },
    },
    arrowLayer('nautik-routes-arrows', ROUTES_SOURCE, ['all', isLine, ['!', isLeg]]),
    legLabelLayer(theme),
    labelLayer('nautik-routes-labels', ROUTES_SOURCE, theme, 8),
    {
      id: 'nautik-draft-line',
      type: 'line',
      source: DRAFT_SOURCE,
      filter: isLine,
      paint: { 'line-color': '#ff6f00', 'line-width': 3, 'line-dasharray': [2, 1.5] },
    },
    arrowLayer('nautik-draft-arrows', DRAFT_SOURCE, isLine),
    {
      id: DRAFT_POINTS_LAYER,
      type: 'circle',
      source: DRAFT_SOURCE,
      filter: isPoint,
      paint: {
        'circle-radius': ['match', ['get', 'role'], 'via', 6, 8],
        // Départ vert, arrivée rouge, intermédiaires blancs.
        'circle-color': ['match', ['get', 'role'], 'start', '#2e7d32', 'end', '#c62828', '#ffffff'],
        'circle-stroke-color': '#ff6f00',
        'circle-stroke-width': 2.5,
      },
    },
    labelLayer('nautik-draft-labels', DRAFT_SOURCE, theme, 0),
  ];
}
