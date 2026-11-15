import { describe, expect, it } from 'vitest';
import { MEASURE_SOURCE, measureData, measureLayers } from './measure';

const p = (latitudeDeg: number, longitudeDeg: number) => ({ latitudeDeg, longitudeDeg });

describe('couche de mesure', () => {
  it('dessine les points et, à partir de deux, le trait en [lon, lat]', () => {
    expect(measureData([]).features).toEqual([]);
    expect(measureData([p(1, 2)]).features.map((f) => f.geometry.type)).toEqual(['Point']);
    const data = measureData([p(1, 2), p(3, 4)]);
    expect(data.features.map((f) => f.geometry.type)).toEqual(['LineString', 'Point', 'Point']);
    expect(data.features[0]?.geometry.coordinates).toEqual([
      [2, 1],
      [4, 3],
    ]);
  });

  it('n’utilise que sa propre source', () => {
    const layers = measureLayers();
    expect(layers.every((l) => 'source' in l && l.source === MEASURE_SOURCE)).toBe(true);
  });
});
