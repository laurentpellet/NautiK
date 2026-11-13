import { describe, expect, it } from 'vitest';
import { DEFAULT_LAYERS, hiddenLayers, loadLayers, saveLayers } from './layers';

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
