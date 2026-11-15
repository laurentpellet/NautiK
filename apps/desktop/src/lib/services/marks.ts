/**
 * Marques personnelles (mouillage, danger, plongée…) et import/export
 * KML. Toutes les données reçues du moteur sont validées ; la validation
 * des saisies (nom, note, icône) est faite par le moteur.
 */
import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './engine';
import {
  EngineProtocolError,
  expectArray,
  expectInteger,
  expectPosition,
  expectRecord,
  expectString,
  expectTimestamp,
  optional,
  type LatLon,
} from './validate';

export const MARK_ICONS = ['flag', 'anchorage', 'danger', 'dive', 'fish', 'fuel', 'info'] as const;
export type MarkIcon = (typeof MARK_ICONS)[number];

export const MARK_ICON_LABELS: Record<MarkIcon, string> = {
  flag: 'Repère',
  anchorage: 'Mouillage',
  danger: 'Danger',
  dive: 'Plongée',
  fish: 'Pêche',
  fuel: 'Carburant',
  info: 'Information',
};

/** Symbole court affiché dans la liste. */
export const MARK_ICON_SYMBOLS: Record<MarkIcon, string> = {
  flag: '⚑',
  anchorage: '⚓',
  danger: '⚠',
  dive: '◉',
  fish: '⋈',
  fuel: '⛽',
  info: 'ℹ',
};

/** Couleur de la pastille de chaque icône. */
export const MARK_ICON_COLORS: Record<MarkIcon, string> = {
  flag: '#6a1b9a',
  anchorage: '#1565c0',
  danger: '#d32f2f',
  dive: '#00838f',
  fish: '#2e7d32',
  fuel: '#ef6c00',
  info: '#455a64',
};

export interface Mark {
  id: string;
  name: string;
  position: LatLon;
  icon: MarkIcon;
  note: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface MarksImportReport {
  imported: number;
  warnings: string[];
}

const MAX_NAME = 100;
const MAX_NOTE = 2000;

export function isMarkIcon(value: unknown): value is MarkIcon {
  return typeof value === 'string' && (MARK_ICONS as readonly string[]).includes(value);
}

function boundedText(value: unknown, what: string, max: number): string {
  const s = expectString(value, what);
  if ([...s].length > max) throw new EngineProtocolError(`${what} : texte trop long`);
  return s;
}

export function parseMark(value: unknown): Mark {
  const r = expectRecord(value, 'marque');
  if (!isMarkIcon(r.icon)) throw new EngineProtocolError('marque.icon : icône inconnue');
  return {
    id: expectString(r.id, 'marque.id'),
    name: boundedText(r.name, 'marque.name', MAX_NAME),
    position: expectPosition(r.position, 'marque.position'),
    icon: r.icon,
    note: optional(r.note, (v) => boundedText(v, 'marque.note', MAX_NOTE)),
    createdAt: expectTimestamp(r.createdAt, 'marque.createdAt'),
    updatedAt: expectTimestamp(r.updatedAt, 'marque.updatedAt'),
  };
}

export function parseMarksImport(value: unknown): MarksImportReport {
  const r = expectRecord(value, 'import');
  return {
    imported: expectInteger(r.imported, 'import.imported', 0, 1_000_000),
    warnings: expectArray(r.warnings, 'import.warnings', (v) => expectString(v, 'avertissement')),
  };
}

/** Marque à enregistrer : `id` absent pour une création. */
export interface MarkDraft {
  id: string | null;
  name: string;
  position: LatLon;
  icon: MarkIcon;
  note: string;
}

export class MarksApi {
  constructor(private readonly call: InvokeFn = invoke) {}

  async list(): Promise<Mark[]> {
    return expectArray(await this.call('list_marks'), 'marques', parseMark);
  }

  async save(draft: MarkDraft): Promise<Mark> {
    const note = draft.note.trim();
    return parseMark(
      await this.call('save_mark', {
        id: draft.id,
        name: draft.name,
        position: draft.position,
        icon: draft.icon,
        note: note === '' ? null : note,
      }),
    );
  }

  async delete(id: string): Promise<void> {
    await this.call('delete_mark', { id });
  }

  /** Importe un KML choisi dans le dialogue natif ; `null` si annulé. */
  async importKml(): Promise<MarksImportReport | null> {
    return optional(await this.call('import_kml'), parseMarksImport);
  }

  /** Exporte toutes les marques ; `false` si annulé. */
  async exportKml(): Promise<boolean> {
    return (await this.call('export_marks_kml')) === true;
  }
}
