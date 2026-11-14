/**
 * État résumé affiché dans la barre d'en-tête : une pastille par
 * chaîne de données (position, liaison, AIS, moteur). Fonctions pures :
 * aucune donnée n'est déduite, seuls les états reçus sont résumés.
 */
import { describeConnection, formatAge } from './format';
import type { AisSnapshot } from './services/ais';
import { alertingAlarms, type AlarmsSnapshot } from './services/alarms';
import type { ConnectionState, PositionStatus } from './services/instruments';

/** Gravité d'une pastille : `off` = information absente. */
export type PillLevel = 'ok' | 'warn' | 'alert' | 'off';

export interface StatusPill {
  id: 'position' | 'link' | 'ais' | 'alarm' | 'engine';
  label: string;
  /** État court affiché sous le libellé. */
  state: string;
  level: PillLevel;
  /** Détail complet (infobulle). */
  title: string;
}

export interface ShellInputs {
  position: PositionStatus | null;
  silent: boolean;
  /** `undefined` : état inconnu ; `null` : pas de liaison (simulateur). */
  connection: ConnectionState | null | undefined;
  ais: AisSnapshot | null;
  /** État des alarmes (moteur) ; absent tant qu'il n'est pas connu. */
  alarms?: AlarmsSnapshot | null;
}

function alarmPill(alarms: AlarmsSnapshot | null | undefined): StatusPill {
  const base = { id: 'alarm', label: 'Alarmes' } as const;
  if (!alarms) return { ...base, state: '…', level: 'off', title: 'État des alarmes inconnu' };
  const alerting = alertingAlarms(alarms);
  if (alerting.length > 0) {
    return {
      ...base,
      state: alerting.length > 1 ? `${alerting.length} actives` : (alerting[0]?.label ?? ''),
      level: 'alert',
      title: alerting.map((a) => `${a.label} : ${a.message}`).join(' — '),
    };
  }
  const pending = alarms.alarms.filter((a) => a.state === 'pending');
  if (pending.length > 0) {
    return {
      ...base,
      state: 'Vérification',
      level: 'warn',
      title: pending.map((a) => `${a.label} : ${a.message}`).join(' — '),
    };
  }
  const off = alarms.alarms.filter((a) => a.kind === 'positionLost' && a.state === 'off');
  if (off.length > 0) {
    return { ...base, state: 'Réduites', level: 'warn', title: 'Perte de position non surveillée' };
  }
  return {
    ...base,
    state: alarms.anchor ? 'Mouillage' : 'Veille',
    level: 'ok',
    title: alarms.anchor ? 'Mouillage surveillé' : 'Aucune alarme déclenchée',
  };
}

function positionPill(position: PositionStatus | null): StatusPill {
  const base = { id: 'position', label: 'Position' } as const;
  if (!position) {
    return { ...base, state: 'Aucune', level: 'alert', title: 'Aucune position reçue' };
  }
  const source = `Source : ${position.fix.sourceId}`;
  if (position.stale) {
    return {
      ...base,
      state: 'Obsolète',
      level: 'alert',
      title: `Position obsolète (${formatAge(position.ageMs)}) — ${source}`,
    };
  }
  if (position.fix.source === 'simulator') {
    return { ...base, state: 'Simulée', level: 'warn', title: `Position simulée — ${source}` };
  }
  if (position.fix.source === 'replay') {
    return { ...base, state: 'Rejeu', level: 'warn', title: `Rejeu, non actuel — ${source}` };
  }
  return { ...base, state: 'OK', level: 'ok', title: source };
}

function linkPill(connection: ConnectionState | null | undefined): StatusPill {
  const base = { id: 'link', label: 'Liaison' } as const;
  if (connection === undefined) {
    return { ...base, state: '…', level: 'off', title: 'État de la liaison inconnu' };
  }
  if (connection === null) {
    return { ...base, state: 'Locale', level: 'ok', title: 'Aucune liaison externe (simulateur)' };
  }
  const d = describeConnection(connection);
  const state = d.level === 'ok' ? 'Connectée' : d.level === 'pending' ? 'Connexion…' : 'En défaut';
  const level: PillLevel = d.level === 'pending' ? 'warn' : d.level;
  return { ...base, state, level, title: d.label };
}

function aisPill(ais: AisSnapshot | null): StatusPill {
  const base = { id: 'ais', label: 'AIS' } as const;
  if (!ais) return { ...base, state: '—', level: 'off', title: 'Aucune donnée AIS' };
  const n = ais.targets.length;
  const state = `${n} cible${n > 1 ? 's' : ''}`;
  if (ais.simulated) {
    return { ...base, state, level: 'warn', title: 'Cibles simulées : pas la situation réelle' };
  }
  return { ...base, state, level: 'ok', title: `${state} suivie${n > 1 ? 's' : ''}` };
}

function enginePill(silent: boolean): StatusPill {
  return silent
    ? {
        id: 'engine',
        label: 'Moteur',
        state: 'Muet',
        level: 'alert',
        title: 'Le moteur ne diffuse plus : valeurs non actualisées',
      }
    : { id: 'engine', label: 'Moteur', state: 'OK', level: 'ok', title: 'Moteur actif' };
}

/** Pastilles d'état, dans l'ordre d'affichage. */
export function statusPills(inputs: ShellInputs): StatusPill[] {
  return [
    positionPill(inputs.position),
    linkPill(inputs.connection),
    aisPill(inputs.ais),
    alarmPill(inputs.alarms),
    enginePill(inputs.silent),
  ];
}

/** Alertes actives (pastilles en défaut), pour le compteur d'en-tête. */
export function activeAlerts(pills: StatusPill[]): string[] {
  return pills.filter((p) => p.level === 'alert').map((p) => `${p.label} : ${p.title}`);
}

/** Date et heure UTC de l'en-tête. */
export function utcClock(now: number): { date: string; time: string } {
  const d = new Date(now);
  const date = new Intl.DateTimeFormat('fr-FR', {
    timeZone: 'UTC',
    weekday: 'short',
    day: 'numeric',
    month: 'short',
    year: 'numeric',
  }).format(d);
  const time = d.toISOString().slice(11, 19);
  return { date, time: `${time} UTC` };
}

/**
 * Date et heure locales de l'en-tête, avec le décalage horaire
 * (« UTC−10 »). L'heure UTC reste disponible par `utcClock`.
 * `timeZone` : fuseau imposé (tests) ; par défaut celui du système.
 */
export function localClock(
  now: number,
  timeZone?: string,
): { date: string; time: string; zone: string } {
  const d = new Date(now);
  const tz = timeZone ? { timeZone } : {};
  const date = new Intl.DateTimeFormat('fr-FR', {
    ...tz,
    weekday: 'short',
    day: 'numeric',
    month: 'short',
    year: 'numeric',
  }).format(d);
  const time = new Intl.DateTimeFormat('fr-FR', {
    ...tz,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hourCycle: 'h23',
  }).format(d);
  const zone =
    new Intl.DateTimeFormat('fr-FR', { ...tz, timeZoneName: 'shortOffset' })
      .formatToParts(d)
      .find((p) => p.type === 'timeZoneName')?.value ?? '';
  return { date, time, zone };
}
