/**
 * Routes, waypoints, traces, navigation active et GPX.
 *
 * Toutes les données reçues du moteur sont validées ; aucun calcul de
 * navigation n'est effectué ici (distances, XTE et ETA viennent du moteur).
 */
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { InvokeFn } from './engine';
import type { ListenFn } from './position';
import {
  expectArray,
  expectBoolean,
  expectInteger,
  expectNumber,
  expectPosition,
  expectRecord,
  expectString,
  expectTimestamp,
  optional,
  EngineProtocolError,
  type LatLon,
} from './validate';

export type { LatLon } from './validate';

/** Événement de navigation émis par le moteur (`NAVIGATION_EVENT`). */
export const NAVIGATION_EVENT = 'navigation-status';

export interface Waypoint {
  id: string;
  name: string;
  position: LatLon;
  description: string | null;
}

export interface Route {
  id: string;
  name: string;
  description: string | null;
  waypoints: Waypoint[];
  totalDistanceM: number;
  active: boolean;
  /** Emprise (pour zoomer dessus). */
  bounds: GeoBounds | null;
}

export interface TrackSummary {
  id: string;
  name: string;
  startedAt: number;
  endedAt: number | null;
  isRecording: boolean;
  pointCount: number;
}

export interface Track {
  id: string;
  name: string;
  /** Positions dans l'ordre chronologique. */
  points: LatLon[];
  lengthM: number;
  /** Durée entre la première et la dernière position (s). */
  durationS: number | null;
  bounds: GeoBounds | null;
}

/** Emprise en degrés ; `west > east` si elle traverse l'antiméridien. */
export interface GeoBounds {
  west: number;
  south: number;
  east: number;
  north: number;
}

/** Point d'une route en cours d'édition : waypoint existant (`id`) ou nouveau. */
export interface WaypointEdit {
  id: string | null;
  /** Nom ; vide : `WPnnn` attribué par le moteur. */
  name: string;
  position: LatLon;
}

export interface PlannedLeg {
  fromIndex: number;
  distanceM: number;
  bearingRad: number | null;
  durationS: number;
  cumulativeDistanceM: number;
  cumulativeDurationS: number;
  /** Heure d'arrivée au bout du segment (ms UTC), si un départ est donné. */
  arrival: number | null;
}

/** Parcours planifié par le moteur à la vitesse prévue. */
export interface RoutePlan {
  speedMps: number;
  departure: number | null;
  legs: PlannedLeg[];
  totalDistanceM: number;
  totalDurationS: number;
  arrival: number | null;
  bounds: GeoBounds;
}

export interface NavigationStatus {
  routeId: string;
  routeName: string;
  targetIndex: number;
  waypointCount: number;
  targetName: string;
  targetPosition: LatLon;
  distanceToTargetM: number;
  bearingToTargetRad: number | null;
  crossTrackM: number | null;
  remainingDistanceM: number;
  etaTarget: number | null;
  etaEnd: number | null;
  reachedWaypoint: string | null;
  finished: boolean;
  simulated: boolean;
  computedAt: number;
}

export interface NavigationState {
  activeRouteId: string | null;
  recordingTrackId: string | null;
}

export interface ImportReport {
  waypoints: number;
  routes: number;
  tracks: number;
  warnings: string[];
}

const COUNT_MAX = 2 ** 31;
const str = (what: string) => (v: unknown) => expectString(v, what);

export function parseWaypoint(value: unknown): Waypoint {
  const r = expectRecord(value, 'waypoint');
  return {
    id: expectString(r.id, 'waypoint.id'),
    name: expectString(r.name, 'waypoint.name'),
    position: expectPosition(r.position, 'waypoint.position'),
    description: optional(r.description, str('waypoint.description')),
  };
}

export function parseRoute(value: unknown): Route {
  const r = expectRecord(value, 'route');
  return {
    id: expectString(r.id, 'route.id'),
    name: expectString(r.name, 'route.name'),
    description: optional(r.description, str('route.description')),
    waypoints: expectArray(r.waypoints, 'route.waypoints', parseWaypoint),
    totalDistanceM: expectNumber(r.totalDistanceM, 'route.totalDistanceM'),
    active: expectBoolean(r.active, 'route.active'),
    bounds: optional(r.bounds, (v) => parseGeoBounds(v, 'route.bounds')),
  };
}

export function parseTrackSummary(value: unknown): TrackSummary {
  const r = expectRecord(value, 'trace');
  return {
    id: expectString(r.id, 'trace.id'),
    name: expectString(r.name, 'trace.name'),
    startedAt: expectTimestamp(r.startedAt, 'trace.startedAt'),
    endedAt: optional(r.endedAt, (v) => expectTimestamp(v, 'trace.endedAt')),
    isRecording: expectBoolean(r.isRecording, 'trace.isRecording'),
    pointCount: expectInteger(r.pointCount, 'trace.pointCount', 0, COUNT_MAX),
  };
}

export function parseTrack(value: unknown): Track {
  const r = expectRecord(value, 'trace');
  return {
    id: expectString(r.id, 'trace.id'),
    name: expectString(r.name, 'trace.name'),
    points: expectArray(r.points, 'trace.points', (p) =>
      expectPosition(expectRecord(p, 'point').position, 'point.position'),
    ),
    lengthM: expectNumber(r.lengthM, 'trace.lengthM'),
    durationS: optional(r.durationS, (v) => expectNumber(v, 'trace.durationS')),
    bounds: optional(r.bounds, (v) => parseGeoBounds(v, 'trace.bounds')),
  };
}

export function parseGeoBounds(value: unknown, what = 'emprise'): GeoBounds {
  const r = expectRecord(value, what);
  const lat = (v: unknown, k: string) => {
    const n = expectNumber(v, `${what}.${k}`);
    if (n < -90 || n > 90) throw new EngineProtocolError(`${what}.${k} hors bornes`);
    return n;
  };
  const lon = (v: unknown, k: string) => {
    const n = expectNumber(v, `${what}.${k}`);
    if (n < -180 || n > 180) throw new EngineProtocolError(`${what}.${k} hors bornes`);
    return n;
  };
  const bounds = {
    west: lon(r.west, 'west'),
    south: lat(r.south, 'south'),
    east: lon(r.east, 'east'),
    north: lat(r.north, 'north'),
  };
  if (bounds.south > bounds.north) throw new EngineProtocolError(`${what} : sud au nord`);
  return bounds;
}

export function parseRoutePlan(value: unknown): RoutePlan {
  const r = expectRecord(value, 'plan');
  const num = (v: unknown, what: string) => {
    const n = expectNumber(v, `plan.${what}`);
    if (n < 0) throw new EngineProtocolError(`plan.${what} négatif`);
    return n;
  };
  const time = (what: string) => (v: unknown) => expectTimestamp(v, `plan.${what}`);
  return {
    speedMps: num(r.speedMps, 'speedMps'),
    departure: optional(r.departure, time('departure')),
    legs: expectArray(r.legs, 'plan.legs', (v) => {
      const l = expectRecord(v, 'segment');
      return {
        fromIndex: expectInteger(l.fromIndex, 'segment.fromIndex', 0, COUNT_MAX),
        distanceM: num(l.distanceM, 'distanceM'),
        bearingRad: optional(l.bearingRad, (b) => expectNumber(b, 'segment.bearingRad')),
        durationS: num(l.durationS, 'durationS'),
        cumulativeDistanceM: num(l.cumulativeDistanceM, 'cumulativeDistanceM'),
        cumulativeDurationS: num(l.cumulativeDurationS, 'cumulativeDurationS'),
        arrival: optional(l.arrival, time('arrival')),
      };
    }),
    totalDistanceM: num(r.totalDistanceM, 'totalDistanceM'),
    totalDurationS: num(r.totalDurationS, 'totalDurationS'),
    arrival: optional(r.arrival, time('arrival')),
    bounds: parseGeoBounds(r.bounds, 'plan.bounds'),
  };
}

export function parseNavigationStatus(value: unknown): NavigationStatus {
  const r = expectRecord(value, 'navigation');
  const num = (v: unknown, what: string) => expectNumber(v, `navigation.${what}`);
  const time = (what: string) => (v: unknown) => expectTimestamp(v, `navigation.${what}`);
  return {
    routeId: expectString(r.routeId, 'navigation.routeId'),
    routeName: expectString(r.routeName, 'navigation.routeName'),
    targetIndex: expectInteger(r.targetIndex, 'navigation.targetIndex', 0, COUNT_MAX),
    waypointCount: expectInteger(r.waypointCount, 'navigation.waypointCount', 2, COUNT_MAX),
    targetName: expectString(r.targetName, 'navigation.targetName'),
    targetPosition: expectPosition(r.targetPosition, 'navigation.targetPosition'),
    distanceToTargetM: num(r.distanceToTargetM, 'distanceToTargetM'),
    bearingToTargetRad: optional(r.bearingToTargetRad, (v) => num(v, 'bearingToTargetRad')),
    crossTrackM: optional(r.crossTrackM, (v) => num(v, 'crossTrackM')),
    remainingDistanceM: num(r.remainingDistanceM, 'remainingDistanceM'),
    etaTarget: optional(r.etaTarget, time('etaTarget')),
    etaEnd: optional(r.etaEnd, time('etaEnd')),
    reachedWaypoint: optional(r.reachedWaypoint, str('navigation.reachedWaypoint')),
    finished: expectBoolean(r.finished, 'navigation.finished'),
    simulated: expectBoolean(r.simulated, 'navigation.simulated'),
    computedAt: expectTimestamp(r.computedAt, 'navigation.computedAt'),
  };
}

export function parseNavigationState(value: unknown): NavigationState {
  const r = expectRecord(value, 'état de navigation');
  return {
    activeRouteId: optional(r.activeRouteId, str('activeRouteId')),
    recordingTrackId: optional(r.recordingTrackId, str('recordingTrackId')),
  };
}

export function parseImportReport(value: unknown): ImportReport {
  const r = expectRecord(value, 'bilan import');
  const count = (v: unknown, what: string) => expectInteger(v, what, 0, COUNT_MAX);
  return {
    waypoints: count(r.waypoints, 'waypoints'),
    routes: count(r.routes, 'routes'),
    tracks: count(r.tracks, 'tracks'),
    warnings: expectArray(r.warnings, 'warnings', str('warning')),
  };
}

/** Client des commandes de routage. */
export class RoutingApi {
  constructor(private readonly call: InvokeFn = invoke) {}

  async listRoutes(): Promise<Route[]> {
    return expectArray(await this.call('list_routes'), 'routes', parseRoute);
  }

  async createRoute(name: string, positions: LatLon[]): Promise<Route> {
    return parseRoute(await this.call('create_route', { name, positions }));
  }

  async reverseRoute(id: string): Promise<Route> {
    return parseRoute(await this.call('reverse_route', { id }));
  }

  /** Enregistre une route éditée ; `id` absent : création. */
  async saveRoute(id: string | null, name: string, waypoints: WaypointEdit[]): Promise<Route> {
    return parseRoute(await this.call('save_route', { id, name, waypoints }));
  }

  /** Longueur, durée et heures de passage à la vitesse prévue (moteur). */
  async planRoute(
    positions: LatLon[],
    speedMps: number,
    departure: number | null,
  ): Promise<RoutePlan> {
    return parseRoutePlan(
      await this.call('plan_route', {
        positions,
        speedMps,
        departure: departure === null ? null : new Date(departure).toISOString(),
      }),
    );
  }

  async renameTrack(id: string, name: string): Promise<void> {
    await this.call('rename_track', { id, name });
  }

  async deleteRoute(id: string): Promise<void> {
    await this.call('delete_route', { id });
  }

  async activateRoute(id: string): Promise<void> {
    await this.call('activate_route', { id });
  }

  async deactivateRoute(): Promise<void> {
    await this.call('deactivate_route');
  }

  async navigationState(): Promise<NavigationState> {
    return parseNavigationState(await this.call('get_navigation_state'));
  }

  async listTracks(): Promise<TrackSummary[]> {
    return expectArray(await this.call('list_tracks'), 'traces', parseTrackSummary);
  }

  async getTrack(id: string): Promise<Track> {
    return parseTrack(await this.call('get_track', { id }));
  }

  async deleteTrack(id: string): Promise<void> {
    await this.call('delete_track', { id });
  }

  async startRecording(): Promise<TrackSummary> {
    return parseTrackSummary(await this.call('start_track_recording'));
  }

  async stopRecording(): Promise<void> {
    await this.call('stop_track_recording');
  }

  /** Importe un GPX choisi dans le dialogue natif ; `null` si annulé. */
  async importGpx(): Promise<ImportReport | null> {
    return optional(await this.call('import_gpx'), parseImportReport);
  }

  /** Exporte une route ; `false` si l'utilisateur annule. */
  async exportRouteGpx(id: string): Promise<boolean> {
    return expectBoolean(await this.call('export_route_gpx', { id }), 'export');
  }

  /** Exporte une trace ; `false` si l'utilisateur annule. */
  async exportTrackGpx(id: string): Promise<boolean> {
    return expectBoolean(await this.call('export_track_gpx', { id }), 'export');
  }
}

/**
 * S'abonne à l'état de navigation (`null` lorsque la navigation est
 * désactivée). Les messages invalides sont signalés via `onError`.
 */
export function subscribeNavigation(
  onStatus: (status: NavigationStatus | null) => void,
  onError: (error: Error) => void,
  listenFn: ListenFn = listen,
): Promise<UnlistenFn> {
  return listenFn(NAVIGATION_EVENT, (event) => {
    try {
      onStatus(optional(event.payload, parseNavigationStatus));
    } catch (e) {
      onError(e instanceof Error ? e : new Error(String(e)));
    }
  });
}
