/**
 * Alarmes (mouillage, perte de position). L'évaluation est faite par le
 * moteur Rust ; ce service ne fait que lire son état et relayer les
 * commandes de l'utilisateur.
 */
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { InvokeFn } from './engine';
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

export type AlarmKind = 'anchor' | 'positionLost';
export type AlarmState = 'off' | 'normal' | 'pending' | 'active' | 'acknowledged';
export type EventKind = 'activated' | 'acknowledged' | 'resolved' | 'enabled' | 'disabled';

const KINDS: readonly AlarmKind[] = ['anchor', 'positionLost'];
const STATES: readonly AlarmState[] = ['off', 'normal', 'pending', 'active', 'acknowledged'];
const EVENTS: readonly EventKind[] = [
  'activated',
  'acknowledged',
  'resolved',
  'enabled',
  'disabled',
];

/** Bornes du rayon d'évitage (m), comme le moteur. */
export const MIN_ANCHOR_RADIUS_M = 10;
export const MAX_ANCHOR_RADIUS_M = 2_000;
/** Temporisations proposées (s). */
export const ANCHOR_DELAYS_S = [0, 5, 10, 20, 30, 60] as const;
export const DEFAULT_ANCHOR_DELAY_S = 10;

export const ALARMS_EVENT = 'alarms';

export interface AlarmView {
  kind: AlarmKind;
  label: string;
  state: AlarmState;
  since: number | null;
  message: string;
}

export interface AnchorView {
  position: LatLon;
  radiusM: number;
  hysteresisM: number;
  delayS: number;
  distanceM: number | null;
  bearingRad: number | null;
}

export interface AlarmsSnapshot {
  computedAt: number;
  alarms: AlarmView[];
  anchor: AnchorView | null;
  anchorRadiusM: number;
  sounding: boolean;
  simulated: boolean;
}

export interface AlarmEvent {
  id: string;
  alarm: AlarmKind;
  kind: EventKind;
  at: number;
  message: string;
  distanceM: number | null;
  simulated: boolean;
}

function oneOf<T extends string>(value: unknown, allowed: readonly T[], what: string): T {
  const s = expectString(value, what);
  if (!(allowed as readonly string[]).includes(s)) {
    throw new EngineProtocolError(`${what} inconnu : ${s}`);
  }
  return s as T;
}

function parseAlarm(value: unknown): AlarmView {
  const r = expectRecord(value, 'alarme');
  return {
    kind: oneOf(r.kind, KINDS, 'alarme'),
    label: expectString(r.label, 'label'),
    state: oneOf(r.state, STATES, 'état'),
    since: optional(r.since, (v) => expectTimestamp(v, 'since')),
    message: expectString(r.message, 'message'),
  };
}

function parseAnchor(value: unknown): AnchorView {
  const r = expectRecord(value, 'mouillage');
  const anchor = {
    position: expectPosition(r.position, 'ancre'),
    radiusM: expectNumber(r.radiusM, 'radiusM'),
    hysteresisM: expectNumber(r.hysteresisM, 'hysteresisM'),
    delayS: expectInteger(r.delayS, 'delayS', 0, 300),
    distanceM: optional(r.distanceM, (v) => expectNumber(v, 'distanceM')),
    bearingRad: optional(r.bearingRad, (v) => expectNumber(v, 'bearingRad')),
  };
  if (
    anchor.radiusM < MIN_ANCHOR_RADIUS_M ||
    anchor.radiusM > MAX_ANCHOR_RADIUS_M ||
    (anchor.distanceM !== null && anchor.distanceM < 0)
  ) {
    throw new EngineProtocolError('mouillage hors bornes');
  }
  return anchor;
}

export function parseAlarmsSnapshot(value: unknown): AlarmsSnapshot {
  const r = expectRecord(value, 'alarmes');
  return {
    computedAt: expectTimestamp(r.computedAt, 'computedAt'),
    alarms: expectArray(r.alarms, 'alarms', parseAlarm),
    anchor: optional(r.anchor, parseAnchor),
    anchorRadiusM: expectNumber(r.anchorRadiusM, 'anchorRadiusM'),
    sounding: expectBoolean(r.sounding, 'sounding'),
    simulated: expectBoolean(r.simulated, 'simulated'),
  };
}

export function parseAlarmEvent(value: unknown): AlarmEvent {
  const r = expectRecord(value, 'événement');
  const d = expectRecord(r.details, 'details');
  return {
    id: expectString(r.id, 'id'),
    alarm: oneOf(r.alarm, KINDS, 'alarme'),
    kind: oneOf(r.kind, EVENTS, 'événement'),
    at: expectTimestamp(r.at, 'at'),
    message: expectString(d.message, 'message'),
    distanceM: optional(d.distanceM, (v) => expectNumber(v, 'distanceM')),
    simulated: d.simulated === undefined ? false : expectBoolean(d.simulated, 'simulated'),
  };
}

/** Alarmes déclenchées (acquittées ou non), à afficher en bandeau. */
export function alertingAlarms(snapshot: AlarmsSnapshot | null): AlarmView[] {
  return (snapshot?.alarms ?? []).filter((a) => a.state === 'active' || a.state === 'acknowledged');
}

/** Libellés des événements de l'historique. */
export const EVENT_LABELS: Record<EventKind, string> = {
  activated: 'Déclenchée',
  acknowledged: 'Acquittée',
  resolved: 'Résolue',
  enabled: 'Activée',
  disabled: 'Désactivée',
};

export class AlarmsApi {
  constructor(private readonly invokeFn: InvokeFn = invoke) {}

  async get(): Promise<AlarmsSnapshot> {
    return parseAlarmsSnapshot(await this.invokeFn('get_alarms'));
  }

  async dropAnchor(radiusM: number, delayS: number): Promise<AlarmsSnapshot> {
    return parseAlarmsSnapshot(await this.invokeFn('drop_anchor', { radiusM, delayS }));
  }

  async raiseAnchor(): Promise<AlarmsSnapshot> {
    return parseAlarmsSnapshot(await this.invokeFn('raise_anchor'));
  }

  async acknowledge(kind: AlarmKind): Promise<AlarmsSnapshot> {
    return parseAlarmsSnapshot(await this.invokeFn('acknowledge_alarm', { kind }));
  }

  async setEnabled(kind: AlarmKind, enabled: boolean): Promise<AlarmsSnapshot> {
    return parseAlarmsSnapshot(await this.invokeFn('set_alarm_enabled', { kind, enabled }));
  }

  async events(limit: number): Promise<AlarmEvent[]> {
    return expectArray(
      await this.invokeFn('list_alarm_events', { limit }),
      'événements',
      parseAlarmEvent,
    );
  }
}

export function subscribeAlarms(
  onSnapshot: (snapshot: AlarmsSnapshot) => void,
  onError: (error: Error) => void,
  listenFn: ListenFn = listen,
): Promise<UnlistenFn> {
  return listenFn(ALARMS_EVENT, (e) => {
    try {
      onSnapshot(parseAlarmsSnapshot(e.payload));
    } catch (err) {
      onError(err instanceof Error ? err : new Error(String(err)));
    }
  });
}
