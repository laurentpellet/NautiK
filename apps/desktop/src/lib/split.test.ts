import { describe, expect, it } from 'vitest';
import {
  defaultSplit,
  followedCamera,
  isValidCamera,
  loadSplit,
  saveSplit,
  type MapCamera,
} from './split';

const store = (value: string | null) => ({ getItem: () => value });
const cam = (lon: number, lat: number, zoom = 10, bearing = 0): MapCamera => ({
  center: [lon, lat],
  zoom,
  bearing,
});

describe('carte partagée', () => {
  it('démarre désactivée et verrouillée, et tolère un stockage invalide', () => {
    expect(defaultSplit()).toEqual({
      enabled: false,
      locked: true,
      hiddenCharts: [],
      vector: { enc: true, cm93: true },
    });
    expect(loadSplit(store(null))).toEqual(defaultSplit());
    expect(loadSplit(store('{oups'))).toEqual(defaultSplit());
    expect(loadSplit(store('42'))).toEqual(defaultSplit());
    expect(
      loadSplit({
        getItem: () => {
          throw new Error('bloqué');
        },
      }),
    ).toEqual(defaultSplit());
  });

  it('relit le choix mémorisé et écarte les valeurs invalides', () => {
    const state = loadSplit(
      store(
        JSON.stringify({
          enabled: true,
          locked: 'oui',
          hiddenCharts: ['tahiti', 3, '', 'x'.repeat(300), 'monde'],
          vector: { enc: 'non', cm93: false },
        }),
      ),
    );
    expect(state).toEqual({
      enabled: true,
      locked: true,
      hiddenCharts: ['tahiti', 'monde'],
      vector: { enc: true, cm93: false },
    });
    expect(loadSplit(store('{"vector":3}')).vector).toEqual({ enc: true, cm93: true });
    const many = loadSplit(store(JSON.stringify({ hiddenCharts: Array(1000).fill('c') })));
    expect(many.hiddenCharts).toHaveLength(256);
  });

  it('mémorise le choix et tolère un stockage plein', () => {
    let saved = '';
    const state = {
      enabled: true,
      locked: false,
      hiddenCharts: ['tahiti'],
      vector: { enc: false, cm93: true },
    };
    saveSplit(state, { setItem: (_k: string, v: string) => (saved = v) });
    expect(loadSplit(store(saved))).toEqual(state);
    expect(() =>
      saveSplit(state, {
        setItem: () => {
          throw new Error('plein');
        },
      }),
    ).not.toThrow();
  });

  it('valide une vue', () => {
    expect(isValidCamera(cam(-147.7, -15))).toBe(true);
    expect(isValidCamera(cam(Number.NaN, -15))).toBe(false);
    expect(isValidCamera(cam(-147.7, 95))).toBe(false);
    expect(isValidCamera(cam(-147.7, -15, -1))).toBe(false);
    expect(isValidCamera(cam(-147.7, -15, 30))).toBe(false);
    expect(isValidCamera(cam(-147.7, -15, 10, Infinity))).toBe(false);
  });

  it('fait suivre le volet meneur seulement quand les cartes sont verrouillées', () => {
    const left = cam(-147.7, -15, 12, 30);
    const right = cam(-149.5, -17.5, 8);
    const cameras = { left, right };
    const on = { enabled: true, locked: true };
    expect(followedCamera('right', on, 'left', cameras)).toEqual(left);
    expect(followedCamera('left', on, 'left', cameras)).toBeNull();
    // Le volet de droite mène quand l'utilisateur le déplace.
    expect(followedCamera('left', on, 'right', cameras)).toEqual(right);
    expect(followedCamera('right', on, 'right', cameras)).toBeNull();
    // Déverrouillé ou non partagé : chaque carte est libre.
    expect(followedCamera('right', { enabled: true, locked: false }, 'left', cameras)).toBeNull();
    expect(followedCamera('right', { enabled: false, locked: true }, 'left', cameras)).toBeNull();
    // Meneur sans vue connue ou vue invalide : rien à suivre.
    expect(followedCamera('right', on, 'left', { right })).toBeNull();
    expect(followedCamera('right', on, 'left', { left: cam(0, 99) })).toBeNull();
  });
});
