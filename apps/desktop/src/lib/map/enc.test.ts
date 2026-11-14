import { describe, expect, it } from 'vitest';
import type { EncFeature, EncMapCell } from '../services/enc';
import {
  ENC_PALETTE,
  ENC_SOURCE,
  SAFETY_CONTOUR_M,
  boundsIntersect,
  cellsForView,
  depthColor,
  encCollection,
  encLayers,
  encThemeChanges,
  parseSoundingImageId,
  soundingImageExpression,
} from './enc';

function cell(name: string, usage: number, patch: Partial<EncMapCell> = {}): EncMapCell {
  return {
    path: `${name}/${name}.000`,
    name,
    usage,
    scale: 10_000,
    bounds: [-71, 42, -70, 43],
    updatesApplied: 0,
    updatesPending: 0,
    updateError: null,
    cancelled: null,
    error: null,
    ...patch,
  };
}

const view = { west: -70.8, south: 42.2, east: -70.2, north: 42.8 };

describe('cellules de la vue', () => {
  it('teste le recouvrement, antiméridien compris', () => {
    expect(boundsIntersect([-71, 42, -70, 43], view)).toBe(true);
    expect(boundsIntersect([10, 42, 11, 43], view)).toBe(false);
    expect(boundsIntersect([-71, 50, -70, 51], view)).toBe(false);
    // Bords communs : recouvrement.
    expect(boundsIntersect([-70.2, 42, -70, 43], view)).toBe(true);
    const across = { west: 170, south: -20, east: -170, north: -10 };
    expect(boundsIntersect([175, -15, 176, -14], across)).toBe(true);
    expect(boundsIntersect([-175, -15, -174, -14], across)).toBe(true);
    expect(boundsIntersect([0, -15, 1, -14], across)).toBe(false);
  });

  it('écarte les cellules illisibles, annulées ou sans emprise', () => {
    const cells = [
      cell('OK', 3),
      cell('ERR', 3, { error: 'illisible' }),
      cell('ANN', 3, { cancelled: '2022-12-30' }),
      cell('VIDE', 3, { bounds: null }),
      cell('LOIN', 3, { bounds: [10, 10, 11, 11] }),
    ];
    expect(cellsForView(cells, view).map((c) => c.name)).toEqual(['OK']);
    expect(cellsForView(cells, null)).toEqual([]);
  });

  it('retient les plus détaillées et les trace par-dessus', () => {
    const cells = [cell('B', 5), cell('A', 2), cell('C', 4), cell('D', 5)];
    expect(cellsForView(cells, view).map((c) => c.name)).toEqual(['A', 'C', 'B', 'D']);
    expect(cellsForView(cells, view, 2).map((c) => c.name)).toEqual(['B', 'D']);
    expect(cellsForView(cells, view, -1)).toEqual([]);
  });

  it('assemble les entités dans l’ordre de tracé', () => {
    const feature = (name: string): EncFeature => ({
      type: 'Feature',
      geometry: { type: 'Point', coordinates: [-70.5, 42.5] },
      properties: { cat: 'sounding', class: 'SOUNDG', usage: 3, cell: name, depth: 4 },
    });
    const a = cell('A', 2);
    const b = cell('B', 5);
    const features = new Map([
      [b.path, [feature('B')]],
      [a.path, [feature('A')]],
    ]);
    const data = encCollection([a, b, cell('ABSENTE', 4)], features);
    expect(data.features.map((f) => f.properties.cell)).toEqual(['A', 'B']);
  });
});

describe('symbologie ENC', () => {
  it('colore les zones de profondeur, inconnue comme petits fonds', () => {
    const expr = depthColor('day');
    expect(expr[0]).toBe('step');
    expect(expr[1]).toEqual(['coalesce', ['get', 'drval1'], 0]);
    expect(expr).toContain(SAFETY_CONTOUR_M);
    expect(expr[2]).toBe(ENC_PALETTE.day.drying);
    expect(depthColor('night')[2]).toBe(ENC_PALETTE.night.drying);
  });

  it('décrit des couches sur la source ENC, sondes depuis un zoom', () => {
    const layers = encLayers('day');
    const ids = layers.map((l) => l.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids[0]).toBe('enc-depth');
    expect(ids.indexOf('enc-land')).toBeGreaterThan(ids.indexOf('enc-depth'));
    expect(ids.indexOf('enc-danger-point')).toBeGreaterThan(ids.indexOf('enc-sounding'));
    for (const layer of layers) {
      expect('source' in layer && layer.source).toBe(ENC_SOURCE);
    }
    const sounding = layers.find((l) => l.id === 'enc-sounding');
    expect(sounding?.minzoom).toBe(12);
  });

  it('change toutes les propriétés au changement de thème', () => {
    const night = encThemeChanges('night');
    const land = night.find(([id, , p]) => id === 'enc-land' && p === 'fill-color');
    expect(land?.[3]).toBe(ENC_PALETTE.night.land);
    const icon = night.find(([id, kind]) => id === 'enc-sounding' && kind === 'layout');
    expect(icon?.[3]).toEqual(soundingImageExpression('night'));
    expect(encThemeChanges('day').length).toBe(night.length);
  });

  it('reconnaît les identifiants d’images de sondes', () => {
    expect(parseSoundingImageId('enc-snd-day-shallow-4.5')).toEqual({
      theme: 'day',
      shallow: true,
      label: '4.5',
    });
    expect(parseSoundingImageId('enc-snd-night-deep-123')).toEqual({
      theme: 'night',
      shallow: false,
      label: '123',
    });
    expect(parseSoundingImageId('enc-snd-day-shallow--1.2')?.label).toBe('-1.2');
    for (const id of [
      'enc-snd-day-shallow-',
      'enc-snd-dusk-deep-4',
      'enc-snd-day-deep-4.55',
      'enc-snd-day-deep-12345',
      'ais-vessel',
      'enc-snd-day-deep-4<script>',
    ]) {
      expect(parseSoundingImageId(id)).toBeNull();
    }
  });
});
