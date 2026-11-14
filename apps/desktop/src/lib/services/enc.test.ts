import { describe, expect, it } from 'vitest';
import { EncApi, formatIssueDate, parseEncCellSummary, parseEncFolder, usageLabel } from './enc';

const folder = {
  dir: 'D:\\ENC_ROOT',
  inventory: {
    cells: [{ name: 'FR501130', path: 'FR501130/1/0/FR501130.000', sizeBytes: 1200, updates: 2 }],
    catalogs: 1,
    unreadableDirs: 0,
    truncated: false,
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
  };

  it('lit un résumé valide, avec ou sans emprise', () => {
    expect(parseEncCellSummary(summary)).toEqual(summary);
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
