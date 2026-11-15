/**
 * Journal de bord : entrées manuelles et relevés automatiques écrits par
 * le moteur, export CSV. Les données reçues sont validées ; une position
 * obsolète au moment de l'entrée reste signalée comme telle.
 */
import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './engine';
import {
  EngineProtocolError,
  expectArray,
  expectBoolean,
  expectNumber,
  expectPosition,
  expectRecord,
  expectString,
  expectTimestamp,
  optional,
  type LatLon,
} from './validate';

/** Intervalles proposés (minutes ; 0 : désactivés), comme le moteur. */
export const AUTO_INTERVALS_MIN = [0, 15, 30, 60, 240] as const;
/** Nombre d'entrées chargées par page. */
export const LOG_PAGE = 100;

export type LogKind = 'auto' | 'manual';

export interface LogEntry {
  id: string;
  at: number;
  kind: LogKind;
  text: string | null;
  position: LatLon | null;
  positionStale: boolean;
  sogMps: number | null;
  cogRad: number | null;
  headingRad: number | null;
  twsMps: number | null;
  twdRad: number | null;
  pressurePa: number | null;
  depthM: number | null;
  simulated: boolean;
}

export interface LogbookSettings {
  autoIntervalMin: number;
}

const num = (what: string) => (v: unknown) => expectNumber(v, what);

export function parseLogEntry(value: unknown): LogEntry {
  const r = expectRecord(value, 'entrée');
  if (r.kind !== 'auto' && r.kind !== 'manual') {
    throw new EngineProtocolError('entrée.kind : type inconnu');
  }
  const text = optional(r.text, (v) => expectString(v, 'entrée.text'));
  if (text !== null && [...text].length > 2000) {
    throw new EngineProtocolError('entrée.text : texte trop long');
  }
  return {
    id: expectString(r.id, 'entrée.id'),
    at: expectTimestamp(r.at, 'entrée.at'),
    kind: r.kind,
    text,
    position: optional(r.position, (v) => expectPosition(v, 'entrée.position')),
    positionStale: expectBoolean(r.positionStale, 'entrée.positionStale'),
    sogMps: optional(r.sogMps, num('entrée.sogMps')),
    cogRad: optional(r.cogRad, num('entrée.cogRad')),
    headingRad: optional(r.headingRad, num('entrée.headingRad')),
    twsMps: optional(r.twsMps, num('entrée.twsMps')),
    twdRad: optional(r.twdRad, num('entrée.twdRad')),
    pressurePa: optional(r.pressurePa, num('entrée.pressurePa')),
    depthM: optional(r.depthM, num('entrée.depthM')),
    simulated: expectBoolean(r.simulated, 'entrée.simulated'),
  };
}

export function parseLogbookSettings(value: unknown): LogbookSettings {
  const r = expectRecord(value, 'réglages du journal');
  const interval = expectNumber(r.autoIntervalMin, 'autoIntervalMin');
  if (!(AUTO_INTERVALS_MIN as readonly number[]).includes(interval)) {
    throw new EngineProtocolError('autoIntervalMin : intervalle inconnu');
  }
  return { autoIntervalMin: interval };
}

export class LogbookApi {
  constructor(private readonly call: InvokeFn = invoke) {}

  /** Entrées de la plus récente à la plus ancienne, antérieures à `before`. */
  async list(limit = LOG_PAGE, before: number | null = null): Promise<LogEntry[]> {
    return expectArray(
      await this.call('list_log_entries', {
        limit,
        before: before === null ? null : new Date(before).toISOString(),
      }),
      'journal',
      parseLogEntry,
    );
  }

  async add(text: string): Promise<LogEntry> {
    return parseLogEntry(await this.call('add_log_entry', { text }));
  }

  async delete(id: string): Promise<void> {
    await this.call('delete_log_entry', { id });
  }

  async settings(): Promise<LogbookSettings> {
    return parseLogbookSettings(await this.call('get_logbook_settings'));
  }

  async setSettings(settings: LogbookSettings): Promise<LogbookSettings> {
    return parseLogbookSettings(await this.call('set_logbook_settings', { settings }));
  }

  /** Exporte le journal en CSV ; `false` si annulé. */
  async exportCsv(): Promise<boolean> {
    return (await this.call('export_logbook_csv')) === true;
  }
}
