import { describe, expect, it } from 'vitest';
import { EncCellCache } from './encCells';
import type { EncMapCell } from './services/enc';

const cell = (path: string) => ({ path }) as EncMapCell;

describe('cache des cellules ENC', () => {
  it('connaît les cellules chargées ou en échec et se vide', () => {
    const cache = new EncCellCache();
    cache.features.set('A.000', []);
    cache.failed.set('B.000', 'illisible');
    expect(cache.known('A.000')).toBe(true);
    expect(cache.known('B.000')).toBe(true);
    expect(cache.known('C.000')).toBe(false);
    cache.clear();
    expect(cache.known('A.000') || cache.known('B.000')).toBe(false);
  });

  it('libère les plus anciennes sans toucher aux cellules de la vue', () => {
    const cache = new EncCellCache();
    for (const p of ['A', 'B', 'C', 'D']) cache.features.set(p, []);
    cache.evict([cell('A')], 4);
    expect(cache.features.size).toBe(4);
    cache.evict([cell('A'), cell('B')], 2);
    expect([...cache.features.keys()]).toEqual(['A', 'B']);
    cache.evict([cell('A'), cell('B')], 0);
    expect([...cache.features.keys()]).toEqual(['A', 'B']);
  });
});
