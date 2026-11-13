import { describe, expect, it } from 'vitest';
import { ANCHOR_SOURCE, CIRCLE_STEPS, anchorData, anchorLayers, circle } from './anchor';
import type { AnchorView } from '../services/alarms';

const anchor: AnchorView = {
  position: { latitudeDeg: -17.5, longitudeDeg: -149.5 },
  radiusM: 50,
  hysteresisM: 5,
  delayS: 10,
  distanceM: 12,
  bearingRad: 0,
};

/** Distance haversine (m), pour vérifier le tracé. */
function haversine(a: number[], b: number[]): number {
  const [lon1 = 0, lat1 = 0] = a;
  const [lon2 = 0, lat2 = 0] = b;
  const r = Math.PI / 180;
  const h =
    Math.sin(((lat2 - lat1) * r) / 2) ** 2 +
    Math.cos(lat1 * r) * Math.cos(lat2 * r) * Math.sin(((lon2 - lon1) * r) / 2) ** 2;
  return 2 * 6_371_008.8 * Math.asin(Math.sqrt(h));
}

describe('circle', () => {
  it('trace un anneau fermé au bon rayon', () => {
    const ring = circle(-17.5, -149.5, 50);
    expect(ring).toHaveLength(CIRCLE_STEPS + 1);
    expect(ring[0]).toEqual(ring.at(-1));
    for (const p of ring) {
      expect(haversine([-149.5, -17.5], p)).toBeCloseTo(50, 3);
    }
  });

  it("reste continu près de l'antiméridien", () => {
    const ring = circle(0, 179.9999, 1_000);
    const lons = ring.map((p) => p[0] ?? 0);
    expect(Math.max(...lons) - Math.min(...lons)).toBeLessThan(0.1);
  });
});

describe('anchorData', () => {
  it('donne le cercle, la limite de retour et l’ancre', () => {
    const data = anchorData(anchor, false);
    expect(data.features.map((f) => f.properties.role)).toEqual(['circle', 'limit', 'anchor']);
    expect(data.features[2]?.geometry).toEqual({ type: 'Point', coordinates: [-149.5, -17.5] });
    const limit = data.features[1]?.geometry;
    if (limit?.type !== 'Polygon') throw new Error('polygone attendu');
    expect(haversine([-149.5, -17.5], limit.coordinates[0]?.[0] ?? [])).toBeCloseTo(45, 3);
    expect(anchorData(anchor, true).features.every((f) => f.properties.alerting)).toBe(true);
  });

  it('est vide sans mouillage', () => {
    expect(anchorData(null, true).features).toEqual([]);
  });
});

describe('anchorLayers', () => {
  it('dessine toutes les couches depuis la source du mouillage', () => {
    const layers = anchorLayers();
    expect(layers.length).toBe(4);
    for (const l of layers) expect('source' in l && l.source).toBe(ANCHOR_SOURCE);
  });
});
