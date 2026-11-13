import { describe, expect, it } from 'vitest';
import type { AisSnapshot, AisTarget } from '../services/ais';
import {
  AIS_ROUND_ICON,
  AIS_SOURCE,
  AIS_SYMBOL_LAYER,
  AIS_VESSEL_ICON,
  ICON_SIZE,
  aisData,
  aisIcons,
  aisLayers,
  targetRotation,
} from './ais';

const p = (latitudeDeg: number, longitudeDeg: number) => ({ latitudeDeg, longitudeDeg });

const target = (patch: Partial<AisTarget> = {}): AisTarget => ({
  mmsi: '227000001',
  class: 'A',
  name: 'ARANUI 5',
  callsign: null,
  shipType: 70,
  navigationStatus: 0,
  position: p(-14.97, -147.64),
  sogMps: 6,
  cogRad: Math.PI / 2,
  headingRad: null,
  vectorEnd: p(-14.97, -147.62),
  lastSeenAt: 0,
  positionAgeMs: 1000,
  state: 'active',
  history: [p(-14.98, -147.65), p(-14.975, -147.645)],
  ...patch,
});

const snapshot = (targets: AisTarget[]): AisSnapshot => ({
  computedAt: 0,
  simulated: false,
  targets,
});

describe('couche AIS', () => {
  it('produit historique, vecteur et symbole en [lon, lat]', () => {
    const data = aisData(snapshot([target()]));
    expect(data.features.map((f) => f.properties.kind)).toEqual(['history', 'vector', 'target']);
    expect(data.features[0]?.geometry).toEqual({
      type: 'LineString',
      coordinates: [
        [-147.65, -14.98],
        [-147.645, -14.975],
        [-147.64, -14.97],
      ],
    });
    expect(data.features[1]?.geometry.coordinates).toEqual([
      [-147.64, -14.97],
      [-147.62, -14.97],
    ]);
    expect(data.features[2]?.properties).toEqual({
      kind: 'target',
      mmsi: '227000001',
      aisClass: 'A',
      state: 'active',
      directed: true,
      rotation: 90,
    });
  });

  it("ignore les cibles sans position et n'invente ni vecteur ni direction", () => {
    const data = aisData(
      snapshot([
        target({ position: null, state: 'lost' }),
        target({
          mmsi: '227000002',
          class: 'B',
          cogRad: null,
          headingRad: null,
          vectorEnd: null,
          history: [],
          state: 'stale',
        }),
      ]),
    );
    expect(data.features).toHaveLength(1);
    expect(data.features[0]?.properties).toMatchObject({
      mmsi: '227000002',
      aisClass: 'B',
      state: 'stale',
      directed: false,
    });
    expect(aisData(null).features).toEqual([]);
  });

  it('oriente le symbole selon le cap, sinon la route fond', () => {
    expect(targetRotation(target({ headingRad: Math.PI }))).toBeCloseTo(180);
    expect(targetRotation(target())).toBeCloseTo(90);
    expect(targetRotation(target({ cogRad: null }))).toBeNull();
  });

  it('déclare des couches sur la source AIS, symboles au-dessus', () => {
    const layers = aisLayers();
    expect(layers.every((l) => 'source' in l && l.source === AIS_SOURCE)).toBe(true);
    expect(layers.at(-1)?.id).toBe(AIS_SYMBOL_LAYER);
  });

  it('génère des icônes SDF pleines au centre et vides aux bords', () => {
    const icons = aisIcons();
    expect(icons.map((i) => i.id)).toEqual([AIS_VESSEL_ICON, AIS_ROUND_ICON]);
    for (const { image } of icons) {
      expect(image.data).toHaveLength(ICON_SIZE * ICON_SIZE * 4);
      const alpha = (x: number, y: number) => image.data[(y * ICON_SIZE + x) * 4 + 3];
      expect(alpha(ICON_SIZE / 2, ICON_SIZE / 2)).toBeGreaterThan(192);
      expect(alpha(0, 0)).toBe(0);
    }
  });
});
