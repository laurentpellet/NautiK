import { describe, expect, it } from 'vitest';
import type { ChartCatalog, ChartInfo } from '../services/charts';
import {
  BACKGROUND_COLOR,
  buildStyle,
  chartSource,
  escapeHtml,
  initialView,
  maxMapZoom,
  rasterPaint,
  tileUrlTemplate,
} from './style';

const chart = (patch: Partial<ChartInfo> = {}): ChartInfo => ({
  id: 'tahiti',
  name: 'Tahiti',
  fileName: 'tahiti.mbtiles',
  description: null,
  format: 'png',
  minZoom: 5,
  maxZoom: 11,
  bounds: { west: -150, south: -18, east: -149, north: -17 },
  center: null,
  attribution: null,
  warnings: [],
  ...patch,
});

const catalog = (charts: ChartInfo[]): ChartCatalog => ({
  chartsDir: '/charts',
  tileUrlBase: 'http://nautik-tiles.localhost',
  charts,
  failures: [],
  error: null,
});

describe('escapeHtml', () => {
  it('neutralise le HTML des attributions', () => {
    expect(escapeHtml(`<img src=x onerror="alert('x')">&`)).toBe(
      '&lt;img src=x onerror=&quot;alert(&#39;x&#39;)&quot;&gt;&amp;',
    );
  });
});

describe('sources et couches', () => {
  it('construit le gabarit de tuiles local', () => {
    expect(tileUrlTemplate('http://nautik-tiles.localhost', chart())).toBe(
      'http://nautik-tiles.localhost/tahiti/{z}/{x}/{y}',
    );
  });

  it('reporte zooms, emprise et attribution échappée', () => {
    const source = chartSource('nautik-tiles://localhost', chart({ attribution: '<b>SHOM</b>' }));
    expect(source.minzoom).toBe(5);
    expect(source.maxzoom).toBe(11);
    expect(source.bounds).toEqual([-150, -18, -149, -17]);
    expect(source.attribution).toBe('&lt;b&gt;SHOM&lt;/b&gt;');
  });

  it("omet l'emprise et l'attribution absentes", () => {
    const source = chartSource('x', chart({ bounds: null }));
    expect(source.bounds).toBeUndefined();
    expect(source.attribution).toBeUndefined();
  });

  it('ordonne fond puis cartes dans l’ordre du catalogue', () => {
    const style = buildStyle(catalog([chart({ id: 'a' }), chart({ id: 'b' })]), 'day');
    expect(style.layers.map((l) => l.id)).toEqual(['background', 'chart-a', 'chart-b']);
    expect(Object.keys(style.sources)).toEqual(['chart-a', 'chart-b']);
    expect(style).not.toHaveProperty('glyphs');
    expect(style).not.toHaveProperty('sprite');
  });

  it('applique le thème nuit', () => {
    const style = buildStyle(catalog([chart()]), 'night');
    expect(style.layers[0]).toMatchObject({
      paint: { 'background-color': BACKGROUND_COLOR.night },
    });
    expect(rasterPaint('night')['raster-brightness-max']).toBeLessThan(
      Number(rasterPaint('day')['raster-brightness-max']),
    );
  });

  it('style sans carte : fond seul', () => {
    const style = buildStyle(catalog([]), 'day');
    expect(style.layers).toHaveLength(1);
  });
});

describe('vue initiale et zoom', () => {
  it('cadre la carte la plus détaillée', () => {
    const view = initialView(catalog([chart({ id: 'a', bounds: null }), chart({ id: 'b' })]));
    expect(view).toEqual({
      kind: 'bounds',
      bounds: [
        [-150, -18],
        [-149, -17],
      ],
    });
  });

  it('utilise le centre à défaut d’emprise', () => {
    const view = initialView(
      catalog([chart({ bounds: null, center: { longitude: 1, latitude: 2, zoom: null } })]),
    );
    expect(view).toEqual({ kind: 'center', center: [1, 2], zoom: 5 });
  });

  it('vue monde sans carte', () => {
    expect(initialView(catalog([]))).toEqual({ kind: 'center', center: [0, 0], zoom: 1 });
  });

  it('autorise un sur-zoom limité', () => {
    expect(maxMapZoom(catalog([]))).toBe(18);
    expect(maxMapZoom(catalog([chart({ maxZoom: 11 }), chart({ maxZoom: 14 })]))).toBe(17);
    expect(maxMapZoom(catalog([chart({ maxZoom: 23 })]))).toBe(24);
  });
});
