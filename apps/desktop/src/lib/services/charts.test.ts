import { describe, expect, it, vi } from 'vitest';
import { ChartsApi, fetchChartCatalog, parseChartCatalog, parseChartInfo } from './charts';
import { EngineProtocolError } from './validate';

export const validChart = {
  id: 'tahiti',
  name: 'Tahiti',
  fileName: 'tahiti.mbtiles',
  description: null,
  format: 'png',
  minZoom: 5,
  maxZoom: 11,
  bounds: { west: -150, south: -18, east: -149, north: -17 },
  center: { longitude: -149.5, latitude: -17.5, zoom: 8 },
  attribution: 'Test',
  warnings: [],
};

export const validCatalog = {
  chartsDir: 'C:\\charts',
  chartsDirSource: 'default',
  tileUrlBase: 'http://nautik-tiles.localhost',
  charts: [validChart],
  failures: [{ fileName: 'bad.mbtiles', reason: 'corrompu' }],
  error: null,
};

describe('parseChartInfo', () => {
  it('accepte une carte valide', () => {
    expect(parseChartInfo(validChart)).toEqual(validChart);
  });

  it('accepte les champs optionnels nuls', () => {
    const chart = parseChartInfo({ ...validChart, bounds: null, center: null, attribution: null });
    expect(chart.bounds).toBeNull();
    expect(chart.center).toBeNull();
  });

  it.each([
    ['identifiant non sûr', { id: '../x' }],
    ['format vectoriel', { format: 'pbf' }],
    ['zoom hors limites', { maxZoom: 30 }],
    ['zoom max < zoom min', { minZoom: 8, maxZoom: 3 }],
    ['zoom non entier', { minZoom: 1.5 }],
    ['emprise incomplète', { bounds: { west: 0 } }],
    ['avertissements mal typés', { warnings: 'x' }],
    ['nom manquant', { name: undefined }],
  ])('rejette : %s', (_label, patch) => {
    expect(() => parseChartInfo({ ...validChart, ...patch })).toThrow(EngineProtocolError);
  });
});

describe('parseChartCatalog', () => {
  it('accepte un catalogue valide', () => {
    expect(parseChartCatalog(validCatalog)).toEqual(validCatalog);
  });

  it('accepte le schéma non Windows', () => {
    const c = parseChartCatalog({ ...validCatalog, tileUrlBase: 'nautik-tiles://localhost' });
    expect(c.tileUrlBase).toBe('nautik-tiles://localhost');
  });

  it('rejette une URL de tuiles distante', () => {
    expect(() =>
      parseChartCatalog({ ...validCatalog, tileUrlBase: 'https://tiles.example.com' }),
    ).toThrow(/non locale/);
  });

  it('transmet une erreur de répertoire', () => {
    const c = parseChartCatalog({ ...validCatalog, charts: [], error: 'introuvable' });
    expect(c.error).toBe('introuvable');
  });

  it.each([
    null,
    [],
    'x',
    { ...validCatalog, charts: null },
    { ...validCatalog, chartsDirSource: 'cloud' },
    { ...validCatalog, chartsDirSource: undefined },
  ])('rejette %s', (raw) => {
    expect(() => parseChartCatalog(raw)).toThrow(EngineProtocolError);
  });
});

describe('fetchChartCatalog', () => {
  it('appelle get_chart_catalog', async () => {
    const call = vi.fn().mockResolvedValue(validCatalog);
    await expect(fetchChartCatalog(call)).resolves.toEqual(validCatalog);
    expect(call).toHaveBeenCalledWith('get_chart_catalog');
  });
});

describe('ChartsApi', () => {
  const chosen = {
    ...validCatalog,
    chartsDir: 'C:\\Users\\x\\Documents\\Charts',
    chartsDirSource: 'settings',
  };

  it('choisit un dossier sans transmettre de chemin', async () => {
    const call = vi.fn().mockResolvedValue(chosen);
    await expect(new ChartsApi(call).chooseDir()).resolves.toEqual(chosen);
    expect(call).toHaveBeenCalledWith('choose_charts_dir');
  });

  it('retourne null si le dialogue est annulé', async () => {
    const call = vi.fn().mockResolvedValue(null);
    await expect(new ChartsApi(call).chooseDir()).resolves.toBeNull();
  });

  it('revient au dossier par défaut et relit le dossier', async () => {
    const call = vi.fn().mockResolvedValue(validCatalog);
    const api = new ChartsApi(call);
    await expect(api.resetDir()).resolves.toEqual(validCatalog);
    await expect(api.reload()).resolves.toEqual(validCatalog);
    expect(call.mock.calls).toEqual([['reset_charts_dir'], ['reload_charts']]);
  });

  it('propage le refus du moteur et rejette une réponse invalide', async () => {
    await expect(
      new ChartsApi(vi.fn().mockRejectedValue('dossier imposé')).chooseDir(),
    ).rejects.toBe('dossier imposé');
    await expect(new ChartsApi(vi.fn().mockResolvedValue({})).resetDir()).rejects.toThrow(
      EngineProtocolError,
    );
  });
});
