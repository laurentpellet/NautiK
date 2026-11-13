import { describe, expect, it } from 'vitest';
import {
  DIAL_CENTER,
  DIAL_RADIUS,
  appendSample,
  arcPath,
  compassTicks,
  dialPoint,
  headingMarker,
  dialTicks,
  inWindow,
  trendPath,
  trendScale,
  trendStats,
  windArrow,
} from './dashboard';

describe('cadran de vent', () => {
  it("place l'étrave en haut et tribord à droite", () => {
    const bow = dialPoint(0, 50);
    expect(bow.x).toBeCloseTo(DIAL_CENTER);
    expect(bow.y).toBeCloseTo(DIAL_CENTER - 50);
    const starboard = dialPoint(Math.PI / 2, 50);
    expect(starboard.x).toBeCloseTo(DIAL_CENTER + 50);
    const port = dialPoint(-Math.PI / 2, 50);
    expect(port.x).toBeCloseTo(DIAL_CENTER - 50);
  });

  it('gradue tous les 10° avec des étiquettes symétriques', () => {
    const ticks = dialTicks();
    expect(ticks).toHaveLength(36);
    expect(ticks.filter((t) => t.major)).toHaveLength(12);
    expect(ticks.find((t) => t.angleDeg === 300)?.label).toBe('60');
    expect(ticks.find((t) => t.angleDeg === 180)?.label).toBe('180');
    expect(ticks.find((t) => t.angleDeg === 0)?.label).toBeNull();
  });

  it('trace arcs et flèche', () => {
    expect(arcPath(25, 60, DIAL_RADIUS)).toMatch(/^M [\d.]+ [\d.]+ A 92 92 0 0 1 [\d.]+ [\d.]+$/);
    expect(windArrow(0)).toMatch(/^M 100\.00 [\d.]+ L .* Z$/);
  });
});

describe('tendance', () => {
  const samples = [
    { t: 0, v: 2 },
    { t: 1000, v: 4 },
    { t: 2000, v: 6 },
  ];

  it('statistiques sur la fenêtre', () => {
    expect(trendStats(samples)).toEqual({ avg: 4, max: 6, last: 6 });
    expect(trendStats([])).toBeNull();
    expect(inWindow(samples, 2000, 1000)).toEqual(samples.slice(1));
  });

  it('ajoute les mesures nouvelles et oublie les anciennes', () => {
    expect(appendSample(samples, { t: 2000, v: 9 }, 10_000)).toBe(samples);
    expect(appendSample(samples, { t: 2500, v: 1 }, 1_000)).toEqual([
      { t: 2000, v: 6 },
      { t: 2500, v: 1 },
    ]);
    expect(appendSample([], { t: 5, v: 1 }, 1_000)).toEqual([{ t: 5, v: 1 }]);
    expect(appendSample(samples, { t: 99_000, v: 1 }, 1_000)).toEqual([{ t: 99_000, v: 1 }]);
  });

  it('échelle arrondie', () => {
    expect(trendScale(11.2, 5)).toBe(15);
    expect(trendScale(0, 5)).toBe(5);
  });

  it("interrompt la courbe sur un trou, sans l'interpoler", () => {
    const frame = { now: 10_000, windowMs: 10_000, yMax: 10, width: 100, height: 50 };
    const path = trendPath([...samples, { t: 9000, v: 3 }], frame);
    expect(path.match(/M/g)).toHaveLength(2);
    expect(path.startsWith('M 0.00 40.00 L 10.00 30.00')).toBe(true);
  });

  it('coupe au passage 360° → 0° et inverse la profondeur', () => {
    const frame = { now: 2000, windowMs: 2000, yMax: 360, width: 100, height: 36 };
    const wrap = [
      { t: 0, v: 350 },
      { t: 1000, v: 5 },
    ];
    expect(trendPath(wrap, { ...frame, maxJump: 180 }).match(/M/g)).toHaveLength(2);
    expect(trendPath(wrap, frame).match(/M/g)).toHaveLength(1);
    expect(trendPath([{ t: 2000, v: 0 }], { ...frame, invert: true })).toBe('M 100.00 0.00');
  });
});

describe('compas', () => {
  it('étiquette les points cardinaux', () => {
    const ticks = compassTicks();
    expect(ticks.filter((t) => t.label).map((t) => t.label)).toEqual(['N', 'E', 'S', 'O']);
    expect(headingMarker(0)).toMatch(/^M 100\.00 /);
  });
});
