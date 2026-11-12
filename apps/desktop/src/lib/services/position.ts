import type { UnlistenFn } from '@tauri-apps/api/event';
import {
  EngineProtocolError,
  expectNumber,
  expectRecord,
  expectString,
  optional,
} from './validate';

export type SourceKind = 'simulator' | 'nmea0183' | 'nmea2000' | 'signalK';

/** Relevé de position (miroir de `nav_core::PositionFix`). */
export interface PositionFix {
  latitudeDeg: number;
  longitudeDeg: number;
  cogRad: number | null;
  sogMps: number | null;
  source: SourceKind;
  sourceId: string;
  /** Horodatage UTC de la mesure, en millisecondes depuis l'epoch. */
  measuredAt: number;
}

const SOURCES: readonly string[] = ['simulator', 'nmea0183', 'nmea2000', 'signalK'];

export function isSourceKind(value: string): value is SourceKind {
  return SOURCES.includes(value);
}

function parseTimestamp(value: unknown, what: string): number {
  const ms = Date.parse(expectString(value, what));
  if (Number.isNaN(ms)) {
    throw new EngineProtocolError(`${what} : horodatage invalide`);
  }
  return ms;
}

/** Valide un relevé reçu du moteur. Une position invalide est rejetée, jamais corrigée. */
export function parsePositionFix(value: unknown): PositionFix {
  const r = expectRecord(value, 'relevé');
  const position = expectRecord(r.position, 'position');
  const latitudeDeg = expectNumber(position.latitudeDeg, 'latitudeDeg');
  const longitudeDeg = expectNumber(position.longitudeDeg, 'longitudeDeg');
  if (Math.abs(latitudeDeg) > 90 || Math.abs(longitudeDeg) > 180) {
    throw new EngineProtocolError('position hors des bornes WGS84');
  }
  const source = expectString(r.source, 'source');
  if (!isSourceKind(source)) {
    throw new EngineProtocolError(`source inconnue : ${source}`);
  }
  return {
    latitudeDeg,
    longitudeDeg,
    cogRad: optional(r.cogRad, (v) => expectNumber(v, 'cogRad')),
    sogMps: optional(r.sogMps, (v) => expectNumber(v, 'sogMps')),
    source,
    sourceId: expectString(r.sourceId, 'sourceId'),
    measuredAt: parseTimestamp(r.measuredAt, 'measuredAt'),
  };
}

export type ListenFn = (
  event: string,
  handler: (event: { payload: unknown }) => void,
) => Promise<UnlistenFn>;
