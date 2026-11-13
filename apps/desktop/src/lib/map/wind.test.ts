import { describe, expect, it } from 'vitest';
import {
  WIND_ICON,
  WIND_LAYER,
  WIND_SCALE,
  viewBounds,
  windData,
  windIcon,
  windLayers,
} from './wind';

describe('windData', () => {
  it('oriente la flèche vers où souffle le vent, vitesse en nœuds', () => {
    const data = windData([
      { lat: -16, lon: -148, speedMps: 5.144, directionRad: Math.PI / 2 },
      { lat: -15, lon: -147, speedMps: 0, directionRad: 0 },
    ]);
    expect(data.features).toHaveLength(2);
    // Vent d'est : la flèche pointe vers l'ouest.
    expect(data.features[0]?.properties).toEqual({ speedKn: 10, rotation: 270 });
    expect(data.features[0]?.geometry.coordinates).toEqual([-148, -16]);
    // Vent du nord : la flèche pointe vers le sud.
    expect(data.features[1]?.properties.rotation).toBe(180);
  });

  it('donne une collection vide sans vent', () => {
    expect(windData(null).features).toEqual([]);
    expect(windData([]).features).toEqual([]);
  });
});

describe('windLayers', () => {
  it('colore selon toute l’échelle et utilise l’icône vent', () => {
    const [layer] = windLayers();
    expect(layer?.id).toBe(WIND_LAYER);
    const json = JSON.stringify(layer);
    expect(json).toContain(WIND_ICON);
    for (const [, color] of WIND_SCALE) expect(json).toContain(color);
  });

  it('dessine une icône carrée non vide', () => {
    const { id, image } = windIcon();
    expect(id).toBe(WIND_ICON);
    expect(image.width).toBe(image.height);
    expect(image.data.some((v) => v > 0)).toBe(true);
  });
});

describe('viewBounds', () => {
  it('conserve une emprise ordinaire', () => {
    expect(viewBounds(-150, -18, -146, -14)).toEqual({
      west: -150,
      south: -18,
      east: -146,
      north: -14,
    });
  });

  it("ramène les longitudes et signale l'antiméridien", () => {
    expect(viewBounds(170, -20, 190, -10)).toEqual({
      west: 170,
      south: -20,
      east: -170,
      north: -10,
    });
    expect(viewBounds(-200, 0, -190, 10)).toEqual({ west: 160, south: 0, east: 170, north: 10 });
  });

  it('couvre le globe quand la vue fait le tour', () => {
    expect(viewBounds(-400, -100, 400, 100)).toEqual({
      west: -180,
      south: -90,
      east: 180,
      north: 90,
    });
  });
});
