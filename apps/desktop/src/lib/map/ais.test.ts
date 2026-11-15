import { describe, expect, it } from 'vitest';
import type { AisSnapshot, AisTarget } from '../services/ais';
import {
  AIS_HEADING_ICON,
  AIS_LABEL_LAYER,
  AIS_ROUND_ICON,
  AIS_SOURCE,
  AIS_SYMBOL_LAYER,
  AIS_VESSEL_ICON,
  HEADING_ICON_SIZE,
  aisData,
  aisIcons,
  aisLayers,
  aisThemeChanges,
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
      hasHeading: false,
      heading: 0,
      label: 'ARANUI 5',
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

  it('trace la ligne de cap seulement si le cap est déclaré, et nomme la cible', () => {
    const [withHeading] = aisData(
      snapshot([target({ headingRad: Math.PI / 4, history: [], vectorEnd: null })]),
    ).features;
    expect(withHeading?.properties).toMatchObject({ hasHeading: true, label: 'ARANUI 5' });
    expect(withHeading?.properties.heading).toBeCloseTo(45);
    const [noName] = aisData(
      snapshot([target({ name: null, history: [], vectorEnd: null })]),
    ).features;
    expect(noName?.properties).toMatchObject({ hasHeading: false, label: 'MMSI 227000001' });
    // Ligne de cap : pleine vers le nord (au-delà de la proue), vide au sud.
    const heading = aisIcons().find((i) => i.id === AIS_HEADING_ICON)?.image;
    const h = HEADING_ICON_SIZE / 2;
    const alpha = (x: number, y: number) => heading?.data[(y * HEADING_ICON_SIZE + x) * 4 + 3] ?? 0;
    expect(alpha(h, 12)).toBeGreaterThan(150);
    expect(alpha(h, HEADING_ICON_SIZE - 12)).toBe(0);
    expect(alpha(h + 12, 12)).toBe(0);
    // Étiquette au thème de la carte.
    expect(aisThemeChanges('night')[0]?.[0]).toBe(AIS_LABEL_LAYER);
    expect(JSON.stringify(aisThemeChanges('night')[0]?.[1])).toContain('night:ais:');
  });

  it('oriente le symbole selon le cap, sinon la route fond', () => {
    expect(targetRotation(target({ headingRad: Math.PI }))).toBeCloseTo(180);
    expect(targetRotation(target())).toBeCloseTo(90);
    expect(targetRotation(target({ cogRad: null }))).toBeNull();
  });

  it('déclare des couches sur la source AIS, symboles au-dessus', () => {
    const layers = aisLayers();
    expect(layers.every((l) => 'source' in l && l.source === AIS_SOURCE)).toBe(true);
    const ids = layers.map((l) => l.id);
    expect(ids.at(-1)).toBe(AIS_LABEL_LAYER);
    expect(ids.indexOf(AIS_SYMBOL_LAYER)).toBe(ids.length - 2);
    expect(ids.indexOf('nautik-ais-heading')).toBeLessThan(ids.indexOf(AIS_SYMBOL_LAYER));
  });

  it('génère des icônes SDF pleines au centre et vides aux bords', () => {
    const icons = aisIcons();
    expect(icons.map((i) => i.id)).toEqual([AIS_VESSEL_ICON, AIS_ROUND_ICON, AIS_HEADING_ICON]);
    for (const { image } of icons) {
      const size = image.width;
      expect(image.data).toHaveLength(size * size * 4);
      const alpha = (x: number, y: number) => image.data[(y * size + x) * 4 + 3] ?? 0;
      expect(alpha(size / 2, size / 2)).toBeGreaterThan(192);
      expect(alpha(0, 0)).toBe(0);
    }
  });
});
