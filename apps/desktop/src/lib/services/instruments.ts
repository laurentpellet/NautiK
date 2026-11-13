/**
 * Instruments, état de la liaison d'acquisition et choix de la source.
 *
 * La fraîcheur (`ageMs`, `stale`) et l'arbitrage des sources sont
 * calculés par le moteur : l'interface les affiche sans les recalculer.
 * Toutes les données reçues sont validées.
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

/** État de la liaison d'acquisition (`SOURCE_STATUS_EVENT`). */
export const SOURCE_STATUS_EVENT = 'source-status';

/** Données rejetées par la liaison (`SOURCE_REJECTIONS_EVENT`). */
export const SOURCE_REJECTIONS_EVENT = 'source-rejections';

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
  'windSpeedTrue',
  'windDirectionTrue',
  'waterTemperature',
  'airTemperature',
  'atmosphericPressure',
  'engineSpeed',
  'engineCoolantTemperature',
  'engineOilPressure',
  'fuelLevel',
  'freshWaterLevel',
  'batteryVoltage',
  'batteryCurrent',
  'batteryStateOfCharge',
  'solarPower',
] as const;

export type Quantity = (typeof QUANTITIES)[number];

function isQuantity(value: string): value is Quantity {
  return (QUANTITIES as readonly string[]).includes(value);
}

export interface PositionStatus {
  fix: PositionFix;
  ageMs: number;
  stale: boolean;
  /** Nombre de sources de position fraîches (arbitrage). */
  freshSources: number;
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
  /** Nombre de sources fraîches pour cette grandeur. */
  freshSources: number;
}

export interface InstrumentsSnapshot {
  computedAt: number;
  position: PositionStatus | null;
  readings: Reading[];
}

const MAX_COUNT = Number.MAX_SAFE_INTEGER;

function parseCount(value: unknown, what: string): number {
  return expectInteger(value, what, 0, MAX_COUNT);
}

function parsePositionStatus(value: unknown): PositionStatus {
  const r = expectRecord(value, 'position');
  return {
    fix: parsePositionFix(r.fix),
    ageMs: parseCount(r.ageMs, 'position.ageMs'),
    stale: expectBoolean(r.stale, 'position.stale'),
    freshSources: parseCount(r.freshSources, 'position.freshSources'),
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
    ageMs: parseCount(r.ageMs, 'ageMs'),
    stale: expectBoolean(r.stale, 'stale'),
    freshSources: parseCount(r.freshSources, 'freshSources'),
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

/** État de la liaison (miroir de `nav_core::ConnectionState`). */
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
        retryInMs: parseCount(r.retryInMs, 'retryInMs'),
      };
    case 'stopped':
      return { state };
    default:
      throw new EngineProtocolError(`état de connexion inconnu : ${state}`);
  }
}

/** Données rejetées par la liaison courante. */
export interface Rejections {
  count: number;
  lastReason: string | null;
}

export function parseRejections(value: unknown): Rejections {
  const r = expectRecord(value, 'rejets');
  return {
    count: parseCount(r.count, 'count'),
    lastReason: optional(r.lastReason, (v) => expectString(v, 'lastReason')),
  };
}

export type SourceMode = 'simulator' | 'signalK' | 'nmea0183';

const SOURCE_MODES: readonly string[] = ['simulator', 'signalK', 'nmea0183'];

function isSourceMode(value: string): value is SourceMode {
  return SOURCE_MODES.includes(value);
}

/** Débits série acceptés par le moteur. */
export const BAUD_RATES = [4800, 9600, 19200, 38400, 57600, 115200] as const;

/** Port TCP/UDP NMEA usuel. */
export const DEFAULT_NMEA_PORT = 10110;

/** Liaison NMEA 0183 (miroir de `NmeaSettings`). */
export type NmeaSettings =
  | { kind: 'tcp'; host: string; port: number }
  | { kind: 'udp'; port: number }
  | { kind: 'serial'; path: string; baudRate: number }
  | { kind: 'replay'; path: string };

function parsePort(value: unknown): number {
  return expectInteger(value, 'port', 1, 65535);
}

export function parseNmeaSettings(value: unknown): NmeaSettings {
  const r = expectRecord(value, 'nmea');
  const kind = expectString(r.kind, 'kind');
  switch (kind) {
    case 'tcp':
      return { kind, host: expectString(r.host, 'host'), port: parsePort(r.port) };
    case 'udp':
      return { kind, port: parsePort(r.port) };
    case 'serial':
      return {
        kind,
        path: expectString(r.path, 'path'),
        baudRate: expectInteger(r.baudRate, 'baudRate', 1, 10_000_000),
      };
    case 'replay':
      return { kind, path: expectString(r.path, 'path') };
    default:
      throw new EngineProtocolError(`liaison NMEA inconnue : ${kind}`);
  }
}

export interface SourcesView {
  mode: SourceMode;
  signalkUrl: string | null;
  nmea: NmeaSettings | null;
  /** Priorités des sources (identifiants ou préfixes). */
  priorities: string[];
  /** Un jeton est enregistré (sa valeur n'est jamais transmise). */
  tokenStored: boolean;
  connection: ConnectionState | null;
  rejections: Rejections;
}

export function parseSourcesView(value: unknown): SourcesView {
  const r = expectRecord(value, 'sources');
  const mode = expectString(r.mode, 'mode');
  if (!isSourceMode(mode)) {
    throw new EngineProtocolError(`mode de source inconnu : ${mode}`);
  }
  return {
    mode,
    signalkUrl: optional(r.signalkUrl, (v) => expectString(v, 'signalkUrl')),
    nmea: optional(r.nmea, parseNmeaSettings),
    priorities: expectArray(r.priorities, 'priorities', (v) => expectString(v, 'priorité')),
    tokenStored: expectBoolean(r.tokenStored, 'tokenStored'),
    connection: optional(r.connection, parseConnectionState),
    rejections: parseRejections(r.rejections),
  };
}

export interface SourcesRequest {
  mode: SourceMode;
  signalkUrl: string | null;
  nmea: NmeaSettings | null;
  priorities: string[];
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
        request: {
          mode: request.mode,
          signalkUrl: request.signalkUrl,
          nmea: request.nmea,
          priorities: request.priorities,
          token: request.token,
          clearToken: request.clearToken,
        },
      }),
    );
  }

  /** Ports série présents sur la machine. */
  async serialPorts(): Promise<string[]> {
    return expectArray(await this.invokeFn('list_serial_ports'), 'ports', (v) =>
      expectString(v, 'port'),
    );
  }

  /** Choix d'un enregistrement à rejouer (dialogue du système). */
  async chooseRecording(): Promise<string | null> {
    return optional(await this.invokeFn('choose_nmea_recording'), (v) =>
      expectString(v, 'fichier'),
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

/** S'abonne à l'état de la liaison (`null` en mode simulateur). */
export function subscribeConnection(
  onState: (state: ConnectionState | null) => void,
  onError: (error: Error) => void,
  listenFn: ListenFn = listen,
): Promise<UnlistenFn> {
  return subscribe(
    SOURCE_STATUS_EVENT,
    (v) => optional(v, parseConnectionState),
    onState,
    onError,
    listenFn,
  );
}

/** S'abonne aux rejets de la liaison. */
export function subscribeRejections(
  onRejections: (rejections: Rejections) => void,
  onError: (error: Error) => void,
  listenFn: ListenFn = listen,
): Promise<UnlistenFn> {
  return subscribe(SOURCE_REJECTIONS_EVENT, parseRejections, onRejections, onError, listenFn);
}
