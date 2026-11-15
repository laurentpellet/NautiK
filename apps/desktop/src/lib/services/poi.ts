/**
 * Îles et amers nautiques (OpenStreetMap) : régions, téléchargement à la
 * demande et points enregistrés. Toutes les données reçues du moteur
 * sont validées.
 */
import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './engine';
import {
  EngineProtocolError,
  expectArray,
  expectInteger,
  expectNumber,
  expectPosition,
  expectRecord,
  expectString,
  expectTimestamp,
  optional,
  type LatLon,
} from './validate';

/** Mention obligatoire des données OpenStreetMap (licence ODbL). */
export const POI_ATTRIBUTION = '© contributeurs OpenStreetMap (ODbL)';

export const POI_KINDS = [
  'archipelago',
  'island',
  'islet',
  'pass',
  'beacon',
  'buoy',
  'light',
  'landmark',
  'anchorage',
  'harbour',
] as const;

export type PoiKind = (typeof POI_KINDS)[number];

export const POI_KIND_LABELS: Record<PoiKind, string> = {
  archipelago: 'archipels',
  island: 'îles',
  islet: 'îlots',
  pass: 'passes',
  beacon: 'balises',
  buoy: 'bouées',
  light: 'feux',
  landmark: 'amers',
  anchorage: 'mouillages',
  harbour: 'ports',
};

export interface Poi {
  osmRef: string;
  kind: PoiKind;
  name: string | null;
  position: LatLon;
  category: string | null;
  details: string | null;
}

export interface PoiCollection {
  pois: Poi[];
  attribution: string;
}

export interface PoiBox {
  south: number;
  west: number;
  north: number;
  east: number;
}

export interface PoiRegion {
  id: string;
  name: string;
  boxes: PoiBox[];
  fetchedAt: number | null;
  poiCount: number;
}

const COUNT_MAX = 2 ** 31;
const OSM_REF = /^[nwr]\d{1,20}$/;
const CATEGORY = /^[a-z0-9_;]{1,40}$/;
const REGION_ID = /^[a-z-]{1,40}$/;
const MAX_TEXT = 120;

function text(value: unknown, what: string): string {
  const s = expectString(value, what);
  if ([...s].length > MAX_TEXT) throw new EngineProtocolError(`${what} trop long`);
  return s;
}

export function parsePoi(value: unknown): Poi {
  const r = expectRecord(value, 'amer');
  const osmRef = expectString(r.osmRef, 'amer.osmRef');
  if (!OSM_REF.test(osmRef)) throw new EngineProtocolError(`référence OSM invalide : ${osmRef}`);
  const kind = expectString(r.kind, 'amer.kind');
  if (!(POI_KINDS as readonly string[]).includes(kind)) {
    throw new EngineProtocolError(`nature d'amer inconnue : ${kind}`);
  }
  const category = optional(r.category, (v) => expectString(v, 'amer.category'));
  if (category !== null && !CATEGORY.test(category)) {
    throw new EngineProtocolError(`catégorie invalide : ${category}`);
  }
  return {
    osmRef,
    kind: kind as PoiKind,
    name: optional(r.name, (v) => text(v, 'amer.name')),
    position: expectPosition(r.position, 'amer.position'),
    category,
    details: optional(r.details, (v) => text(v, 'amer.details')),
  };
}

export function parsePoiCollection(value: unknown): PoiCollection {
  const r = expectRecord(value, 'amers');
  return {
    pois: expectArray(r.pois, 'amers.pois', parsePoi),
    attribution: text(r.attribution, 'amers.attribution'),
  };
}

function parseBox(value: unknown): PoiBox {
  const r = expectRecord(value, 'emprise');
  const box = {
    south: expectNumber(r.south, 'emprise.south'),
    west: expectNumber(r.west, 'emprise.west'),
    north: expectNumber(r.north, 'emprise.north'),
    east: expectNumber(r.east, 'emprise.east'),
  };
  const ok =
    box.south >= -90 &&
    box.north <= 90 &&
    box.south < box.north &&
    box.west >= -180 &&
    box.east <= 180 &&
    box.west < box.east;
  if (!ok) throw new EngineProtocolError('emprise de région invalide');
  return box;
}

export function parsePoiRegion(value: unknown): PoiRegion {
  const r = expectRecord(value, 'région');
  const id = expectString(r.id, 'région.id');
  if (!REGION_ID.test(id)) throw new EngineProtocolError(`identifiant de région invalide : ${id}`);
  return {
    id,
    name: text(r.name, 'région.name'),
    boxes: expectArray(r.boxes, 'région.boxes', parseBox),
    fetchedAt: optional(r.fetchedAt, (v) => expectTimestamp(v, 'région.fetchedAt')),
    poiCount: expectInteger(r.poiCount, 'région.poiCount', 0, COUNT_MAX),
  };
}

const parseRegions = (value: unknown) => expectArray(value, 'régions', parsePoiRegion);

/** Client des commandes îles et amers. */
export class PoiApi {
  constructor(private readonly call: InvokeFn = invoke) {}

  async listRegions(): Promise<PoiRegion[]> {
    return parseRegions(await this.call('list_poi_regions'));
  }

  /** Télécharge (ou met à jour) une région ; accès réseau. */
  async download(regionId: string): Promise<PoiRegion[]> {
    return parseRegions(await this.call('download_pois', { regionId }));
  }

  async remove(regionId: string): Promise<PoiRegion[]> {
    return parseRegions(await this.call('delete_pois', { regionId }));
  }

  async getPois(): Promise<PoiCollection> {
    return parsePoiCollection(await this.call('get_pois'));
  }
}
