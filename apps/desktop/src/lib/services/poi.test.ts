import { describe, expect, it, vi } from 'vitest';
import { PoiApi, parsePoi, parsePoiCollection, parsePoiRegion } from './poi';
import { EngineProtocolError } from './validate';

const rawPoi = {
  osmRef: 'n42',
  kind: 'beacon',
  name: null,
  position: { latitudeDeg: -17.53, longitudeDeg: -149.57 },
  category: 'starboard',
  details: 'green',
};

const rawRegion = {
  id: 'fidji',
  name: 'Fidji',
  boxes: [
    { south: -21.5, west: 176.5, north: -12, east: 180 },
    { south: -21.5, west: -180, north: -15.5, east: -178 },
  ],
  fetchedAt: '2026-11-14T06:00:00Z',
  poiCount: 1234,
};

describe('validation des îles et amers', () => {
  it('accepte des données valides', () => {
    expect(parsePoi(rawPoi).category).toBe('starboard');
    const region = parsePoiRegion(rawRegion);
    expect(region.boxes).toHaveLength(2);
    expect(region.fetchedAt).toBe(Date.UTC(2026, 10, 14, 6));
    expect(parsePoiRegion({ ...rawRegion, fetchedAt: null }).fetchedAt).toBeNull();
    const collection = parsePoiCollection({ pois: [rawPoi], attribution: '© OSM' });
    expect(collection.pois).toHaveLength(1);
  });

  it('rejette les données malformées', () => {
    const bad: unknown[] = [
      { ...rawPoi, osmRef: 'x1' },
      { ...rawPoi, kind: 'volcan' },
      { ...rawPoi, category: '<b>' },
      { ...rawPoi, position: { latitudeDeg: 95, longitudeDeg: 0 } },
      { ...rawPoi, name: 'x'.repeat(121) },
      null,
    ];
    for (const value of bad) expect(() => parsePoi(value)).toThrow(EngineProtocolError);
    expect(() => parsePoiRegion({ ...rawRegion, id: '../x' })).toThrow(EngineProtocolError);
    expect(() =>
      parsePoiRegion({ ...rawRegion, boxes: [{ south: 0, west: 10, north: 1, east: 5 }] }),
    ).toThrow(EngineProtocolError);
    expect(() => parsePoiRegion({ ...rawRegion, poiCount: -1 })).toThrow(EngineProtocolError);
  });

  it('appelle les commandes du moteur', async () => {
    const call = vi.fn((command: string) =>
      Promise.resolve(
        command === 'get_pois' ? { pois: [rawPoi], attribution: '© OSM' } : [rawRegion],
      ),
    );
    const api = new PoiApi(call);
    await api.listRegions();
    await api.download('fidji');
    await api.remove('fidji');
    expect((await api.getPois()).pois[0]?.osmRef).toBe('n42');
    expect(call.mock.calls).toEqual([
      ['list_poi_regions'],
      ['download_pois', { regionId: 'fidji' }],
      ['delete_pois', { regionId: 'fidji' }],
      ['get_pois'],
    ]);
  });
});
