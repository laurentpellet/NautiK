import { describe, expect, it } from 'vitest';
import {
  chartChoice,
  defaultSplit,
  effectiveRightChart,
  followedCamera,
  rightChartOptions,
  rightPaneCharts,
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
    expect(defaultSplit()).toEqual({ enabled: false, locked: true, chart: null });
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
          chart: 'chart:tahiti',
          // Ancien format (cases à cocher) : ignoré.
          hiddenCharts: ['monde'],
        }),
      ),
    );
    expect(state).toEqual({ enabled: true, locked: true, chart: 'chart:tahiti' });
    for (const chart of [3, '', 'x'.repeat(301), null]) {
      expect(loadSplit(store(JSON.stringify({ chart }))).chart).toBeNull();
    }
  });

  it('mémorise le choix et tolère un stockage plein', () => {
    let saved = '';
    const state = {
      enabled: true,
      locked: false,
      chart: 'cm93',
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

  it('propose à droite les cartes non affichées à gauche', () => {
    const ids = ['tahiti', 'moorea', 'monde'];
    expect(
      rightChartOptions(ids, ['monde', 'tahiti'], ['enc', 'cm93'], { enc: true, cm93: false }),
    ).toEqual([chartChoice('tahiti'), chartChoice('monde'), 'cm93']);
    // Tout est affiché à gauche : rien à proposer.
    expect(rightChartOptions(ids, [], ['cm93'], { enc: false, cm93: true })).toEqual([]);
    // Carte vectorielle indisponible : jamais proposée.
    expect(rightChartOptions([], [], [], { enc: false, cm93: false })).toEqual([]);
  });

  it('garde le choix proposé, sinon prend la première carte proposée', () => {
    const options = [chartChoice('tahiti'), 'cm93'];
    expect(effectiveRightChart('cm93', options)).toBe('cm93');
    expect(effectiveRightChart(chartChoice('monde'), options)).toBe(chartChoice('tahiti'));
    expect(effectiveRightChart(null, options)).toBe(chartChoice('tahiti'));
    expect(effectiveRightChart('cm93', [])).toBeNull();
  });

  it('affiche à droite la seule carte choisie', () => {
    const ids = ['tahiti', 'monde'];
    expect(rightPaneCharts(chartChoice('monde'), ids)).toEqual({
      hiddenCharts: ['tahiti'],
      vector: { enc: false, cm93: false },
    });
    expect(rightPaneCharts('cm93', ids)).toEqual({
      hiddenCharts: ['tahiti', 'monde'],
      vector: { enc: false, cm93: true },
    });
    expect(rightPaneCharts('enc', []).vector).toEqual({ enc: true, cm93: false });
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
