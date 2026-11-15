import { describe, expect, it } from 'vitest';
import {
  MEASURE_LEG_LABELS_LAYER,
  MEASURE_SOURCE,
  measureData,
  measureLayers,
  measureThemeChanges,
} from './measure';

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

  it('affiche la longueur de chaque segment, seulement si elle correspond', () => {
    const points = [p(1, 2), p(3, 4), p(5, 6)];
    const legs = measureData(points, ['1.2 NM', '0.8 NM']).features.filter(
      (f) => 'leg' in f.properties,
    );
    expect(legs.map((f) => f.properties)).toEqual([
      { leg: 0, label: '1.2 NM' },
      { leg: 1, label: '0.8 NM' },
    ]);
    expect(legs[1]?.geometry.coordinates).toEqual([
      [4, 3],
      [6, 5],
    ]);
    // Calcul en cours (nombre différent) : aucune longueur affichée.
    expect(measureData(points, ['1.2 NM']).features.some((f) => 'leg' in f.properties)).toBe(false);
    expect(measureLayers().at(-1)?.id).toBe(MEASURE_LEG_LABELS_LAYER);
    expect(JSON.stringify(measureThemeChanges('night'))).toContain('night:leg:');
  });

  it('n’utilise que sa propre source', () => {
    const layers = measureLayers();
    expect(layers.every((l) => 'source' in l && l.source === MEASURE_SOURCE)).toBe(true);
  });
});
