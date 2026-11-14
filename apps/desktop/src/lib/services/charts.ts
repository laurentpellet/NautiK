import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './engine';
import {
  EngineProtocolError,
  expectArray,
  expectInteger,
  expectNumber,
  expectRecord,
  expectString,
  optional,
} from './validate';

export type TileFormat = 'png' | 'jpg' | 'webp';

export interface Bounds {
  west: number;
  south: number;
  east: number;
  north: number;
}

export interface Center {
  longitude: number;
  latitude: number;
  zoom: number | null;
}

/** Carte du catalogue (miroir de `nav_charts::ChartInfo`). */
export interface ChartInfo {
  id: string;
  name: string;
  fileName: string;
  description: string | null;
  format: TileFormat;
  minZoom: number;
  maxZoom: number;
  bounds: Bounds | null;
  center: Center | null;
  attribution: string | null;
  warnings: string[];
}

export interface ChartLoadFailure {
  fileName: string;
  reason: string;
}

/** Catalogue des cartes (miroir de `ChartCatalogView`). */
/** Origine du dossier des cartes. */
export type ChartsDirSource = 'environment' | 'settings' | 'default';

export interface ChartCatalog {
  chartsDir: string;
  chartsDirSource: ChartsDirSource;
  tileUrlBase: string;
  charts: ChartInfo[];
  failures: ChartLoadFailure[];
  error: string | null;
}

const CHART_ID = /^[a-z0-9_-]{1,64}$/;
const TILE_URL_BASE = /^(http:\/\/nautik-tiles\.localhost|nautik-tiles:\/\/localhost)$/;
export const MAX_ZOOM = 24;

function parseFormat(value: unknown): TileFormat {
  const format = expectString(value, 'format');
  if (format !== 'png' && format !== 'jpg' && format !== 'webp') {
    throw new EngineProtocolError(`format de tuile inconnu : ${format}`);
  }
  return format;
}

function parseBounds(value: unknown): Bounds {
  const r = expectRecord(value, 'bounds');
  return {
    west: expectNumber(r.west, 'bounds.west'),
    south: expectNumber(r.south, 'bounds.south'),
    east: expectNumber(r.east, 'bounds.east'),
    north: expectNumber(r.north, 'bounds.north'),
  };
}

function parseCenter(value: unknown): Center {
  const r = expectRecord(value, 'center');
  return {
    longitude: expectNumber(r.longitude, 'center.longitude'),
    latitude: expectNumber(r.latitude, 'center.latitude'),
    zoom: optional(r.zoom, (z) => expectInteger(z, 'center.zoom', 0, MAX_ZOOM)),
  };
}

export function parseChartInfo(value: unknown): ChartInfo {
  const r = expectRecord(value, 'carte');
  const id = expectString(r.id, 'id');
  if (!CHART_ID.test(id)) {
    throw new EngineProtocolError(`identifiant de carte invalide : ${id}`);
  }
  const minZoom = expectInteger(r.minZoom, 'minZoom', 0, MAX_ZOOM);
  const maxZoom = expectInteger(r.maxZoom, 'maxZoom', minZoom, MAX_ZOOM);
  return {
    id,
    name: expectString(r.name, 'name'),
    fileName: expectString(r.fileName, 'fileName'),
    description: optional(r.description, (v) => expectString(v, 'description')),
    format: parseFormat(r.format),
    minZoom,
    maxZoom,
    bounds: optional(r.bounds, parseBounds),
    center: optional(r.center, parseCenter),
    attribution: optional(r.attribution, (v) => expectString(v, 'attribution')),
    warnings: expectArray(r.warnings, 'warnings', (v) => expectString(v, 'warning')),
  };
}

function parseDirSource(value: unknown): ChartsDirSource {
  const source = expectString(value, 'chartsDirSource');
  if (source !== 'environment' && source !== 'settings' && source !== 'default') {
    throw new EngineProtocolError(`origine du dossier des cartes inconnue : ${source}`);
  }
  return source;
}

export function parseChartCatalog(value: unknown): ChartCatalog {
  const r = expectRecord(value, 'catalogue');
  const tileUrlBase = expectString(r.tileUrlBase, 'tileUrlBase');
  if (!TILE_URL_BASE.test(tileUrlBase)) {
    throw new EngineProtocolError(`URL de tuiles non locale : ${tileUrlBase}`);
  }
  return {
    chartsDir: expectString(r.chartsDir, 'chartsDir'),
    chartsDirSource: parseDirSource(r.chartsDirSource),
    tileUrlBase,
    charts: expectArray(r.charts, 'charts', parseChartInfo),
    failures: expectArray(r.failures, 'failures', (v) => {
      const f = expectRecord(v, 'échec');
      return {
        fileName: expectString(f.fileName, 'fileName'),
        reason: expectString(f.reason, 'reason'),
      };
    }),
    error: optional(r.error, (v) => expectString(v, 'error')),
  };
}

/** Charge le catalogue des cartes depuis le moteur. */
export async function fetchChartCatalog(call: InvokeFn = invoke): Promise<ChartCatalog> {
  return parseChartCatalog(await call('get_chart_catalog'));
}

/**
 * Choix du dossier des cartes. Le dossier est choisi par le dialogue du
 * système côté moteur : aucun chemin n'est transmis par l'interface.
 */
export class ChartsApi {
  constructor(private readonly invokeFn: InvokeFn = invoke) {}

  /** Ouvre le dialogue ; `null` si l'utilisateur annule. */
  async chooseDir(): Promise<ChartCatalog | null> {
    const value = await this.invokeFn('choose_charts_dir');
    return value === null ? null : parseChartCatalog(value);
  }

  /** Revient au dossier par défaut de l'application. */
  async resetDir(): Promise<ChartCatalog> {
    return parseChartCatalog(await this.invokeFn('reset_charts_dir'));
  }

  /** Relit le dossier (cartes ajoutées ou retirées). */
  async reload(): Promise<ChartCatalog> {
    return parseChartCatalog(await this.invokeFn('reload_charts'));
  }
}
