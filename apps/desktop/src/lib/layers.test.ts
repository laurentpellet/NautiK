import { describe, expect, it } from 'vitest';
import {
  visibleRoutes,
  DEFAULT_LAYERS,
  MAX_HIDDEN_CHARTS,
  hiddenChartNames,
  hiddenLayers,
  layerLabel,
  loadHiddenCharts,
  loadLayers,
  saveHiddenCharts,
  saveLayers,
  toggleChart,
  vectorKeys,
  vectorLabel,
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
      enc: true,
      cm93: true,
      poi: true,
      marks: true,
      vectors: true,
      laylines: true,
    });
    expect(loadLayers(store('{"enc":false}')).enc).toBe(false);
    expect(loadLayers(store('{"poi":false}')).poi).toBe(false);
    expect(loadLayers(store('{"cm93":false}')).cm93).toBe(false);
    expect(loadLayers(store('{"laylines":false}')).laylines).toBe(false);
  });

  it('mémorise et liste les calques masqués', () => {
    let saved = '';
    const layers = {
      routes: true,
      tracks: false,
      ais: false,
      enc: false,
      cm93: false,
      poi: false,
      marks: true,
      vectors: false,
      laylines: true,
    };
    saveLayers(layers, { setItem: (_k, v) => (saved = v) });
    expect(JSON.parse(saved)).toEqual(layers);
    expect(hiddenLayers(layers)).toEqual([
      'Traces',
      'Cibles AIS',
      'Cartes ENC (S-57)',
      'Îles et amers (OSM)',
      'Vecteur et ligne de cap',
    ]);
    // CM93 rappelée seulement si une base est trouvée.
    expect(hiddenLayers(layers, true)).toContain('Cartes CM93');
  });

  it('propose et résume les cartes vectorielles', () => {
    expect(vectorKeys(false)).toEqual(['enc']);
    expect(vectorKeys(true)).toEqual(['enc', 'cm93']);
    expect(vectorLabel({ enc: true, cm93: true }, true)).toEqual(['ENC', 'CM93']);
    expect(vectorLabel({ enc: true, cm93: true }, false)).toEqual(['ENC']);
    expect(vectorLabel({ enc: false, cm93: true }, true)).toEqual(['CM93']);
    expect(vectorLabel({ enc: false, cm93: false }, true)).toEqual([]);
  });

  it('utilise le nom choisi pour la CM93', () => {
    expect(layerLabel('cm93')).toBe('Cartes CM93');
    expect(layerLabel('cm93', 'C-Map 2008')).toBe('C-Map 2008');
    expect(layerLabel('enc', 'C-Map 2008')).toBe('Cartes ENC (S-57)');
    expect(vectorLabel({ enc: true, cm93: true }, true, 'C-Map')).toEqual(['ENC', 'C-Map']);
    expect(vectorLabel({ enc: false, cm93: true }, false, 'C-Map')).toEqual([]);
    const layers = { ...DEFAULT_LAYERS, cm93: false };
    expect(hiddenLayers(layers, true, 'C-Map')).toEqual(['C-Map']);
    expect(hiddenLayers(layers, false, 'C-Map')).toEqual([]);
  });

  it("ne propose pas les ENC s'il n'y a aucune cellule", () => {
    expect(vectorKeys(true, false)).toEqual(['cm93']);
    expect(vectorKeys(false, false)).toEqual([]);
    expect(vectorLabel({ enc: true, cm93: true }, true, null, false)).toEqual(['CM93']);
    const layers = { ...DEFAULT_LAYERS, enc: false };
    expect(hiddenLayers(layers, true, null, false)).toEqual([]);
    expect(hiddenLayers(layers, true, null, true)).toEqual(['Cartes ENC (S-57)']);
  });
});

describe('routes affichées', () => {
  const routes = [
    { id: 'a', active: false },
    { id: 'b', active: true },
    { id: 'c', active: false },
  ];

  it('affiche toutes les routes quand le calque est visible', () => {
    expect(visibleRoutes(routes, { routes: true })).toEqual(routes);
  });

  it('garde la route active quand le calque est masqué', () => {
    expect(visibleRoutes(routes, { routes: false }).map((r) => r.id)).toEqual(['b']);
    expect(visibleRoutes([{ id: 'a', active: false }], { routes: false })).toEqual([]);
    expect(visibleRoutes([], { routes: false })).toEqual([]);
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
