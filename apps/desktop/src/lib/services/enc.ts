import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './engine';
import {
  expectArray,
  expectBoolean,
  expectInteger,
  expectNumber,
  expectRecord,
  expectString,
  optional,
} from './validate';

/** Cellule ENC S-57 (miroir de `nav_charts::EncCell`). */
export interface EncCell {
  name: string;
  /** Chemin relatif au dossier ENC. */
  path: string;
  sizeBytes: number;
  /** Fichiers de mise à jour (.001, .002…) trouvés. */
  updates: number;
}

/** Inventaire d'un dossier ENC (miroir de `nav_charts::EncInventory`). */
export interface EncInventory {
  cells: EncCell[];
  /** Catalogues CATALOG.031 trouvés. */
  catalogs: number;
  unreadableDirs: number;
  /** Parcours borné : inventaire incomplet. */
  truncated: boolean;
}

/** Dossier ENC choisi et son inventaire. */
export interface EncFolder {
  dir: string | null;
  inventory: EncInventory | null;
  /** Dossier illisible ou absent. */
  error: string | null;
}

const MAX_COUNT = 1_000_000;

function parseCell(value: unknown): EncCell {
  const r = expectRecord(value, 'cellule ENC');
  return {
    name: expectString(r.name, 'cellule.name'),
    path: expectString(r.path, 'cellule.path'),
    sizeBytes: expectInteger(r.sizeBytes, 'cellule.sizeBytes', 0, Number.MAX_SAFE_INTEGER),
    updates: expectInteger(r.updates, 'cellule.updates', 0, MAX_COUNT),
  };
}

function parseInventory(value: unknown): EncInventory {
  const r = expectRecord(value, 'inventaire ENC');
  return {
    cells: expectArray(r.cells, 'inventaire.cells', parseCell),
    catalogs: expectInteger(r.catalogs, 'inventaire.catalogs', 0, MAX_COUNT),
    unreadableDirs: expectInteger(r.unreadableDirs, 'inventaire.unreadableDirs', 0, MAX_COUNT),
    truncated: expectBoolean(r.truncated, 'inventaire.truncated'),
  };
}

/** Résumé d'une cellule décodée (miroir de `nav_charts::S57CellSummary`). */
export interface EncCellSummary {
  datasetName: string;
  edition: string;
  updateNumber: string;
  /** AAAAMMJJ. */
  issueDate: string;
  /** 1 vue d'ensemble … 6 accostage. */
  intendedUsage: number;
  /** Dénominateur de l'échelle de compilation. */
  compilationScale: number;
  features: number;
  /** Nombre d'objets par classe (acronyme S-57 ou code). */
  classes: Record<string, number>;
  soundings: number;
  /** [ouest, sud, est, nord] en degrés. */
  bounds: [number, number, number, number] | null;
  /** Objets écartés (géométrie incohérente). */
  skippedFeatures: number;
  problems: string[];
}

function parseBounds(value: unknown): [number, number, number, number] {
  const list = expectArray(value, 'résumé.bounds', (v) => expectNumber(v, 'résumé.bounds[]'));
  if (list.length !== 4) throw new Error('résumé.bounds : 4 valeurs attendues');
  const [w, s, e, n] = list as [number, number, number, number];
  if (Math.abs(s) > 90 || Math.abs(n) > 90 || Math.abs(w) > 180 || Math.abs(e) > 180) {
    throw new Error('résumé.bounds : hors limites');
  }
  return [w, s, e, n];
}

export function parseEncCellSummary(value: unknown): EncCellSummary {
  const r = expectRecord(value, 'résumé de cellule');
  const rawClasses = expectRecord(r.classes, 'résumé.classes');
  const classes: Record<string, number> = {};
  for (const [name, count] of Object.entries(rawClasses)) {
    classes[name] = expectInteger(count, `résumé.classes.${name}`, 0, MAX_COUNT);
  }
  return {
    datasetName: expectString(r.datasetName, 'résumé.datasetName'),
    edition: expectString(r.edition, 'résumé.edition'),
    updateNumber: expectString(r.updateNumber, 'résumé.updateNumber'),
    issueDate: expectString(r.issueDate, 'résumé.issueDate'),
    intendedUsage: expectInteger(r.intendedUsage, 'résumé.intendedUsage', 0, 255),
    compilationScale: expectInteger(r.compilationScale, 'résumé.compilationScale', 0, 2 ** 32),
    features: expectInteger(r.features, 'résumé.features', 0, 2 ** 32),
    classes,
    soundings: expectInteger(r.soundings, 'résumé.soundings', 0, 2 ** 32),
    bounds: optional(r.bounds, parseBounds),
    skippedFeatures: expectInteger(r.skippedFeatures, 'résumé.skippedFeatures', 0, 2 ** 32),
    problems: expectArray(r.problems, 'résumé.problems', (v) => expectString(v, 'problème')),
  };
}

const USAGES = ["vue d'ensemble", 'générale', 'côtière', 'approche', 'port', 'accostage'];

/** Usage prévu d'une cellule (INTU), en clair. */
export function usageLabel(usage: number): string {
  return USAGES[usage - 1] ?? `usage ${usage}`;
}

/** Date S-57 AAAAMMJJ → JJ/MM/AAAA (texte d'origine si mal formée). */
export function formatIssueDate(date: string): string {
  const m = /^(\d{4})(\d{2})(\d{2})$/.exec(date);
  return m ? `${m[3]}/${m[2]}/${m[1]}` : date;
}

export function parseEncFolder(value: unknown): EncFolder {
  const r = expectRecord(value, 'dossier ENC');
  return {
    dir: optional(r.dir, (v) => expectString(v, 'dossier.dir')),
    inventory: optional(r.inventory, parseInventory),
    error: optional(r.error, (v) => expectString(v, 'dossier.error')),
  };
}

/**
 * Dossier des cartes ENC. Le dossier est choisi par le dialogue du système
 * côté moteur : aucun chemin n'est transmis par l'interface.
 */
export class EncApi {
  constructor(private readonly invokeFn: InvokeFn = invoke) {}

  /** Dossier mémorisé, relu et inventorié. */
  async get(): Promise<EncFolder> {
    return parseEncFolder(await this.invokeFn('get_enc_folder'));
  }

  /** Ouvre le dialogue ; `null` si l'utilisateur annule. */
  async chooseDir(): Promise<EncFolder | null> {
    const value = await this.invokeFn('choose_enc_dir');
    return value === null ? null : parseEncFolder(value);
  }

  /** Oublie le dossier (les fichiers ne sont pas touchés). */
  async clearDir(): Promise<EncFolder> {
    return parseEncFolder(await this.invokeFn('clear_enc_dir'));
  }

  /** Décode une cellule de l'inventaire (chemin relatif, vérifié par le moteur). */
  async inspectCell(path: string): Promise<EncCellSummary> {
    return parseEncCellSummary(await this.invokeFn('inspect_enc_cell', { path }));
  }
}
