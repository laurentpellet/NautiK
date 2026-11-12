import { describe, expect, it, vi } from 'vitest';
import { fetchChartCatalog, parseChartCatalog, parseChartInfo } from './charts';
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

  it.each([null, [], 'x', { ...validCatalog, charts: null }])('rejette %s', (raw) => {
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
