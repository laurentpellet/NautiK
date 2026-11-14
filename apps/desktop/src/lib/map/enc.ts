/**
 * Cartes ENC S-57 sur la carte : choix des cellules à afficher pour la vue
 * et couches MapLibre d'une symbologie simplifiée, inspirée de l'IHO S-52
 * (sans la remplacer). Les objets et leurs catégories sont préparés par
 * le moteur ; ce module ne fait que du tracé.
 *
 * La carte n'a pas de polices (fonctionnement hors ligne) : les sondes
 * sont dessinées en petites images, créées à la demande
 * (`soundingImage`).
 */
import type { FeatureCollection } from 'geojson';
import type { ExpressionSpecification, LayerSpecification } from 'maplibre-gl';
import type { EncFeature, EncMapCell } from '../services/enc';
import type { Bounds } from '../services/weather';
import type { Theme } from './style';

export const ENC_SOURCE = 'nautik-enc';

/** Isobathe des petits fonds (m). */
export const SHALLOW_CONTOUR_M = 2;
/** Isobathe de sécurité (m) : en deçà, sondes et dangers en évidence. */
export const SAFETY_CONTOUR_M = 10;
/** Isobathe des grands fonds (m). */
export const DEEP_CONTOUR_M = 30;

/** Nombre maximal de cellules affichées en même temps. */
export const MAX_DISPLAYED_CELLS = 16;

/** Préfixe des images de sondes. */
export const SOUNDING_IMAGE_PREFIX = 'enc-snd-';

/** Zoom à partir duquel les sondes sont dessinées. */
export const SOUNDING_MIN_ZOOM = 12;

type Collection = FeatureCollection<EncFeature['geometry'], EncFeature['properties']>;

export function emptyEncCollection(): Collection {
  return { type: 'FeatureCollection', features: [] };
}

/** L'emprise d'une cellule recoupe la vue (vue à cheval sur l'antiméridien comprise). */
export function boundsIntersect(bounds: [number, number, number, number], view: Bounds): boolean {
  const [w, s, e, n] = bounds;
  if (n < view.south || s > view.north) return false;
  const overlaps = (west: number, east: number) => e >= west && w <= east;
  return view.west <= view.east
    ? overlaps(view.west, view.east)
    : overlaps(view.west, 180) || overlaps(-180, view.east);
}

/**
 * Cellules à afficher pour la vue : lisibles, non annulées, dont l'emprise
 * recoupe la vue ; les plus détaillées sont retenues en priorité (au plus
 * `max`), puis rangées de la moins à la plus détaillée (ordre de tracé :
 * la plus détaillée par-dessus).
 */
export function cellsForView(
  cells: EncMapCell[],
  view: Bounds | null,
  max: number = MAX_DISPLAYED_CELLS,
): EncMapCell[] {
  if (!view) return [];
  return cells
    .filter((c) => !c.error && !c.cancelled && c.bounds !== null && boundsIntersect(c.bounds, view))
    .sort((a, b) => b.usage - a.usage || a.name.localeCompare(b.name))
    .slice(0, Math.max(0, max))
    .sort((a, b) => a.usage - b.usage || a.name.localeCompare(b.name));
}

/** Entités des cellules, dans l'ordre de tracé donné. */
export function encCollection(
  cells: EncMapCell[],
  features: Map<string, EncFeature[]>,
): Collection {
  return {
    type: 'FeatureCollection',
    features: cells.flatMap((c) => features.get(c.path) ?? []),
  };
}

interface Palette {
  land: string;
  drying: string;
  veryShallow: string;
  shallow: string;
  medium: string;
  deep: string;
  unsurveyed: string;
  contour: string;
  coast: string;
  danger: string;
  soundingShallow: string;
  soundingDeep: string;
  halo: string;
}

/** Couleurs jour et nuit (la nuit, teintes sombres pour préserver la vision). */
export const ENC_PALETTE: Record<Theme, Palette> = {
  day: {
    land: '#e8d9a8',
    drying: '#a3c99a',
    veryShallow: '#73a9e0',
    shallow: '#a5c9ef',
    medium: '#d0e3f7',
    deep: '#f7fbff',
    unsurveyed: '#c8c8c8',
    contour: '#6f8293',
    coast: '#3b3b3b',
    danger: '#c2187a',
    soundingShallow: '#101820',
    soundingDeep: '#6f7c88',
    halo: '#ffffff',
  },
  night: {
    land: '#2e2817',
    drying: '#1f3320',
    veryShallow: '#14304f',
    shallow: '#102640',
    medium: '#0c1d31',
    deep: '#081422',
    unsurveyed: '#262626',
    contour: '#4f5f6e',
    coast: '#9a8f78',
    danger: '#a3266d',
    soundingShallow: '#c9c3b0',
    soundingDeep: '#6d6a60',
    halo: '#081422',
  },
};

const cat = (name: string): ExpressionSpecification => ['==', ['get', 'cat'], name];

/** Couleur d'une zone de profondeur selon DRVAL1 (absente : petits fonds, par prudence). */
export function depthColor(theme: Theme): ExpressionSpecification {
  const p = ENC_PALETTE[theme];
  return [
    'step',
    ['coalesce', ['get', 'drval1'], 0],
    p.drying,
    0,
    p.veryShallow,
    SHALLOW_CONTOUR_M,
    p.shallow,
    SAFETY_CONTOUR_M,
    p.medium,
    DEEP_CONTOUR_M,
    p.deep,
  ];
}

/** Couleur d'une bouée ou balise d'après COLOUR. */
export const BUOY_COLOR: ExpressionSpecification = [
  'match',
  ['coalesce', ['get', 'colour'], 0],
  1,
  '#ffffff',
  2,
  '#111111',
  3,
  '#d32f2f',
  4,
  '#2e7d32',
  6,
  '#f2c200',
  11,
  '#ff8c00',
  '#9e9e9e',
];

/** Profondeur sous l'isobathe de sécurité (inconnue : dangereuse). */
const shallowerThanSafety: ExpressionSpecification = [
  '<',
  ['coalesce', ['get', 'depth'], 0],
  SAFETY_CONTOUR_M,
];

/** Identifiant de l'image d'une sonde. */
export function soundingImageExpression(theme: Theme): ExpressionSpecification {
  return [
    'concat',
    `${SOUNDING_IMAGE_PREFIX}${theme}-`,
    ['case', shallowerThanSafety, 'shallow-', 'deep-'],
    ['get', 'label'],
  ];
}

/** Décompose l'identifiant d'une image de sonde ; `null` si ce n'en est pas une. */
export function parseSoundingImageId(
  id: string,
): { theme: Theme; shallow: boolean; label: string } | null {
  const m = new RegExp(
    `^${SOUNDING_IMAGE_PREFIX}(day|night)-(shallow|deep)-(-?\\d{1,4}(\\.\\d)?)$`,
  ).exec(id);
  if (!m) return null;
  return { theme: m[1] as Theme, shallow: m[2] === 'shallow', label: m[3] ?? '' };
}

/** Couches ENC, de bas en haut. */
export function encLayers(theme: Theme): LayerSpecification[] {
  const p = ENC_PALETTE[theme];
  const source = ENC_SOURCE;
  return [
    {
      id: 'enc-depth',
      type: 'fill',
      source,
      filter: cat('depth'),
      paint: { 'fill-color': depthColor(theme) },
    },
    {
      id: 'enc-unsurveyed',
      type: 'fill',
      source,
      filter: cat('unsurveyed'),
      paint: { 'fill-color': p.unsurveyed },
    },
    {
      id: 'enc-land',
      type: 'fill',
      source,
      filter: cat('land'),
      paint: { 'fill-color': p.land },
    },
    {
      id: 'enc-danger-area',
      type: 'fill',
      source,
      filter: ['all', cat('danger'), ['==', ['geometry-type'], 'Polygon']],
      paint: { 'fill-color': p.danger, 'fill-opacity': 0.2 },
    },
    {
      id: 'enc-contour',
      type: 'line',
      source,
      filter: cat('contour'),
      paint: {
        'line-color': p.contour,
        'line-width': ['case', ['==', ['get', 'valdco'], SAFETY_CONTOUR_M], 1.8, 0.6],
      },
    },
    {
      id: 'enc-coast',
      type: 'line',
      source,
      filter: cat('coast'),
      paint: { 'line-color': p.coast, 'line-width': 1.2 },
    },
    {
      id: 'enc-danger-line',
      type: 'line',
      source,
      filter: ['all', cat('danger'), ['!=', ['geometry-type'], 'Point']],
      paint: { 'line-color': p.danger, 'line-width': 1.2, 'line-dasharray': [2, 1.5] },
    },
    {
      id: 'enc-sounding',
      type: 'symbol',
      source,
      minzoom: SOUNDING_MIN_ZOOM,
      filter: cat('sounding'),
      layout: { 'icon-image': soundingImageExpression(theme), 'icon-allow-overlap': false },
    },
    {
      id: 'enc-danger-point',
      type: 'circle',
      source,
      filter: ['all', cat('danger'), ['==', ['geometry-type'], 'Point']],
      paint: {
        'circle-radius': 4.5,
        'circle-color': ['case', shallowerThanSafety, p.danger, p.soundingDeep],
        'circle-stroke-color': p.halo,
        'circle-stroke-width': 1.5,
      },
    },
    {
      id: 'enc-light',
      type: 'circle',
      source,
      filter: cat('light'),
      paint: { 'circle-radius': 6, 'circle-color': p.danger, 'circle-opacity': 0.35 },
    },
    {
      id: 'enc-aid',
      type: 'circle',
      source,
      filter: ['any', cat('buoy'), cat('beacon')],
      paint: {
        'circle-radius': ['case', cat('buoy'), 4, 3.5],
        'circle-color': BUOY_COLOR,
        'circle-stroke-color': p.coast,
        'circle-stroke-width': 1,
      },
    },
  ];
}

/** Propriétés à changer au changement de thème : [couche, propriété, valeur]. */
export function encThemeChanges(theme: Theme): [string, 'paint' | 'layout', string, unknown][] {
  const changes: [string, 'paint' | 'layout', string, unknown][] = [];
  for (const layer of encLayers(theme)) {
    const sections = [
      ['paint', 'paint' in layer ? layer.paint : undefined],
      ['layout', 'layout' in layer ? layer.layout : undefined],
    ] as const;
    for (const [kind, values] of sections) {
      for (const [property, value] of Object.entries(values ?? {})) {
        changes.push([layer.id, kind, property, value]);
      }
    }
  }
  return changes;
}

/** Image d'une sonde : chiffres avec un liseré pour rester lisibles. */
export function soundingImage(
  label: string,
  shallow: boolean,
  theme: Theme,
  ratio = 1,
): ImageData | null {
  const p = ENC_PALETTE[theme];
  const fontPx = 11 * ratio;
  const canvas = document.createElement('canvas');
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;
  const font = `${shallow ? 'bold ' : ''}${fontPx}px sans-serif`;
  ctx.font = font;
  const width = Math.ceil(ctx.measureText(label).width + 4 * ratio);
  const height = Math.ceil(fontPx + 4 * ratio);
  canvas.width = width;
  canvas.height = height;
  ctx.font = font;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.lineWidth = 3 * ratio;
  ctx.strokeStyle = p.halo;
  ctx.strokeText(label, width / 2, height / 2);
  ctx.fillStyle = shallow ? p.soundingShallow : p.soundingDeep;
  ctx.fillText(label, width / 2, height / 2);
  return ctx.getImageData(0, 0, width, height);
}
