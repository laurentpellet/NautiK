import { afterEach, describe, expect, it, vi } from 'vitest';
import { formatDepth, formatDistance, formatSpeed } from './format';
import { DEFAULT_UNITS, loadUnits, saveUnits, setUnits, speedIn, speedToMps } from './units.svelte';

const KNOT = 1852 / 3600;

afterEach(() => setUnits(DEFAULT_UNITS));

describe('unités', () => {
  it('convertit les vitesses dans les deux sens', () => {
    expect(speedIn(10 * KNOT, 'kn')).toBeCloseTo(10);
    expect(speedIn(10, 'kmh')).toBeCloseTo(36);
    expect(speedToMps(36, 'kmh')).toBeCloseTo(10);
    expect(speedToMps(3, 'ms')).toBe(3);
  });

  it("applique l'unité choisie à l'affichage", () => {
    setUnits({ speed: 'kmh', distance: 'km', depth: 'ft' });
    expect(formatSpeed(10)).toBe('36.0 km/h');
    expect(formatDistance(1500)).toBe('1.5 km');
    expect(formatDistance(500)).toBe('0.50 km');
    expect(formatDepth(3.048)).toBe('10.0 ft');
    setUnits(DEFAULT_UNITS);
    expect(formatSpeed(10 * KNOT)).toBe('10.0 kn');
    expect(formatDistance(1852)).toBe('1.0 NM');
    expect(formatDepth(12.5)).toBe('12.5 m');
  });

  it('mémorise les unités et ignore les valeurs inconnues', () => {
    const store = new Map<string, string>();
    const storage = {
      getItem: (k: string) => store.get(k) ?? null,
      setItem: (k: string, v: string) => void store.set(k, v),
    };
    expect(loadUnits(storage)).toEqual(DEFAULT_UNITS);
    saveUnits({ speed: 'ms', distance: 'km', depth: 'ft' }, storage);
    expect(loadUnits(storage)).toEqual({ speed: 'ms', distance: 'km', depth: 'ft' });
    store.set('nautik.units', JSON.stringify({ speed: 'mph', distance: 'km', depth: 2 }));
    expect(loadUnits(storage)).toEqual({ speed: 'kn', distance: 'km', depth: 'm' });
    store.set('nautik.units', '{pas du json');
    expect(loadUnits(storage)).toEqual(DEFAULT_UNITS);
    const broken = {
      getItem: vi.fn(() => {
        throw new Error('refusé');
      }),
    };
    expect(loadUnits(broken)).toEqual(DEFAULT_UNITS);
  });
});
