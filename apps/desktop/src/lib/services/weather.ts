/** Fichiers météo GRIB et champs de vent (moteur Rust). */
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { InvokeFn } from './engine';
import type { ListenFn } from './position';
import {
  EngineProtocolError,
  expectArray,
  expectInteger,
  expectNumber,
  expectRecord,
  expectString,
  expectTimestamp,
  optional,
} from './validate';

/** Zone (degrés) ; `west > east` : antiméridien traversé. */
export interface Bounds {
  west: number;
  south: number;
  east: number;
  north: number;
}

export interface WeatherFile {
  id: string;
  sizeBytes: number;
  referenceTime: number | null;
  validTimes: number[];
  bounds: Bounds | null;
  skipped: string[];
  error: string | null;
}

export interface WindVector {
  lat: number;
  lon: number;
  speedMps: number;
  /** Direction d'où vient le vent (rad, [0, 2π)). */
  directionRad: number;
}

export interface WindField {
  id: string;
  referenceTime: number;
  validTime: number;
  step: number;
  steps: number;
  points: WindVector[];
}

export interface DownloadProgress {
  done: number;
  total: number;
}

/** Horizons proposés au téléchargement (h). */
export const HORIZONS_H = [24, 48, 72, 120] as const;
/** Zone maximale acceptée par le moteur (degrés). */
export const MAX_AREA_WIDTH_DEG = 120;
export const MAX_AREA_HEIGHT_DEG = 90;
/** Âge au-delà duquel une prévision est signalée ancienne. */
export const OLD_FORECAST_MS = 24 * 3600 * 1000;

export const DOWNLOAD_EVENT = 'weather-download';

export function parseBounds(value: unknown, what = 'zone'): Bounds {
  const r = expectRecord(value, what);
  const b = {
    west: expectNumber(r.west, 'west'),
    south: expectNumber(r.south, 'south'),
    east: expectNumber(r.east, 'east'),
    north: expectNumber(r.north, 'north'),
  };
  if (Math.abs(b.south) > 90 || Math.abs(b.north) > 90 || b.south >= b.north) {
    throw new EngineProtocolError(`${what} invalide`);
  }
  return b;
}

export function parseWeatherFile(value: unknown): WeatherFile {
  const r = expectRecord(value, 'fichier météo');
  return {
    id: expectString(r.id, 'id'),
    sizeBytes: expectInteger(r.sizeBytes, 'sizeBytes', 0, Number.MAX_SAFE_INTEGER),
    referenceTime: optional(r.referenceTime, (v) => expectTimestamp(v, 'referenceTime')),
    validTimes: expectArray(r.validTimes, 'validTimes', (v) => expectTimestamp(v, 'validTime')),
    bounds: optional(r.bounds, (v) => parseBounds(v)),
    skipped: expectArray(r.skipped, 'skipped', (v) => expectString(v, 'skipped')),
    error: optional(r.error, (v) => expectString(v, 'error')),
  };
}

function parseVector(value: unknown): WindVector {
  const r = expectRecord(value, 'vent');
  const v = {
    lat: expectNumber(r.lat, 'lat'),
    lon: expectNumber(r.lon, 'lon'),
    speedMps: expectNumber(r.speedMps, 'speedMps'),
    directionRad: expectNumber(r.directionRad, 'directionRad'),
  };
  if (Math.abs(v.lat) > 90 || Math.abs(v.lon) > 180 || v.speedMps < 0) {
    throw new EngineProtocolError('vecteur de vent hors bornes');
  }
  return v;
}

export function parseWindField(value: unknown): WindField {
  const r = expectRecord(value, 'champ de vent');
  return {
    id: expectString(r.id, 'id'),
    referenceTime: expectTimestamp(r.referenceTime, 'referenceTime'),
    validTime: expectTimestamp(r.validTime, 'validTime'),
    step: expectInteger(r.step, 'step', 0, 10_000),
    steps: expectInteger(r.steps, 'steps', 1, 10_000),
    points: expectArray(r.points, 'points', parseVector),
  };
}

export function parseProgress(value: unknown): DownloadProgress {
  const r = expectRecord(value, 'progression');
  return {
    done: expectInteger(r.done, 'done', 0, 10_000),
    total: expectInteger(r.total, 'total', 1, 10_000),
  };
}

/** Date et heure UTC lisibles, ex. « jeu. 13 nov. 09:00 UTC ». */
export function formatUtc(t: number): string {
  const d = new Date(t);
  const day = new Intl.DateTimeFormat('fr-FR', {
    timeZone: 'UTC',
    weekday: 'short',
    day: 'numeric',
    month: 'short',
  }).format(d);
  return `${day} ${d.toISOString().slice(11, 16)} UTC`;
}

/** Zone en clair, ex. « 20°S–13°S, 152°O–144°O ». */
export function formatArea(b: Bounds): string {
  const lat = (v: number) => `${Math.abs(Math.round(v))}°${v < 0 ? 'S' : 'N'}`;
  const lon = (v: number) => `${Math.abs(Math.round(v))}°${v < 0 ? 'O' : 'E'}`;
  return `${lat(b.south)}–${lat(b.north)}, ${lon(b.west)}–${lon(b.east)}`;
}

/** Zone téléchargeable : celle affichée, réduite si besoin autour de son centre. */
export function downloadArea(view: Bounds): Bounds {
  const width = (((view.east - view.west) % 360) + 360) % 360 || 360;
  const height = view.north - view.south;
  if (width <= MAX_AREA_WIDTH_DEG && height <= MAX_AREA_HEIGHT_DEG) return view;
  const centerLon = view.west + width / 2;
  const centerLat = (view.north + view.south) / 2;
  const w = Math.min(width, MAX_AREA_WIDTH_DEG) / 2;
  const h = Math.min(height, MAX_AREA_HEIGHT_DEG) / 2;
  const wrap = (lon: number) => ((((lon + 180) % 360) + 360) % 360) - 180;
  return {
    west: wrap(centerLon - w),
    east: wrap(centerLon + w),
    south: Math.max(-90, centerLat - h),
    north: Math.min(90, centerLat + h),
  };
}

export class WeatherApi {
  constructor(private readonly invokeFn: InvokeFn = invoke) {}

  async list(): Promise<WeatherFile[]> {
    return expectArray(await this.invokeFn('list_weather_files'), 'fichiers', parseWeatherFile);
  }

  /** `null` si l'utilisateur annule. */
  async importFile(): Promise<WeatherFile | null> {
    return optional(await this.invokeFn('import_weather_file'), parseWeatherFile);
  }

  async download(area: Bounds, horizonH: number): Promise<WeatherFile> {
    return parseWeatherFile(await this.invokeFn('download_weather', { area, horizonH }));
  }

  async remove(id: string): Promise<void> {
    await this.invokeFn('delete_weather_file', { id });
  }

  async windField(id: string, step: number, area: Bounds | null, maxPoints: number) {
    return parseWindField(await this.invokeFn('get_wind_field', { id, step, area, maxPoints }));
  }
}

export function subscribeDownload(
  onProgress: (p: DownloadProgress) => void,
  listenFn: ListenFn = listen,
): Promise<UnlistenFn> {
  return listenFn(DOWNLOAD_EVENT, (e) => {
    try {
      onProgress(parseProgress(e.payload));
    } catch {
      // Progression illisible : sans conséquence sur le téléchargement.
    }
  });
}
