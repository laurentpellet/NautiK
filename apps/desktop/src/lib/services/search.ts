/**
 * Recherche de lieux (moteur) : coordonnées saisies, marques, waypoints,
 * îles et amers téléchargés. Tout est local.
 */
import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './engine';
import {
  EngineProtocolError,
  expectArray,
  expectPosition,
  expectRecord,
  expectString,
  optional,
  type LatLon,
} from './validate';

export const PLACE_KINDS = ['coordinates', 'mark', 'waypoint', 'routePoint', 'poi'] as const;
export type PlaceKind = (typeof PLACE_KINDS)[number];

export const PLACE_KIND_LABELS: Record<PlaceKind, string> = {
  coordinates: 'Coordonnées',
  mark: 'Marque',
  waypoint: 'Waypoint',
  routePoint: 'Route',
  poi: 'Île / amer',
};

/** Longueur maximale d'une requête, comme le moteur. */
export const MAX_QUERY_CHARS = 80;

export interface Place {
  kind: PlaceKind;
  name: string;
  detail: string | null;
  position: LatLon;
}

function isPlaceKind(value: unknown): value is PlaceKind {
  return typeof value === 'string' && (PLACE_KINDS as readonly string[]).includes(value);
}

export function parsePlace(value: unknown): Place {
  const r = expectRecord(value, 'lieu');
  if (!isPlaceKind(r.kind)) throw new EngineProtocolError('lieu.kind : nature inconnue');
  return {
    kind: r.kind,
    name: expectString(r.name, 'lieu.name'),
    detail: optional(r.detail, (v) => expectString(v, 'lieu.detail')),
    position: expectPosition(r.position, 'lieu.position'),
  };
}

export async function searchPlaces(query: string, call: InvokeFn = invoke): Promise<Place[]> {
  return expectArray(await call('search_places', { query }), 'résultats', parsePlace);
}
