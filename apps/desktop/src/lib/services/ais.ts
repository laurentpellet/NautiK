/**
 * Cibles AIS diffusées par le moteur (`AIS_EVENT`).
 *
 * Le décodage, la fraîcheur (`state`), l'historique et les vecteurs de
 * déplacement sont calculés par le moteur : l'interface les affiche sans
 * les recalculer. Toutes les données reçues sont validées.
 */
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { ListenFn } from './position';
import {
  EngineProtocolError,
  expectArray,
  expectBoolean,
  expectInteger,
  expectNumber,
  expectPosition,
  expectRecord,
  expectString,
  expectTimestamp,
  optional,
  type LatLon,
} from './validate';

export const AIS_EVENT = 'ais-targets';

export type AisClass = 'A' | 'B';

/** Active : position récente ; obsolète : comptes rendus manqués ;
 * perdue : plus de compte rendu, ou aucune position valide. */
export type TargetState = 'active' | 'stale' | 'lost';

export interface AisTarget {
  /** MMSI sur 9 chiffres. */
  mmsi: string;
  class: AisClass;
  name: string | null;
  callsign: string | null;
  shipType: number | null;
  navigationStatus: number | null;
  position: LatLon | null;
  /** Vitesse fond (m/s). */
  sogMps: number | null;
  /** Route fond (rad). */
  cogRad: number | null;
  /** Cap vrai (rad). */
  headingRad: number | null;
  /** Extrémité du vecteur de déplacement (cible active seulement). */
  vectorEnd: LatLon | null;
  lastSeenAt: number;
  positionAgeMs: number | null;
  state: TargetState;
  /** Positions passées, de la plus ancienne à la plus récente. */
  history: LatLon[];
}

export interface AisSnapshot {
  computedAt: number;
  /** Cibles simulées ou rejouées : pas la situation réelle actuelle. */
  simulated: boolean;
  targets: AisTarget[];
}

const TAU = 2 * Math.PI;

function expectAngle(value: unknown, what: string): number {
  const angle = expectNumber(value, what);
  if (angle < 0 || angle >= TAU) {
    throw new EngineProtocolError(`${what} hors bornes : ${angle}`);
  }
  return angle;
}

function expectSpeed(value: unknown, what: string): number {
  const speed = expectNumber(value, what);
  if (speed < 0) {
    throw new EngineProtocolError(`${what} négative : ${speed}`);
  }
  return speed;
}

function parseTarget(value: unknown): AisTarget {
  const r = expectRecord(value, 'cible AIS');
  const mmsi = expectString(r.mmsi, 'mmsi');
  if (!/^\d{9}$/.test(mmsi) || mmsi === '000000000') {
    throw new EngineProtocolError(`MMSI invalide : ${mmsi}`);
  }
  const aisClass = expectString(r.class, 'class');
  if (aisClass !== 'A' && aisClass !== 'B') {
    throw new EngineProtocolError(`classe AIS inconnue : ${aisClass}`);
  }
  const state = expectString(r.state, 'state');
  if (state !== 'active' && state !== 'stale' && state !== 'lost') {
    throw new EngineProtocolError(`état de cible inconnu : ${state}`);
  }
  const text = (v: unknown, what: string) => optional(v, (x) => expectString(x, what));
  return {
    mmsi,
    class: aisClass,
    name: text(r.name, 'name'),
    callsign: text(r.callsign, 'callsign'),
    shipType: optional(r.shipType, (v) => expectInteger(v, 'shipType', 1, 255)),
    navigationStatus: optional(r.navigationStatus, (v) =>
      expectInteger(v, 'navigationStatus', 0, 14),
    ),
    position: optional(r.position, (v) => expectPosition(v, 'position')),
    sogMps: optional(r.sogMps, (v) => expectSpeed(v, 'sogMps')),
    cogRad: optional(r.cogRad, (v) => expectAngle(v, 'cogRad')),
    headingRad: optional(r.headingRad, (v) => expectAngle(v, 'headingRad')),
    vectorEnd: optional(r.vectorEnd, (v) => expectPosition(v, 'vectorEnd')),
    lastSeenAt: expectTimestamp(r.lastSeenAt, 'lastSeenAt'),
    positionAgeMs: optional(r.positionAgeMs, (v) =>
      expectInteger(v, 'positionAgeMs', 0, Number.MAX_SAFE_INTEGER),
    ),
    state,
    history: expectArray(r.history, 'history', (v) => expectPosition(v, 'historique')),
  };
}

/** Valide un instantané. Une cible invalide rejette tout l'instantané. */
export function parseAisSnapshot(value: unknown): AisSnapshot {
  const r = expectRecord(value, 'ais');
  return {
    computedAt: expectTimestamp(r.computedAt, 'computedAt'),
    simulated: expectBoolean(r.simulated, 'simulated'),
    targets: expectArray(r.targets, 'targets', parseTarget),
  };
}

/** S'abonne aux cibles AIS ; les instantanés invalides sont signalés et ignorés. */
export function subscribeAis(
  onSnapshot: (snapshot: AisSnapshot) => void,
  onError: (error: Error) => void,
  listenFn: ListenFn = listen,
): Promise<UnlistenFn> {
  return listenFn(AIS_EVENT, (e) => {
    try {
      onSnapshot(parseAisSnapshot(e.payload));
    } catch (err) {
      onError(err instanceof Error ? err : new Error(String(err)));
    }
  });
}

/** Libellés des statuts de navigation (ITU-R M.1371, 0 à 14). */
const NAVIGATION_STATUS: readonly string[] = [
  'Au moteur',
  'Au mouillage',
  'Non maître de sa manœuvre',
  'Manœuvrabilité restreinte',
  'Gêné par son tirant d’eau',
  'Amarré',
  'Échoué',
  'En pêche',
  'À la voile',
  'Réservé (HSC)',
  'Réservé (WIG)',
  'Remorquage arrière',
  'Remorquage avant',
  'Réservé',
  'AIS-SART actif',
];

export function navigationStatusLabel(status: number | null): string | null {
  return status === null ? null : (NAVIGATION_STATUS[status] ?? null);
}

/** Nom affiché : nom du navire, sinon MMSI. */
export function targetLabel(target: AisTarget): string {
  return target.name ?? `MMSI ${target.mmsi}`;
}
