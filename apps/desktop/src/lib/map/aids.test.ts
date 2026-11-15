import { describe, expect, it } from 'vitest';
import type { OwnShip } from '../services/ownShip';
import { AIDS_SOURCE, EBL_LENGTH_M, aidsData, aidsLayers } from './aids';

const p = (latitudeDeg: number, longitudeDeg: number) => ({ latitudeDeg, longitudeDeg });

const own: OwnShip = {
  computedAt: 0,
  simulated: false,
  vectorHorizonS: 360,
  position: p(0, 179.99),
  vectorEnd: p(0, -179.99),
  headingEnd: p(0.01, 179.99),
  trueWindDirectionRad: 0,
  vmgWindMps: 1,
  vmgWaypointMps: 1,
  otherTackCogRad: 0,
  laylines: {
    mark: p(0.1, 0),
    starboardEnd: p(0.05, 0.05),
    portEnd: p(0.05, -0.05),
    upwind: true,
    windAngleRad: 0.78,
  },
};

const kinds = (c: ReturnType<typeof aidsData>) => c.features.map((f) => f.properties.kind);

describe('aidsData', () => {
  it('dessine vecteur, ligne de cap et laylines selon les calques', () => {
    const all = aidsData({ own, vectors: true, laylines: true, mob: null, rangeBearing: null });
    expect(kinds(all)).toEqual(['heading', 'vector', 'layline', 'layline']);
    // Antiméridien : la ligne ne fait pas le tour de la Terre.
    const vector = all.features[1];
    expect(vector?.geometry.coordinates).toEqual([
      [179.99, 0],
      [180.01, 0],
    ]);
    const none = aidsData({ own, vectors: false, laylines: false, mob: null, rangeBearing: null });
    expect(none.features).toEqual([]);
  });

  it("ne trace rien autour d'un bateau sans position fraîche", () => {
    const c = aidsData({
      own: { ...own, position: null },
      vectors: true,
      laylines: false,
      mob: p(0, 0),
      rangeBearing: { radiusM: 500, bearingRad: 0 },
    });
    expect(c.features).toEqual([]);
    expect(
      aidsData({ own: null, vectors: true, laylines: true, mob: null, rangeBearing: null })
        .features,
    ).toEqual([]);
  });

  it('trace la ligne MOB, le VRM et l’EBL', () => {
    const c = aidsData({
      own: { ...own, laylines: null, vectorEnd: null, headingEnd: null },
      vectors: true,
      laylines: true,
      mob: p(0.001, 179.99),
      rangeBearing: { radiusM: 1000, bearingRad: Math.PI / 2 },
    });
    expect(kinds(c)).toEqual(['mobLine', 'vrm', 'ebl']);
    const ebl = aidsData({
      own: { ...own, laylines: null, vectorEnd: null, headingEnd: null },
      vectors: false,
      laylines: false,
      mob: null,
      rangeBearing: { radiusM: null, bearingRad: 0 },
    }).features[0];
    expect(ebl?.properties.kind).toBe('ebl');
    const coords = ebl?.geometry.coordinates as number[][];
    const dLat = (coords[1]?.[1] ?? 0) - (coords[0]?.[1] ?? 0);
    expect(dLat * 111_195).toBeCloseTo(EBL_LENGTH_M, -2);
  });
});

describe('aidsLayers', () => {
  it('utilise toutes la source des aides', () => {
    const layers = aidsLayers();
    expect(layers.length).toBe(6);
    expect(layers.every((l) => 'source' in l && l.source === AIDS_SOURCE)).toBe(true);
  });
});
