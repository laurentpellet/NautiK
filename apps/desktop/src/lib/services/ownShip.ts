/**
 * Aides à la navigation du bateau (vecteur de route, ligne de cap, VMG,
 * laylines), calculées par le moteur Rust à chaque pas. Ce service ne
 * fait que valider et relayer son état.
 */
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { ListenFn } from './position';
import {
  expectBoolean,
  expectNumber,
  expectPosition,
  expectRecord,
  expectTimestamp,
  optional,
  type LatLon,
} from './validate';

export const OWN_SHIP_EVENT = 'own-ship';

export interface Laylines {
  mark: LatLon;
  starboardEnd: LatLon;
  portEnd: LatLon;
  /** Marque au vent (près) ; sinon sous le vent (portant). */
  upwind: boolean;
  windAngleRad: number;
}

/** Dérive : route fond et ligne de foi (moteur), données fraîches seulement. */
export interface Drift {
  cogRad: number;
  headingRad: number;
  sogMps: number;
  /** `COG − cap` (rad, ]−π, π]) : positif quand le bateau est porté sur tribord. */
  angleRad: number;
}

export interface OwnShip {
  computedAt: number;
  simulated: boolean;
  vectorHorizonS: number;
  /** Position fraîche utilisée ; `null` : rien n'est calculé. */
  position: LatLon | null;
  vectorEnd: LatLon | null;
  headingEnd: LatLon | null;
  trueWindDirectionRad: number | null;
  vmgWindMps: number | null;
  vmgWaypointMps: number | null;
  otherTackCogRad: number | null;
  laylines: Laylines | null;
  drift: Drift | null;
}

const num = (what: string) => (v: unknown) => expectNumber(v, what);
const pos = (what: string) => (v: unknown) => expectPosition(v, what);

function parseLaylines(value: unknown): Laylines {
  const r = expectRecord(value, 'laylines');
  return {
    mark: expectPosition(r.mark, 'marque'),
    starboardEnd: expectPosition(r.starboardEnd, 'starboardEnd'),
    portEnd: expectPosition(r.portEnd, 'portEnd'),
    upwind: expectBoolean(r.upwind, 'upwind'),
    windAngleRad: expectNumber(r.windAngleRad, 'windAngleRad'),
  };
}

function parseDrift(value: unknown): Drift {
  const r = expectRecord(value, 'dérive');
  return {
    cogRad: expectNumber(r.cogRad, 'cogRad'),
    headingRad: expectNumber(r.headingRad, 'headingRad'),
    sogMps: expectNumber(r.sogMps, 'sogMps'),
    angleRad: expectNumber(r.angleRad, 'angleRad'),
  };
}

export function parseOwnShip(value: unknown): OwnShip {
  const r = expectRecord(value, 'bateau');
  return {
    computedAt: expectTimestamp(r.computedAt, 'computedAt'),
    simulated: expectBoolean(r.simulated, 'simulated'),
    vectorHorizonS: expectNumber(r.vectorHorizonS, 'vectorHorizonS'),
    position: optional(r.position, pos('position')),
    vectorEnd: optional(r.vectorEnd, pos('vectorEnd')),
    headingEnd: optional(r.headingEnd, pos('headingEnd')),
    trueWindDirectionRad: optional(r.trueWindDirectionRad, num('trueWindDirectionRad')),
    vmgWindMps: optional(r.vmgWindMps, num('vmgWindMps')),
    vmgWaypointMps: optional(r.vmgWaypointMps, num('vmgWaypointMps')),
    otherTackCogRad: optional(r.otherTackCogRad, num('otherTackCogRad')),
    laylines: optional(r.laylines, parseLaylines),
    drift: optional(r.drift, parseDrift),
  };
}

export function subscribeOwnShip(
  onSnapshot: (snapshot: OwnShip) => void,
  onError: (error: Error) => void,
  listenFn: ListenFn = listen,
): Promise<UnlistenFn> {
  return listenFn(OWN_SHIP_EVENT, (e) => {
    try {
      onSnapshot(parseOwnShip(e.payload));
    } catch (err) {
      onError(err instanceof Error ? err : new Error(String(err)));
    }
  });
}
