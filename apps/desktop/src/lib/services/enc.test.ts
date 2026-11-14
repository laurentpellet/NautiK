import { describe, expect, it } from 'vitest';
import {
  EncApi,
  formatIssueDate,
  parseEncCellSummary,
  parseEncFeatures,
  parseEncFolder,
  parseCm93CellData,
  parseCm93View,
  parseEncMapIndex,
  usageLabel,
} from './enc';

const folder = {
  dir: 'D:\\ENC_ROOT',
  inventory: {
    cells: [{ name: 'FR501130', path: 'FR501130/1/0/FR501130.000', sizeBytes: 1200, updates: 2 }],
    catalogs: 1,
    unreadableDirs: 0,
    truncated: false,
    cm93: null,
  },
  error: null,
};

describe('dossier ENC', () => {
  it('lit un dossier inventorié, absent ou non choisi', () => {
    expect(parseEncFolder(folder)).toEqual(folder);
    expect(parseEncFolder({ dir: null, inventory: null, error: null })).toEqual({
      dir: null,
      inventory: null,
      error: null,
    });
    const gone = { dir: 'E:\\ENC', inventory: null, error: 'répertoire inaccessible' };
    expect(parseEncFolder(gone)).toEqual(gone);
  });

  it('rejette des données malformées', () => {
    expect(() => parseEncFolder(null)).toThrow();
    expect(() => parseEncFolder({ ...folder, dir: 3 })).toThrow();
    const bad = (cell: Record<string, unknown>) => ({
      ...folder,
      inventory: { ...folder.inventory, cells: [{ ...folder.inventory.cells[0], ...cell }] },
    });
    expect(() => parseEncFolder(bad({ sizeBytes: -1 }))).toThrow();
    expect(() => parseEncFolder(bad({ updates: 1.5 }))).toThrow();
    expect(() => parseEncFolder(bad({ name: null }))).toThrow();
    expect(() =>
      parseEncFolder({ ...folder, inventory: { ...folder.inventory, truncated: 'non' } }),
    ).toThrow();
  });

  it('appelle les commandes du moteur', async () => {
    const calls: string[] = [];
    const api = new EncApi((command: string) => {
      calls.push(command);
      return Promise.resolve(command === 'choose_enc_dir' ? null : folder);
    });
    expect(await api.get()).toEqual(folder);
    expect(await api.chooseDir()).toBeNull();
    expect(await api.clearDir()).toEqual(folder);
    expect(calls).toEqual(['get_enc_folder', 'choose_enc_dir', 'clear_enc_dir']);
  });
});

describe('résumé de cellule ENC', () => {
  const summary = {
    datasetName: 'FR501130.000',
    edition: '3',
    updateNumber: '0',
    issueDate: '20261101',
    intendedUsage: 5,
    compilationScale: 22000,
    features: 10,
    classes: { DEPARE: 4, SOUNDG: 6 },
    soundings: 30,
    bounds: [-149.6, -17.5, -149.5, -17.4],
    skippedFeatures: 0,
    problems: [],
    updatesApplied: 2,
    updatesPending: 0,
    updateIssueDate: '20261105',
    updateError: null,
    cancelled: null,
  };

  it('lit un résumé valide, avec ou sans emprise', () => {
    expect(parseEncCellSummary(summary)).toEqual(summary);
    const cancelled = { ...summary, cancelled: '20221230', updateError: 'refusée' };
    expect(parseEncCellSummary(cancelled)).toEqual(cancelled);
    expect(parseEncCellSummary({ ...summary, bounds: null }).bounds).toBeNull();
  });

  it('rejette un résumé malformé', () => {
    expect(() => parseEncCellSummary(null)).toThrow();
    expect(() => parseEncCellSummary({ ...summary, features: -1 })).toThrow();
    expect(() => parseEncCellSummary({ ...summary, classes: { DEPARE: 'x' } })).toThrow();
    expect(() => parseEncCellSummary({ ...summary, bounds: [1, 2, 3] })).toThrow();
    expect(() => parseEncCellSummary({ ...summary, bounds: [0, 95, 0, 0] })).toThrow();
    expect(() => parseEncCellSummary({ ...summary, bounds: [0, 0, 'e', 0] })).toThrow();
    expect(() => parseEncCellSummary({ ...summary, problems: [1] })).toThrow();
    expect(() => parseEncCellSummary({ ...summary, intendedUsage: 300 })).toThrow();
    expect(() => parseEncCellSummary({ ...summary, updatesApplied: -1 })).toThrow();
    expect(() => parseEncCellSummary({ ...summary, updateError: 3 })).toThrow();
    expect(() => parseEncCellSummary({ ...summary, cancelled: false })).toThrow();
    const missing: Record<string, unknown> = { ...summary };
    delete missing.updatesPending;
    expect(() => parseEncCellSummary(missing)).toThrow();
  });

  it('transmet le chemin relatif de la cellule au moteur', async () => {
    const calls: unknown[] = [];
    const api = new EncApi((command: string, args?: Record<string, unknown>) => {
      calls.push([command, args]);
      return Promise.resolve(summary);
    });
    expect(await api.inspectCell('FR501130/FR501130.000')).toEqual(summary);
    expect(calls).toEqual([['inspect_enc_cell', { path: 'FR501130/FR501130.000' }]]);
  });

  it('met en forme la date et l’usage', () => {
    expect(formatIssueDate('20261101')).toBe('01/11/2026');
    expect(formatIssueDate('2026-11')).toBe('2026-11');
    expect(usageLabel(1)).toBe("vue d'ensemble");
    expect(usageLabel(6)).toBe('accostage');
    expect(usageLabel(0)).toBe('usage 0');
    expect(usageLabel(9)).toBe('usage 9');
  });
});

describe('index et entités ENC pour la carte', () => {
  const mapCell = {
    path: 'US5MA1AM/US5MA1AM.000',
    name: 'US5MA1AM',
    usage: 5,
    scale: 10_000,
    bounds: [-70.9, 42.3, -70.8, 42.4],
    updatesApplied: 7,
    updatesPending: 0,
    updateError: null,
    cancelled: null,
    error: null,
  };

  it('lit l’index, sans dossier ou avec dossier absent', () => {
    expect(
      parseEncMapIndex({
        dir: 'D:\\ENC_ROOT',
        index: { cells: [mapCell], truncated: true },
        error: null,
      }),
    ).toEqual({
      dir: 'D:\\ENC_ROOT',
      cells: [mapCell],
      truncated: true,
      error: null,
      cm93: null,
      cm93Error: null,
    });
    expect(parseEncMapIndex({ dir: null, index: null, error: null })).toEqual({
      dir: null,
      cells: [],
      truncated: false,
      error: null,
      cm93: null,
      cm93Error: null,
    });
    expect(parseEncMapIndex({ dir: 'E:\\', index: null, error: 'absent' }).error).toBe('absent');
  });

  it('refuse un index malformé', () => {
    const bad = (cell: Record<string, unknown>) => () =>
      parseEncMapIndex({ dir: null, index: { cells: [cell], truncated: false }, error: null });
    expect(bad({ ...mapCell, bounds: [0, 95, 1, 96] })).toThrow();
    expect(bad({ ...mapCell, bounds: [0, 1, 2] })).toThrow();
    expect(bad({ ...mapCell, usage: 300 })).toThrow();
    expect(bad({ ...mapCell, updatesApplied: -1 })).toThrow();
    expect(bad({ ...mapCell, path: 3 })).toThrow();
    expect(() => parseEncMapIndex(null)).toThrow();
    expect(() =>
      parseEncMapIndex({ dir: null, index: { cells: 'x', truncated: false } }),
    ).toThrow();
  });

  const feature = (patch: Record<string, unknown> = {}, geometry?: unknown) => ({
    type: 'Feature',
    geometry: geometry ?? { type: 'Point', coordinates: [-70.85, 42.35] },
    properties: { cat: 'sounding', class: 'SOUNDG', usage: 5, cell: 'US5MA1AM', ...patch },
  });

  it('lit les entités et leurs attributs connus', () => {
    const area = {
      type: 'Polygon',
      coordinates: [
        [
          [-70.9, 42.3],
          [-70.8, 42.3],
          [-70.8, 42.4],
          [-70.9, 42.3],
        ],
      ],
    };
    const line = {
      type: 'MultiLineString',
      coordinates: [
        [
          [-70.9, 42.3],
          [-70.8, 42.4],
        ],
      ],
    };
    const [snd, depare, coast, buoy] = parseEncFeatures([
      feature({ depth: 3.2, label: '3.2', inconnu: 'ignoré' }),
      feature({ cat: 'depth', class: 'DEPARE', drval1: 5, drval2: 10 }, area),
      feature({ cat: 'coast', class: 'COALNE' }, line),
      feature({ cat: 'buoy', class: 'BOYLAT', colour: 3, name: 'R "2"' }),
    ]);
    expect(snd?.properties).toEqual({
      cat: 'sounding',
      class: 'SOUNDG',
      usage: 5,
      cell: 'US5MA1AM',
      depth: 3.2,
      label: '3.2',
    });
    expect(depare?.geometry.type).toBe('Polygon');
    expect(depare?.properties.drval1).toBe(5);
    expect(coast?.geometry.type).toBe('MultiLineString');
    expect(buoy?.properties.colour).toBe(3);
    expect(parseEncFeatures([])).toEqual([]);
  });

  it('refuse des entités malformées', () => {
    const bad = (value: unknown) => () => parseEncFeatures([value]);
    expect(() => parseEncFeatures({})).toThrow('tableau');
    expect(bad(feature({ cat: 'inconnue' }))).toThrow('inconnue');
    expect(bad(feature({ depth: 'profond' }))).toThrow();
    expect(bad(feature({ colour: 1.5 }))).toThrow();
    expect(bad({ ...feature(), type: 'Autre' })).toThrow();
    expect(bad(feature({}, { type: 'Point', coordinates: [200, 0] }))).toThrow('hors limites');
    expect(bad(feature({}, { type: 'Point', coordinates: [0, Number.NaN] }))).toThrow();
    expect(bad(feature({}, { type: 'Point', coordinates: [0] }))).toThrow();
    expect(bad(feature({}, { type: 'GeometryCollection', geometries: [] }))).toThrow();
    expect(bad(feature({}, { type: 'MultiLineString', coordinates: [[[0, 0]]] }))).toThrow();
    expect(bad(feature({}, { type: 'Polygon', coordinates: 'x' }))).toThrow();
  });

  it('demande l’index puis les entités d’une cellule', async () => {
    const calls: [string, unknown][] = [];
    const api = new EncApi((command, args) => {
      calls.push([command, args]);
      return Promise.resolve(
        command === 'get_enc_map_index'
          ? { dir: null, index: null, error: null }
          : [feature({ depth: 1 })],
      );
    });
    expect((await api.mapIndex()).cells).toEqual([]);
    expect(await api.cellFeatures('US5MA1AM/US5MA1AM.000')).toHaveLength(1);
    expect(calls).toEqual([
      ['get_enc_map_index', undefined],
      ['get_enc_cell_features', { path: 'US5MA1AM/US5MA1AM.000' }],
    ]);
  });
});

describe('base CM93', () => {
  const feature = (patch: Record<string, unknown> = {}) => ({
    type: 'Feature',
    geometry: { type: 'Point', coordinates: [-149.6, -17.5] },
    properties: { cat: 'sounding', class: 'SOUNDG', usage: 7, cell: '02170631.F', ...patch },
  });
  const cm93Cell = {
    path: '02100600/F/02170631.F',
    name: '02170631.F',
    usage: 7,
    scale: 20_000,
    bounds: [-149.67, -17.67, -149.33, -17.33],
    updatesApplied: 0,
    updatesPending: 0,
    updateError: null,
    cancelled: null,
    error: null,
  };

  it('lit la base signalée par l’inventaire et l’index', () => {
    const withBase = { ...folder, inventory: { ...folder.inventory, cm93: 'CM93' } };
    expect(parseEncFolder(withBase).inventory?.cm93).toBe('CM93');
    const index = parseEncMapIndex({
      dir: 'D:\\ENC_ROOT',
      index: null,
      error: null,
      cm93: '.',
      cm93Error: 'CM93OBJ.DIC illisible',
    });
    expect(index.cm93).toBe('.');
    expect(index.cm93Error).toBe('CM93OBJ.DIC illisible');
    expect(() => parseEncMapIndex({ dir: null, index: null, error: null, cm93: 3 })).toThrow();
  });

  it('lit une vue et une cellule', () => {
    const view = { base: '.', cells: [cm93Cell], omitted: 2, truncated: false };
    expect(parseCm93View(view)).toEqual(view);
    const data = parseCm93CellData({
      features: [feature({ depth: 1 })],
      dataDate: '20140602',
      declaredOffsetM: 42.5,
    });
    expect(data.features).toHaveLength(1);
    expect(data.dataDate).toBe('20140602');
    expect(data.declaredOffsetM).toBe(42.5);
    expect(parseCm93CellData({ features: [], dataDate: null, declaredOffsetM: 0 }).dataDate).toBe(
      null,
    );
  });

  it('refuse une vue ou une cellule malformée', () => {
    const view = { base: null, cells: [cm93Cell], omitted: 0, truncated: false };
    expect(() => parseCm93View({ ...view, omitted: -1 })).toThrow();
    expect(() => parseCm93View({ ...view, truncated: 'non' })).toThrow();
    expect(() =>
      parseCm93View({ ...view, cells: [{ ...cm93Cell, bounds: [0, 0, 200, 1] }] }),
    ).toThrow();
    expect(() => parseCm93View(null)).toThrow();
    const cell = { features: [], dataDate: '20140602', declaredOffsetM: 0 };
    expect(() => parseCm93CellData({ ...cell, dataDate: '2014-06-02' })).toThrow();
    expect(() => parseCm93CellData({ ...cell, declaredOffsetM: 'loin' })).toThrow();
    expect(() => parseCm93CellData({ ...cell, declaredOffsetM: Number.NaN })).toThrow();
    expect(() => parseCm93CellData({ ...cell, declaredOffsetM: -1 })).toThrow('négatif');
    expect(() => parseCm93CellData({ ...cell, features: 'x' })).toThrow();
  });

  it('demande la vue (ouest, sud, est, nord) puis une cellule', async () => {
    const calls: [string, unknown][] = [];
    const api = new EncApi((command, args) => {
      calls.push([command, args]);
      return Promise.resolve(
        command === 'get_cm93_view'
          ? { base: '.', cells: [cm93Cell], omitted: 0, truncated: false }
          : { features: [], dataDate: null, declaredOffsetM: 0 },
      );
    });
    const view = await api.cm93View({ west: -150, south: -18, east: -149, north: -17 });
    expect(view.cells).toHaveLength(1);
    await api.cm93Features(cm93Cell.path);
    expect(calls).toEqual([
      ['get_cm93_view', { view: [-150, -18, -149, -17] }],
      ['get_cm93_cell_features', { path: cm93Cell.path }],
    ]);
  });
});
