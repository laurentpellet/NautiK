import { describe, expect, it } from 'vitest';
import { STALE_POSITION_MS, formatAge, formatLatitude, formatLongitude, isStale } from './format';

describe('coordonnées', () => {
  it('formate latitude et longitude en degrés-minutes', () => {
    expect(formatLatitude(-17.535)).toBe('17°32.100′ S');
    expect(formatLongitude(-149.569)).toBe('149°34.140′ W');
    expect(formatLatitude(48.5)).toBe('48°30.000′ N');
    expect(formatLongitude(2.25)).toBe('002°15.000′ E');
  });

  it('gère équateur, méridien et bornes', () => {
    expect(formatLatitude(0)).toBe('00°00.000′ N');
    expect(formatLongitude(0)).toBe('000°00.000′ E');
    expect(formatLatitude(-90)).toBe('90°00.000′ S');
    expect(formatLongitude(180)).toBe('180°00.000′ E');
  });

  it('reporte l’arrondi des minutes sur les degrés', () => {
    expect(formatLatitude(10.9999999)).toBe('11°00.000′ N');
  });
});

describe('âge et obsolescence', () => {
  it('formate les âges', () => {
    expect(formatAge(0)).toBe('0 s');
    expect(formatAge(-500)).toBe('0 s');
    expect(formatAge(59_999)).toBe('59 s');
    expect(formatAge(60_000)).toBe('1 min');
    expect(formatAge(3_600_000)).toBe('1 h');
  });

  it('détecte une position obsolète au-delà du seuil', () => {
    expect(isStale(0, STALE_POSITION_MS)).toBe(false);
    expect(isStale(0, STALE_POSITION_MS + 1)).toBe(true);
  });
});
