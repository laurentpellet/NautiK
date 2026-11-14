import { describe, expect, it } from 'vitest';
import {
  DEFAULT_LAYERS,
  MAX_HIDDEN_CHARTS,
  hiddenChartNames,
  hiddenLayers,
  loadHiddenCharts,
  loadLayers,
  saveHiddenCharts,
  saveLayers,
  toggleChart,
} from './layers';

const store = (value: string | null) => ({ getItem: () => value });

describe('calques', () => {
  it('affiche tout par défaut ou si le stockage est invalide', () => {
    expect(loadLayers(store(null))).toEqual(DEFAULT_LAYERS);
    expect(loadLayers(store('{pas du json'))).toEqual(DEFAULT_LAYERS);
    expect(loadLayers(store('42'))).toEqual(DEFAULT_LAYERS);
    expect(
      loadLayers({
        getItem: () => {
          throw new Error('bloqué');
        },
      }),
    ).toEqual(DEFAULT_LAYERS);
  });

  it('relit un choix mémorisé et ignore les valeurs non booléennes', () => {
    expect(loadLayers(store('{"ais":false,"routes":"non"}'))).toEqual({
      routes: true,
      tracks: true,
      ais: false,
    });
  });

  it('mémorise et liste les calques masqués', () => {
    let saved = '';
    saveLayers({ routes: true, tracks: false, ais: false }, { setItem: (_k, v) => (saved = v) });
    expect(JSON.parse(saved)).toEqual({ routes: true, tracks: false, ais: false });
    expect(hiddenLayers({ routes: true, tracks: false, ais: false })).toEqual([
      'Traces',
      'Cibles AIS',
    ]);
  });
});

describe('cartes masquées', () => {
  const charts = [
    { id: 'monde', name: 'Monde' },
    { id: 'tahiti', name: 'Tahiti' },
  ];

  it('affiche tout par défaut ou si le stockage est invalide', () => {
    expect(loadHiddenCharts(store(null))).toEqual([]);
    expect(loadHiddenCharts(store('{pas du json'))).toEqual([]);
    expect(loadHiddenCharts(store('{"a":1}'))).toEqual([]);
    expect(
      loadHiddenCharts({
        getItem: () => {
          throw new Error('bloqué');
        },
      }),
    ).toEqual([]);
  });

  it('relit un choix mémorisé sans doublon ni valeur invalide', () => {
    expect(loadHiddenCharts(store('["tahiti", 3, "tahiti", null, "monde"]'))).toEqual([
      'tahiti',
      'monde',
    ]);
    expect(loadHiddenCharts(store(JSON.stringify(['x'.repeat(65)])))).toEqual([]);
    const many = JSON.stringify(Array.from({ length: 400 }, (_, i) => `c${i}`));
    expect(loadHiddenCharts(store(many))).toHaveLength(MAX_HIDDEN_CHARTS);
  });

  it('mémorise le choix et tolère un stockage indisponible', () => {
    let saved = '';
    saveHiddenCharts(['tahiti'], { setItem: (_k: string, v: string) => (saved = v) });
    expect(saved).toBe('["tahiti"]');
    expect(() =>
      saveHiddenCharts(['tahiti'], {
        setItem: () => {
          throw new Error('plein');
        },
      }),
    ).not.toThrow();
  });

  it('bascule une carte et nomme les cartes masquées', () => {
    const hidden = toggleChart([], 'tahiti', false);
    expect(hidden).toEqual(['tahiti']);
    expect(toggleChart(hidden, 'tahiti', false)).toEqual(['tahiti']);
    expect(toggleChart(hidden, 'tahiti', true)).toEqual([]);
    expect(hiddenChartNames(charts, ['tahiti', 'absente'])).toEqual(['Tahiti']);
  });
});
