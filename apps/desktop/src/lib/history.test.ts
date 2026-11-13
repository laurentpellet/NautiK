import { describe, expect, it } from 'vitest';
import {
  chartRange,
  chartUnit,
  chartValue,
  recordSnapshot,
  utcHourMinute,
  type History,
} from './history';
import type { InstrumentsSnapshot, Reading } from './services/instruments';

const reading = (over: Partial<Reading>): Reading => ({
  quantity: 'speedOverGround',
  value: 3,
  source: 'nmea0183',
  sourceId: 'gps',
  measuredAt: 10_000,
  ageMs: 100,
  stale: false,
  freshSources: 1,
  ...over,
});

const snap = (readings: Reading[]): InstrumentsSnapshot =>
  ({ readings, position: null }) as unknown as InstrumentsSnapshot;

describe('conversion des courbes', () => {
  it('convertit vitesses, profondeurs et angles', () => {
    expect(chartValue('speed', 1852 / 3600)).toBeCloseTo(1);
    expect(chartUnit('speed')).toBe('kn');
    expect(chartValue('depth', 10)).toBe(10);
    expect(chartUnit('depth')).toBe('m');
    expect(chartValue('angle', -Math.PI / 2)).toBeCloseTo(270);
    expect(chartValue('angle', 2 * Math.PI)).toBeCloseTo(0);
    expect(chartUnit('angle')).toBe('°');
    expect(utcHourMinute(Date.UTC(2026, 0, 1, 7, 4))).toBe('07:04');
    expect(chartValue('rpm', 40)).toBe(2400);
    expect(chartUnit('rpm')).toBe('tr/min');
    expect(chartValue('voltage', 12.7)).toBe(12.7);
  });

  it('choisit une échelle lisible', () => {
    expect(chartRange('angle', 10, 5)).toEqual([0, 360]);
    expect(chartRange('rpm', 2140, 0)).toEqual([0, 2500]);
    expect(chartRange('voltage', 13.4, 12.6)).toEqual([12, 14]);
    expect(chartRange('voltage', 12.4, 11.2)).toEqual([11, 13]);
    expect(chartRange('depth', 48, 10)).toEqual([0, 50]);
    expect(chartRange('speed', 0, 0)).toEqual([0, 5]);
  });
});

describe('historique', () => {
  it('retient les mesures fraîches des grandeurs suivies', () => {
    const h = recordSnapshot(
      {},
      snap([
        reading({}),
        reading({ quantity: 'depthBelowTransducer', value: 12 }),
        reading({ quantity: 'windAngleApparent', value: 1 }),
      ]),
    );
    expect(h.speedOverGround).toEqual([{ t: 10_000, v: 3 }]);
    expect(h.depthBelowTransducer).toEqual([{ t: 10_000, v: 12 }]);
    expect(Object.keys(h)).toHaveLength(2);
  });

  it('ignore les mesures obsolètes, non finies, répétées ou trop rapprochées', () => {
    const start: History = { speedOverGround: [{ t: 10_000, v: 3 }] };
    expect(recordSnapshot(start, snap([reading({ stale: true, measuredAt: 20_000 })]))).toBe(start);
    expect(recordSnapshot(start, snap([reading({ value: NaN, measuredAt: 20_000 })]))).toBe(start);
    expect(recordSnapshot(start, snap([reading({})]))).toBe(start);
    expect(recordSnapshot(start, snap([reading({ measuredAt: 11_000 })]))).toBe(start);
    expect(
      recordSnapshot(start, snap([reading({ measuredAt: 12_000 })])).speedOverGround,
    ).toHaveLength(2);
  });

  it('oublie les mesures hors fenêtre', () => {
    const start: History = { speedOverGround: [{ t: 0, v: 1 }] };
    const h = recordSnapshot(start, snap([reading({ measuredAt: 50_000 })]), 10_000);
    expect(h.speedOverGround).toEqual([{ t: 50_000, v: 3 }]);
  });
});
