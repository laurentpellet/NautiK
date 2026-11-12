/**
 * Instruments, état de la connexion Signal K et choix de la source.
 *
 * La fraîcheur (`ageMs`, `stale`) est calculée par le moteur : l'interface
 * l'affiche sans la recalculer. Toutes les données reçues sont validées.
 */
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { InvokeFn } from './engine';
import {
  isSourceKind,
  parsePositionFix,
  type ListenFn,
  type PositionFix,
  type SourceKind,
} from './position';
import {
  EngineProtocolError,
  expectArray,
  expectBoolean,
  expectInteger,
  expectNumber,
  expectRecord,
  expectString,
  expectTimestamp,
  optional,
} from './validate';

/** Instantané périodique du moteur (`INSTRUMENTS_EVENT`). */
export const INSTRUMENTS_EVENT = 'instruments';

/** État de la connexion Signal K (`SIGNALK_STATUS_EVENT`). */
export const SIGNALK_STATUS_EVENT = 'signalk-status';

/** Grandeurs mesurées (miroir de `nav_core::Quantity`), en unités SI. */
export const QUANTITIES = [
  'speedOverGround',
  'courseOverGroundTrue',
  'headingTrue',
  'headingMagnetic',
  'speedThroughWater',
  'depthBelowTransducer',
  'windSpeedApparent',
  'windAngleApparent',
] as const;

export type Quantity = (typeof QUANTITIES)[number];

function isQuantity(value: string): value is Quantity {
  return (QUANTITIES as readonly string[]).includes(value);
}

export interface PositionStatus {
  fix: PositionFix;
  ageMs: number;
  stale: boolean;
}

export interface Reading {
  quantity: Quantity;
  /** Valeur SI (m/s, rad, m). */
  value: number;
  source: SourceKind;
  sourceId: string;
  measuredAt: number;
  ageMs: number;
  stale: boolean;
}

export interface InstrumentsSnapshot {
  computedAt: number;
  position: PositionStatus | null;
  readings: Reading[];
}

const MAX_AGE_MS = Number.MAX_SAFE_INTEGER;

function parseAge(value: unknown, what: string): number {
  return expectInteger(value, what, 0, MAX_AGE_MS);
}

function parsePositionStatus(value: unknown): PositionStatus {
  const r = expectRecord(value, 'position');
  return {
    fix: parsePositionFix(r.fix),
    ageMs: parseAge(r.ageMs, 'position.ageMs'),
    stale: expectBoolean(r.stale, 'position.stale'),
  };
}

function parseReading(value: unknown): Reading {
  const r = expectRecord(value, 'mesure');
  const quantity = expectString(r.quantity, 'quantity');
  if (!isQuantity(quantity)) {
    throw new EngineProtocolError(`grandeur inconnue : ${quantity}`);
  }
  const source = expectString(r.source, 'source');
  if (!isSourceKind(source)) {
    throw new EngineProtocolError(`source inconnue : ${source}`);
  }
  return {
    quantity,
    value: expectNumber(r.value, quantity),
    source,
    sourceId: expectString(r.sourceId, 'sourceId'),
    measuredAt: expectTimestamp(r.measuredAt, 'measuredAt'),
    ageMs: parseAge(r.ageMs, 'ageMs'),
    stale: expectBoolean(r.stale, 'stale'),
  };
}

/** Valide un instantané. Une donnée invalide rejette tout l'instantané. */
export function parseInstrumentsSnapshot(value: unknown): InstrumentsSnapshot {
  const r = expectRecord(value, 'instruments');
  return {
    computedAt: expectTimestamp(r.computedAt, 'computedAt'),
    position: optional(r.position, parsePositionStatus),
    readings: expectArray(r.readings, 'readings', parseReading),
  };
}

/** État de la connexion Signal K (miroir de `nav_signalk::ConnectionState`). */
export type ConnectionState =
  | { state: 'connecting'; attempt: number }
  | { state: 'connected'; serverName: string | null; serverVersion: string | null }
  | { state: 'disconnected'; reason: string; retryInMs: number }
  | { state: 'unauthorized'; reason: string; retryInMs: number }
  | { state: 'stopped' };

export function parseConnectionState(value: unknown): ConnectionState {
  const r = expectRecord(value, 'connexion');
  const state = expectString(r.state, 'state');
  switch (state) {
    case 'connecting':
      return { state, attempt: expectInteger(r.attempt, 'attempt', 0, 2 ** 32 - 1) };
    case 'connected':
      return {
        state,
        serverName: optional(r.serverName, (v) => expectString(v, 'serverName')),
        serverVersion: optional(r.serverVersion, (v) => expectString(v, 'serverVersion')),
      };
    case 'disconnected':
    case 'unauthorized':
      return {
        state,
        reason: expectString(r.reason, 'reason'),
        retryInMs: parseAge(r.retryInMs, 'retryInMs'),
      };
    case 'stopped':
      return { state };
    default:
      throw new EngineProtocolError(`état de connexion inconnu : ${state}`);
  }
}

export type SourceMode = 'simulator' | 'signalK';

export interface SourcesView {
  mode: SourceMode;
  signalkUrl: string | null;
  /** Un jeton est enregistré (sa valeur n'est jamais transmise). */
  tokenStored: boolean;
  connection: ConnectionState | null;
}

export function parseSourcesView(value: unknown): SourcesView {
  const r = expectRecord(value, 'sources');
  const mode = expectString(r.mode, 'mode');
  if (mode !== 'simulator' && mode !== 'signalK') {
    throw new EngineProtocolError(`mode de source inconnu : ${mode}`);
  }
  return {
    mode,
    signalkUrl: optional(r.signalkUrl, (v) => expectString(v, 'signalkUrl')),
    tokenStored: expectBoolean(r.tokenStored, 'tokenStored'),
    connection: optional(r.connection, parseConnectionState),
  };
}

export interface SourcesRequest {
  mode: SourceMode;
  signalkUrl: string | null;
  /** Nouveau jeton ; `null` = inchangé. */
  token: string | null;
  clearToken: boolean;
}

/** Commandes de choix de la source. */
export class SourcesApi {
  constructor(private readonly invokeFn: InvokeFn = invoke) {}

  async get(): Promise<SourcesView> {
    return parseSourcesView(await this.invokeFn('get_sources'));
  }

  async set(request: SourcesRequest): Promise<SourcesView> {
    return parseSourcesView(
      await this.invokeFn('set_sources', {
        mode: request.mode,
        signalkUrl: request.signalkUrl,
        token: request.token,
        clearToken: request.clearToken,
      }),
    );
  }
}

function subscribe<T>(
  event: string,
  parse: (value: unknown) => T,
  onValue: (value: T) => void,
  onError: (error: Error) => void,
  listenFn: ListenFn,
): Promise<UnlistenFn> {
  return listenFn(event, (e) => {
    try {
      onValue(parse(e.payload));
    } catch (err) {
      onError(err instanceof Error ? err : new Error(String(err)));
    }
  });
}

/** S'abonne aux instantanés ; les instantanés invalides sont signalés et ignorés. */
export function subscribeInstruments(
  onSnapshot: (snapshot: InstrumentsSnapshot) => void,
  onError: (error: Error) => void,
  listenFn: ListenFn = listen,
): Promise<UnlistenFn> {
  return subscribe(INSTRUMENTS_EVENT, parseInstrumentsSnapshot, onSnapshot, onError, listenFn);
}

/** S'abonne à l'état Signal K (`null` hors mode Signal K). */
export function subscribeConnection(
  onState: (state: ConnectionState | null) => void,
  onError: (error: Error) => void,
  listenFn: ListenFn = listen,
): Promise<UnlistenFn> {
  return subscribe(
    SIGNALK_STATUS_EVENT,
    (v) => optional(v, parseConnectionState),
    onState,
    onError,
    listenFn,
  );
}
