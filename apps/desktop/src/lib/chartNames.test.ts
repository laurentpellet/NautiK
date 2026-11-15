import { describe, expect, it } from 'vitest';
import {
  MAX_CHART_NAME_CHARS,
  chartNames,
  chartSort,
  cm93Name,
  loadChartSort,
  setChartSort,
  sortCharts,
  loadCm93Name,
  setCm93Name,
  cleanChartName,
  loadChartNames,
  parseChartNames,
  setChartName,
  withChartNames,
} from './chartNames.svelte';
import type { ChartInfo } from './services/charts';

const memory = () => {
  const data = new Map<string, string>();
  return {
    data,
    getItem: (k: string) => data.get(k) ?? null,
    setItem: (k: string, v: string) => void data.set(k, v),
  };
};

const chart = (id: string, name: string) => ({ id, name, fileName: `${id}.mbtiles` }) as ChartInfo;

describe('noms des cartes', () => {
  it('nettoie un nom : espaces, longueur, vide', () => {
    expect(cleanChartName('  Tahiti   Moorea ')).toBe('Tahiti Moorea');
    expect(cleanChartName('   ')).toBeNull();
    expect(cleanChartName('x'.repeat(100))).toHaveLength(MAX_CHART_NAME_CHARS);
  });

  it('valide les noms mémorisés', () => {
    expect(parseChartNames({ a: 'Lagon', b: 3, c: '  ', '': 'x' })).toEqual({ a: 'Lagon' });
    expect(parseChartNames(['a'])).toEqual({});
    expect(parseChartNames(null)).toEqual({});
    const storage = memory();
    storage.data.set('nautik.chartNames', '{illisible');
    expect(loadChartNames(storage)).toEqual({});
    expect(
      loadChartNames({
        getItem: () => {
          throw new Error('bloqué');
        },
      }),
    ).toEqual({});
  });

  it('nomme, mémorise et revient au nom d’origine', () => {
    const storage = memory();
    setChartName('tahiti', ' Îles du Vent ', storage);
    expect(chartNames.names).toEqual({ tahiti: 'Îles du Vent' });
    expect(loadChartNames(storage)).toEqual({ tahiti: 'Îles du Vent' });
    const charts = [chart('tahiti', 'Tahiti 2019'), chart('moorea', 'Moorea')];
    expect(withChartNames(charts, chartNames.names).map((c) => c.name)).toEqual([
      'Îles du Vent',
      'Moorea',
    ]);
    setChartName('tahiti', '', storage);
    expect(chartNames.names).toEqual({});
    expect(withChartNames(charts, chartNames.names)[0]?.name).toBe('Tahiti 2019');
    // Stockage indisponible : le nom change quand même.
    setChartName('moorea', 'M', {
      setItem: () => {
        throw new Error('plein');
      },
    });
    expect(chartNames.names).toEqual({ moorea: 'M' });
    setChartName('moorea', '', undefined);
  });

  it('nomme la base CM93 et revient au nom par défaut', () => {
    const storage = { ...memory(), removeItem: (k: string) => void storage.data.delete(k) };
    expect(cm93Name.name).toBeNull();
    setCm93Name('  C-Map   2008 ', storage);
    expect(cm93Name.name).toBe('C-Map 2008');
    expect(loadCm93Name(storage)).toBe('C-Map 2008');
    setCm93Name('   ', storage);
    expect(cm93Name.name).toBeNull();
    expect(storage.data.has('nautik.cm93Name')).toBe(false);
    expect(loadCm93Name(storage)).toBeNull();
    // Valeur mémorisée trop longue ou stockage bloqué.
    storage.data.set('nautik.cm93Name', 'x'.repeat(100));
    expect(loadCm93Name(storage)).toHaveLength(MAX_CHART_NAME_CHARS);
    expect(
      loadCm93Name({
        getItem: () => {
          throw new Error('bloqué');
        },
      }),
    ).toBeNull();
    setCm93Name('C', {
      setItem: () => {
        throw new Error('plein');
      },
      removeItem: () => undefined,
    });
    expect(cm93Name.name).toBe('C');
    setCm93Name('', undefined);
  });
});

describe('tri des cartes', () => {
  const charts = [
    { id: 'monde', name: 'Monde', fileName: 'z-monde.mbtiles' },
    { id: 'tahiti', name: 'tahiti', fileName: 'b-tahiti.mbtiles' },
    { id: 'port', name: 'Port 10', fileName: 'a-port.mbtiles' },
  ] as ChartInfo[];
  const ids = (list: ChartInfo[]) => list.map((c) => c.id);

  it('trie par détail, nom (choisi ou d’origine) ou fichier, sans modifier la liste', () => {
    expect(ids(sortCharts(charts, 'detail'))).toEqual(['port', 'tahiti', 'monde']);
    expect(ids(sortCharts(charts, 'name'))).toEqual(['monde', 'port', 'tahiti']);
    expect(ids(sortCharts(charts, 'name', { monde: 'Zone large' }))).toEqual([
      'port',
      'tahiti',
      'monde',
    ]);
    expect(ids(sortCharts(charts, 'file'))).toEqual(['port', 'tahiti', 'monde']);
    expect(ids(charts)).toEqual(['monde', 'tahiti', 'port']);
  });

  it('mémorise un tri valide et ignore le reste', () => {
    const storage = memory();
    expect(loadChartSort(storage)).toBe('detail');
    setChartSort('name', storage);
    expect(chartSort.value).toBe('name');
    expect(loadChartSort(storage)).toBe('name');
    setChartSort('n’importe quoi', storage);
    expect(chartSort.value).toBe('name');
    storage.data.set('nautik.chartSort', 'taille');
    expect(loadChartSort(storage)).toBe('detail');
    expect(
      loadChartSort({
        getItem: () => {
          throw new Error('bloqué');
        },
      }),
    ).toBe('detail');
    setChartSort('detail', undefined);
  });
});
