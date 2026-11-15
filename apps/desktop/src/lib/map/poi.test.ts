import { describe, expect, it } from 'vitest';
import type { Poi } from '../services/poi';
import { POI_SOURCE, poiData, poiLayers, poiThemeChanges } from './poi';

const poi = (over: Partial<Poi>): Poi => ({
  osmRef: 'n1',
  kind: 'island',
  name: 'Tahiti',
  position: { latitudeDeg: -17.65, longitudeDeg: -149.45 },
  category: null,
  details: null,
  ...over,
});

describe('couche des îles et amers', () => {
  it('produit des points en [lon, lat] avec étiquette', () => {
    const data = poiData([
      poi({}),
      poi({ osmRef: 'n2', kind: 'light', name: null, details: 'Fl(2) WR 10s 12M' }),
      poi({ osmRef: 'n3', kind: 'beacon', name: null, category: 'starboard' }),
    ]);
    expect(data.features[0]?.geometry.coordinates).toEqual([-149.45, -17.65]);
    expect(data.features.map((f) => f.properties.label)).toEqual([
      'Tahiti',
      'Fl(2) WR 10s 12M',
      '',
    ]);
    expect(data.features[2]?.properties.category).toBe('starboard');
  });

  it('déclare des couches sur la seule source des amers, noms selon le zoom', () => {
    const layers = poiLayers('day');
    expect(new Set(layers.map((l) => ('source' in l ? l.source : null)))).toEqual(
      new Set([POI_SOURCE]),
    );
    const archipelago = layers.find((l) => l.id === 'nautik-poi-archipelago-labels');
    expect(archipelago?.minzoom).toBe(4);
    expect(archipelago?.maxzoom).toBe(9);
    expect(layers.find((l) => l.id === 'nautik-poi-islet-labels')?.minzoom).toBe(11);
    // Les étiquettes vides ne demandent jamais d'image.
    const symbols = layers.filter((l) => l.type === 'symbol');
    expect(symbols.every((l) => JSON.stringify(l.filter).includes('"label"'))).toBe(true);
  });

  it('suit le thème jour / nuit', () => {
    const changes = poiThemeChanges('night');
    expect(changes).toHaveLength(5);
    expect(JSON.stringify(changes[0]?.[1])).toContain('night:island:');
    expect(JSON.stringify(changes[4]?.[1])).toContain('night:mark:');
  });
});
