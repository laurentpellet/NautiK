import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './engine';
import type { Bounds } from './weather';
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
  /** Base CM93 trouvée (chemin relatif, « . » : le dossier lui-même). */
  cm93: string | null;
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
    cm93: optional(r.cm93, (v) => expectString(v, 'inventaire.cm93')),
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
  /** Mises à jour (.001…) appliquées. */
  updatesApplied: number;
  /** Mises à jour présentes mais non appliquées. */
  updatesPending: number;
  /** Date (AAAAMMJJ) de la dernière mise à jour appliquée. */
  updateIssueDate: string | null;
  /** Raison de l'arrêt des mises à jour (manquante, refusée…). */
  updateError: string | null;
  /** Date (AAAAMMJJ) d'annulation de la cellule par son producteur. */
  cancelled: string | null;
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
    updatesApplied: expectInteger(r.updatesApplied, 'résumé.updatesApplied', 0, 999),
    updatesPending: expectInteger(r.updatesPending, 'résumé.updatesPending', 0, 999),
    updateIssueDate: optional(r.updateIssueDate, (v) => expectString(v, 'résumé.updateIssueDate')),
    updateError: optional(r.updateError, (v) => expectString(v, 'résumé.updateError')),
    cancelled: optional(r.cancelled, (v) => expectString(v, 'résumé.cancelled')),
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

/** Cellule de l'index cartographique (miroir de `nav_charts::EncMapCell`). */
export interface EncMapCell {
  path: string;
  name: string;
  /** 1 vue d'ensemble … 6 accostage ; 0 si illisible. */
  usage: number;
  scale: number;
  /** [ouest, sud, est, nord]. */
  bounds: [number, number, number, number] | null;
  updatesApplied: number;
  updatesPending: number;
  /** Cellule pas à jour : raison. */
  updateError: string | null;
  /** Cellule annulée (date) : jamais affichée. */
  cancelled: string | null;
  /** Cellule illisible. */
  error: string | null;
}

/** Index cartographique du dossier ENC. */
export interface EncMapIndex {
  dir: string | null;
  cells: EncMapCell[];
  truncated: boolean;
  /** Dossier illisible ou absent. */
  error: string | null;
  /** Base CM93 du dossier (chemin relatif), s'il y en a une. */
  cm93: string | null;
  /** Base CM93 illisible. */
  cm93Error: string | null;
}

/** Cellules CM93 d'une vue (miroir de `nav_charts::Cm93View`). */
export interface Cm93View {
  base: string | null;
  /** De la moins à la plus détaillée ; `usage` : rang d'échelle 1 à 8. */
  cells: EncMapCell[];
  /** Cellules de la vue non dessinées faute de place. */
  omitted: number;
  truncated: boolean;
}

/** Entités d'une cellule CM93 et ce qu'il faut en signaler. */
export interface Cm93CellData {
  features: EncFeature[];
  /** AAAAMMJJ de la donnée la plus récente. */
  dataDate: string | null;
  /** Décalage WGS 84 déclaré, non appliqué (mètres). */
  declaredOffsetM: number;
}

/** Catégories d'affichage (miroir de `nav_charts::s57::display`). */
export const ENC_CATEGORIES = [
  'land',
  'depth',
  'unsurveyed',
  'contour',
  'coast',
  'sounding',
  'danger',
  'light',
  'buoy',
  'beacon',
] as const;
export type EncCategory = (typeof ENC_CATEGORIES)[number];

export interface EncFeatureProperties {
  cat: EncCategory;
  class: string;
  usage: number;
  cell: string;
  depth?: number;
  label?: string;
  drval1?: number;
  drval2?: number;
  valdco?: number;
  name?: string;
  colour?: number;
}

export type EncGeometry =
  | { type: 'Point'; coordinates: [number, number] }
  | { type: 'MultiLineString'; coordinates: [number, number][][] }
  | { type: 'Polygon'; coordinates: [number, number][][] };

export interface EncFeature {
  type: 'Feature';
  geometry: EncGeometry;
  properties: EncFeatureProperties;
}

/** Nombre maximal d'entités acceptées pour une cellule. */
export const MAX_CELL_FEATURES = 200_000;

function parseMapCell(value: unknown): EncMapCell {
  const r = expectRecord(value, 'cellule ENC');
  return {
    path: expectString(r.path, 'cellule.path'),
    name: expectString(r.name, 'cellule.name'),
    usage: expectInteger(r.usage, 'cellule.usage', 0, 255),
    scale: expectInteger(r.scale, 'cellule.scale', 0, 2 ** 32),
    bounds: optional(r.bounds, parseBounds),
    updatesApplied: expectInteger(r.updatesApplied, 'cellule.updatesApplied', 0, 999),
    updatesPending: expectInteger(r.updatesPending, 'cellule.updatesPending', 0, 999),
    updateError: optional(r.updateError, (v) => expectString(v, 'cellule.updateError')),
    cancelled: optional(r.cancelled, (v) => expectString(v, 'cellule.cancelled')),
    error: optional(r.error, (v) => expectString(v, 'cellule.error')),
  };
}

export function parseEncMapIndex(value: unknown): EncMapIndex {
  const r = expectRecord(value, 'index ENC');
  const index = optional(r.index, (v) => expectRecord(v, 'index ENC.index'));
  return {
    dir: optional(r.dir, (v) => expectString(v, 'index.dir')),
    cells: index ? expectArray(index.cells, 'index.cells', parseMapCell) : [],
    truncated: index ? expectBoolean(index.truncated, 'index.truncated') : false,
    error: optional(r.error, (v) => expectString(v, 'index.error')),
    cm93: optional(r.cm93, (v) => expectString(v, 'index.cm93')),
    cm93Error: optional(r.cm93Error, (v) => expectString(v, 'index.cm93Error')),
  };
}

export function parseCm93View(value: unknown): Cm93View {
  const r = expectRecord(value, 'vue CM93');
  return {
    base: optional(r.base, (v) => expectString(v, 'vue.base')),
    cells: expectArray(r.cells, 'vue.cells', parseMapCell),
    omitted: expectInteger(r.omitted, 'vue.omitted', 0, MAX_COUNT),
    truncated: expectBoolean(r.truncated, 'vue.truncated'),
  };
}

export function parseCm93CellData(value: unknown): Cm93CellData {
  const r = expectRecord(value, 'cellule CM93');
  const date = optional(r.dataDate, (v) => expectString(v, 'cellule.dataDate'));
  if (date !== null && !/^\d{8}$/.test(date)) throw new Error('cellule.dataDate invalide');
  const offset = expectNumber(r.declaredOffsetM, 'cellule.declaredOffsetM');
  if (offset < 0) throw new Error('cellule.declaredOffsetM négatif');
  return {
    features: parseEncFeatures(r.features),
    dataDate: date,
    declaredOffsetM: offset,
  };
}

function parsePosition(value: unknown): [number, number] {
  if (!Array.isArray(value) || value.length !== 2) throw new Error('position ENC invalide');
  const [lon, lat] = value as unknown[];
  if (
    typeof lon !== 'number' ||
    typeof lat !== 'number' ||
    !(Math.abs(lon) <= 180) ||
    !(Math.abs(lat) <= 90)
  ) {
    throw new Error('position ENC hors limites');
  }
  return [lon, lat];
}

function parseLines(value: unknown): [number, number][][] {
  if (!Array.isArray(value)) throw new Error('géométrie ENC invalide');
  return value.map((line) => {
    if (!Array.isArray(line) || line.length < 2) throw new Error('ligne ENC invalide');
    return line.map(parsePosition);
  });
}

function parseGeometry(value: unknown): EncGeometry {
  const r = expectRecord(value, 'géométrie ENC');
  switch (r.type) {
    case 'Point':
      return { type: 'Point', coordinates: parsePosition(r.coordinates) };
    case 'MultiLineString':
      return { type: 'MultiLineString', coordinates: parseLines(r.coordinates) };
    case 'Polygon':
      return { type: 'Polygon', coordinates: parseLines(r.coordinates) };
    default:
      throw new Error(`géométrie ENC « ${String(r.type)} » non prise en charge`);
  }
}

const optionalNumber = (v: unknown, what: string) =>
  v === undefined ? undefined : expectNumber(v, what);
const optionalString = (v: unknown, what: string) =>
  v === undefined ? undefined : expectString(v, what);

function parseFeature(value: unknown): EncFeature {
  const r = expectRecord(value, 'entité ENC');
  if (r.type !== 'Feature') throw new Error('entité ENC invalide');
  const p = expectRecord(r.properties, 'entité.properties');
  const cat = expectString(p.cat, 'entité.cat');
  if (!(ENC_CATEGORIES as readonly string[]).includes(cat)) {
    throw new Error(`catégorie ENC « ${cat} » inconnue`);
  }
  const properties: EncFeatureProperties = {
    cat: cat as EncCategory,
    class: expectString(p.class, 'entité.class'),
    usage: expectInteger(p.usage, 'entité.usage', 0, 255),
    cell: expectString(p.cell, 'entité.cell'),
  };
  const numbers = ['depth', 'drval1', 'drval2', 'valdco'] as const;
  for (const key of numbers) {
    const n = optionalNumber(p[key], `entité.${key}`);
    if (n !== undefined) properties[key] = n;
  }
  const label = optionalString(p.label, 'entité.label');
  if (label !== undefined) properties.label = label.slice(0, 8);
  const name = optionalString(p.name, 'entité.name');
  if (name !== undefined) properties.name = name;
  if (p.colour !== undefined) properties.colour = expectInteger(p.colour, 'entité.colour', 0, 255);
  return { type: 'Feature', geometry: parseGeometry(r.geometry), properties };
}

export function parseEncFeatures(value: unknown): EncFeature[] {
  if (!Array.isArray(value)) throw new Error('entités ENC : tableau attendu');
  if (value.length > MAX_CELL_FEATURES) throw new Error('entités ENC trop nombreuses');
  return value.map(parseFeature);
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
  /** Index cartographique des cellules (emprises, état des mises à jour). */
  async mapIndex(): Promise<EncMapIndex> {
    return parseEncMapIndex(await this.invokeFn('get_enc_map_index'));
  }

  /** Entités à afficher d'une cellule de l'index ; refusé si annulée. */
  async cellFeatures(path: string): Promise<EncFeature[]> {
    return parseEncFeatures(await this.invokeFn('get_enc_cell_features', { path }));
  }

  /** Cellules CM93 à dessiner pour la vue. */
  async cm93View(view: Bounds): Promise<Cm93View> {
    const bounds = [view.west, view.south, view.east, view.north];
    return parseCm93View(await this.invokeFn('get_cm93_view', { view: bounds }));
  }

  /** Entités d'une cellule CM93 de la vue. */
  async cm93Features(path: string): Promise<Cm93CellData> {
    return parseCm93CellData(await this.invokeFn('get_cm93_cell_features', { path }));
  }
}
