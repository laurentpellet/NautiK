import { describe, expect, it } from 'vitest';
import type { Route } from '../services/routing';
import {
  DRAFT_SOURCE,
  ROUTES_SOURCE,
  TRACKS_SOURCE,
  draftData,
  overlayLayers,
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

  it('ne marque aucune cible sans route active', () => {
    const data = routesData([route('a', false)], 0);
    expect(data.features.some((f) => f.properties.target === true)).toBe(false);
  });

  it("n'ajoute la ligne du tracé qu'à partir de deux points", () => {
    expect(draftData([]).features).toHaveLength(0);
    expect(draftData([p(0, 0)]).features).toHaveLength(1);
    const two = draftData([p(0, 0), p(0, 1)]);
    expect(two.features).toHaveLength(3);
    expect(two.features[0]?.geometry.type).toBe('LineString');
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
    expect(overlayLayers().some((l) => l.type === 'symbol')).toBe(false);
  });
});
