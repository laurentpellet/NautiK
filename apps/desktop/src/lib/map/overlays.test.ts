import { describe, expect, it } from 'vitest';
import type { Route } from '../services/routing';
import {
  DRAFT_POINTS_LAYER,
  DRAFT_SOURCE,
  OVERLAY_LABEL_LAYERS,
  ROUTES_SOURCE,
  ROUTE_ARROW_IMAGE,
  TRACKS_SOURCE,
  draftData,
  draftPointName,
  DEFAULT_ACTIVE_ROUTE_COLOR,
  activeRouteColorChanges,
  trackColorChanges,
  DEFAULT_TRACK_COLOR,
  overlayLayers,
  overlayThemeChanges,
  routesData,
  tracksData,
} from './overlays';

const p = (latitudeDeg: number, longitudeDeg: number) => ({ latitudeDeg, longitudeDeg });

const route = (id: string, active: boolean): Route => ({
  id,
  name: id,
  description: null,
  totalDistanceM: 1000,
  active,
  bounds: null,
  waypoints: [
    { id: `${id}-1`, name: 'WP001', position: p(-14.968, -147.638), description: null },
    { id: `${id}-2`, name: 'WP002', position: p(-14.9, -147.6), description: null },
  ],
});

describe('surimpressions', () => {
  it('produit une ligne et des points par route, en [lon, lat]', () => {
    const data = routesData([route('a', false), route('b', true)], 1);
    expect(data.features).toHaveLength(6);
    const lineA = data.features[0];
    expect(lineA?.geometry).toEqual({
      type: 'LineString',
      coordinates: [
        [-147.638, -14.968],
        [-147.6, -14.9],
      ],
    });
    const targets = data.features.filter((f) => f.properties.target === true);
    expect(targets).toHaveLength(1);
    expect(targets[0]?.properties.id).toBe('b-2');
  });

  it('ajoute la longueur de chaque segment de la route active, au milieu', () => {
    const data = routesData([route('a', false), route('b', true)], null, null, ['4.2 NM']);
    const legs = data.features.filter((f) => 'leg' in f.properties);
    expect(legs).toHaveLength(1);
    expect(legs[0]?.properties).toEqual({ leg: 0, label: '4.2 NM' });
    expect(legs[0]?.geometry.type).toBe('LineString');
    // Nombre incohérent avec les segments : rien d'affiché.
    const wrong = routesData([route('b', true)], null, null, ['1', '2']);
    expect(wrong.features.some((f) => 'leg' in f.properties)).toBe(false);
    // Les segments étiquetés ne sont pas redessinés comme routes.
    const line = overlayLayers().find((l) => l.id === 'nautik-routes-line');
    expect(JSON.stringify(line && 'filter' in line ? line.filter : null)).toContain('leg');
  });

  it('oriente des flèches de sens le long des routes et du tracé', () => {
    const arrows = overlayLayers().filter((l) => l.id.endsWith('-arrows'));
    expect(arrows.map((l) => ('source' in l ? l.source : null))).toEqual([
      ROUTES_SOURCE,
      DRAFT_SOURCE,
    ]);
    for (const layer of arrows) {
      const layout = ('layout' in layer ? layer.layout : {}) as Record<string, unknown>;
      expect(layout['symbol-placement']).toBe('line');
      expect(layout['icon-image']).toBe(ROUTE_ARROW_IMAGE);
      expect(layout['icon-rotation-alignment']).toBe('map');
    }
  });

  it('ne marque aucune cible sans route active', () => {
    const data = routesData([route('a', false)], 0);
    expect(data.features.some((f) => f.properties.target === true)).toBe(false);
  });

  it("n'ajoute la ligne du tracé qu'à partir de deux points", () => {
    const d = (name: string, lat: number, lon: number) => ({ name, position: p(lat, lon) });
    expect(draftData([]).features).toHaveLength(0);
    expect(draftData([d('', 0, 0)]).features).toHaveLength(1);
    const three = draftData([d('Départ', 0, 0), d('', 0, 1), d('Arrivée', 0, 2)]);
    expect(three.features).toHaveLength(4);
    expect(three.features[0]?.geometry.type).toBe('LineString');
    expect(three.features.slice(1).map((f) => f.properties.role)).toEqual(['start', 'via', 'end']);
    expect(three.features.slice(1).map((f) => f.properties.label)).toEqual([
      'Départ',
      'WP002',
      'Arrivée',
    ]);
  });

  it('nomme provisoirement les points sans nom', () => {
    expect(draftPointName('  ', 0)).toBe('WP001');
    expect(draftPointName(' Passe ', 4)).toBe('Passe');
  });

  it('étiquette les waypoints et masque la route en édition', () => {
    const data = routesData([route('a', false), route('b', false)], null, 'a');
    expect(data.features.map((f) => f.properties.routeId ?? f.properties.id)).toEqual([
      'b',
      'b',
      'b',
    ]);
    expect(data.features[1]?.properties.label).toBe('WP001');
  });

  it('ignore les traces de moins de deux points', () => {
    const data = tracksData([
      { id: 'vide', points: [p(0, 0)], recording: true },
      { id: 't', points: [p(0, 0), p(0, 1)], recording: false },
    ]);
    expect(data.features.map((f) => f.properties.id)).toEqual(['t']);
  });

  it('déclare des couches sur les seules sources de surimpression', () => {
    const sources = new Set(overlayLayers().map((l) => ('source' in l ? l.source : null)));
    expect(sources).toEqual(new Set([ROUTES_SOURCE, DRAFT_SOURCE, TRACKS_SOURCE]));
    const symbols = overlayLayers().filter((l) => l.type === 'symbol');
    expect(symbols.map((l) => l.id)).toEqual([
      'nautik-routes-arrows',
      'nautik-routes-leg-labels',
      'nautik-routes-labels',
      'nautik-draft-arrows',
      'nautik-draft-labels',
    ]);
    expect(
      OVERLAY_LABEL_LAYERS.map(([id]) => id).every((id) => symbols.some((l) => l.id === id)),
    ).toBe(true);
    expect(overlayLayers().some((l) => l.id === DRAFT_POINTS_LAYER)).toBe(true);
  });

  it('dessine toutes les traces, en cours comprise, dans la couleur choisie', () => {
    const color = (layers: ReturnType<typeof overlayLayers>) => {
      const layer = layers.find((l) => l.id === 'nautik-tracks-line');
      return layer && 'paint' in layer
        ? (layer.paint as Record<string, unknown>)['line-color']
        : null;
    };
    expect(color(overlayLayers())).toBe(DEFAULT_TRACK_COLOR);
    expect(color(overlayLayers('day', '#ffd600', '#1565c0'))).toBe('#1565c0');
    expect(trackColorChanges('#1565c0')).toEqual([['nautik-tracks-line', 'line-color', '#1565c0']]);
  });

  it('dessine la route active dans la couleur choisie, avec un liseré', () => {
    const paint = (layers: ReturnType<typeof overlayLayers>, id: string) =>
      JSON.stringify(layers.find((l) => l.id === id && 'paint' in l)?.paint);
    const byDefault = overlayLayers();
    expect(paint(byDefault, 'nautik-routes-line')).toContain(DEFAULT_ACTIVE_ROUTE_COLOR);
    expect(paint(byDefault, 'nautik-routes-points')).toContain(DEFAULT_ACTIVE_ROUTE_COLOR);
    const custom = overlayLayers('night', '#00e5ff');
    expect(paint(custom, 'nautik-routes-line')).toContain('#00e5ff');
    expect(paint(custom, 'nautik-routes-line')).not.toContain(DEFAULT_ACTIVE_ROUTE_COLOR);
    // Le liseré est sous la ligne et ne concerne que la route active.
    const ids = custom.map((l) => l.id);
    expect(ids.indexOf('nautik-routes-casing')).toBe(ids.indexOf('nautik-routes-line') - 1);
    const casing = custom.find((l) => l.id === 'nautik-routes-casing');
    expect(JSON.stringify(casing && 'filter' in casing ? casing.filter : null)).toContain('active');

    const changes = activeRouteColorChanges('#00e5ff');
    expect(changes.map(([id, property]) => [id, property])).toEqual([
      ['nautik-routes-line', 'line-color'],
      ['nautik-routes-points', 'circle-stroke-color'],
    ]);
    for (const [id, property, value] of changes) {
      const layer = custom.find((l) => l.id === id);
      expect(
        layer && 'paint' in layer ? (layer.paint as Record<string, unknown>)[property] : null,
      ).toEqual(value);
    }
  });

  it('change l’image des étiquettes avec le thème', () => {
    const changes = overlayThemeChanges('night');
    expect(changes.map(([id]) => id)).toEqual(OVERLAY_LABEL_LAYERS.map(([id]) => id));
    expect(JSON.stringify(changes[0]?.[1])).toContain('night:wp:');
    expect(JSON.stringify(changes[1]?.[1])).toContain('night:leg:');
  });
});
