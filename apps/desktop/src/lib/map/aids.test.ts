import { describe, expect, it } from 'vitest';
import type { OwnShip } from '../services/ownShip';
import {
  AIDS_SOURCE,
  DRIFT_TICK_ICON,
  EBL_LENGTH_M,
  aidsData,
  aidsIcons,
  aidsLayers,
} from './aids';

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
  drift: null,
};

/** Mètres par degré de latitude (sphère de l'outil de tracé). */
const M_PER_DEG = 111_195;

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

describe('lignes de dérive', () => {
  const base = {
    vectors: false,
    laylines: false,
    mob: null,
    rangeBearing: null,
  };
  // Cap 0° (nord), route fond 90° (est) : 3 m/s, soit 180 m par minute.
  const drifting: OwnShip = {
    ...own,
    position: p(0, 0),
    laylines: null,
    drift: { cogRad: Math.PI / 2, headingRad: 0, sogMps: 3, angleRad: Math.PI / 2 },
  };

  it('trace la route fond et la ligne de foi graduées chaque minute', () => {
    const c = aidsData({ ...base, own: drifting, driftMinutes: 5 });
    expect(kinds(c).filter((k) => k !== 'driftTick')).toEqual(['driftHeading', 'driftCog']);
    const ticks = c.features.filter((f) => f.properties.kind === 'driftTick');
    expect(ticks).toHaveLength(10);
    const heading = ticks.filter((t) => t.properties.line === 'heading');
    expect(heading.map((t) => t.properties.minute)).toEqual([1, 2, 3, 4, 5]);
    expect(heading.map((t) => t.properties.last)).toEqual([false, false, false, false, true]);
    expect(heading[0]?.properties.bearing).toBe(0);
    // Graduation à 1 min sur la ligne de foi : 180 m au nord.
    const first = heading[0]?.geometry.coordinates as number[];
    expect((first[1] ?? 0) * M_PER_DEG).toBeCloseTo(180, 0);
    // Bout de la route fond : 5 min, soit 900 m à l'est.
    const cog = c.features.find((f) => f.properties.kind === 'driftCog');
    const end = (cog?.geometry.coordinates as number[][])[1] ?? [];
    expect((end[0] ?? 0) * M_PER_DEG).toBeCloseTo(900, 0);
    const cogTick = ticks.find((t) => t.properties.line === 'cog');
    expect(cogTick?.properties.bearing).toBeCloseTo(90);
  });

  it('suit la durée choisie et se masque', () => {
    const one = aidsData({ ...base, own: drifting, driftMinutes: 1 });
    expect(one.features.filter((f) => f.properties.kind === 'driftTick')).toHaveLength(2);
    expect(aidsData({ ...base, own: drifting, driftMinutes: null }).features).toEqual([]);
    expect(aidsData({ ...base, own: drifting }).features).toEqual([]);
    expect(aidsData({ ...base, own: drifting, driftMinutes: 0 }).features).toEqual([]);
  });

  it('ne trace rien sans dérive du moteur ni position fraîche', () => {
    expect(
      aidsData({ ...base, own: { ...drifting, drift: null }, driftMinutes: 5 }).features,
    ).toEqual([]);
    expect(
      aidsData({ ...base, own: { ...drifting, position: null }, driftMinutes: 5 }).features,
    ).toEqual([]);
  });

  it('déroule les lignes à l’antiméridien', () => {
    const c = aidsData({
      ...base,
      own: { ...drifting, position: p(0, 179.999) },
      driftMinutes: 5,
    });
    const cog = c.features.find((f) => f.properties.kind === 'driftCog');
    const end = (cog?.geometry.coordinates as number[][])[1] ?? [];
    expect(end[0]).toBeGreaterThan(180);
  });

  it('fournit l’icône de graduation', () => {
    const [tick] = aidsIcons();
    expect(tick?.id).toBe(DRIFT_TICK_ICON);
    expect(tick?.image.data.length).toBe((tick?.image.width ?? 0) * (tick?.image.height ?? 0) * 4);
  });
});

describe('aidsLayers', () => {
  it('utilise toutes la source des aides', () => {
    const layers = aidsLayers();
    expect(layers.length).toBe(9);
    expect(layers.every((l) => 'source' in l && l.source === AIDS_SOURCE)).toBe(true);
  });
});
