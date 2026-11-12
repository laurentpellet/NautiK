/**
 * Formulaire de choix de la source : conversion entre la vue renvoyée
 * par le moteur et la requête `set_sources`. Seule la forme des saisies
 * est vérifiée ici ; le moteur valide tout (adresses, ports, chemins).
 */
import {
  BAUD_RATES,
  DEFAULT_NMEA_PORT,
  type NmeaSettings,
  type SourceMode,
  type SourcesRequest,
  type SourcesView,
} from './services/instruments';

export type NmeaKind = NmeaSettings['kind'];

export interface SourceForm {
  mode: SourceMode;
  signalkUrl: string;
  /** Nouveau jeton saisi (jamais pré-rempli). */
  token: string;
  clearToken: boolean;
  nmeaKind: NmeaKind;
  host: string;
  port: string;
  serialPath: string;
  baudRate: number;
  replayPath: string;
  /** Priorités, une par ligne (ou séparées par des virgules). */
  priorities: string;
}

const DEFAULT_BAUD_RATE = BAUD_RATES[0];

/** Formulaire initialisé à partir des réglages courants. */
export function formFromView(view: SourcesView): SourceForm {
  const nmea = view.nmea;
  return {
    mode: view.mode,
    signalkUrl: view.signalkUrl ?? '',
    token: '',
    clearToken: false,
    nmeaKind: nmea?.kind ?? 'tcp',
    host: nmea?.kind === 'tcp' ? nmea.host : '',
    port: String(nmea?.kind === 'tcp' || nmea?.kind === 'udp' ? nmea.port : DEFAULT_NMEA_PORT),
    serialPath: nmea?.kind === 'serial' ? nmea.path : '',
    baudRate: nmea?.kind === 'serial' ? nmea.baudRate : DEFAULT_BAUD_RATE,
    replayPath: nmea?.kind === 'replay' ? nmea.path : '',
    priorities: view.priorities.join('\n'),
  };
}

function parsePort(raw: string): number {
  const text = raw.trim();
  const port = Number(text);
  if (!/^\d+$/.test(text) || port < 1 || port > 65535) {
    throw new Error('port invalide (1 à 65535)');
  }
  return port;
}

function nmeaFromForm(form: SourceForm): NmeaSettings {
  switch (form.nmeaKind) {
    case 'tcp':
      if (form.host.trim() === '') throw new Error("adresse de l'équipement requise");
      return { kind: 'tcp', host: form.host.trim(), port: parsePort(form.port) };
    case 'udp':
      return { kind: 'udp', port: parsePort(form.port) };
    case 'serial':
      if (form.serialPath.trim() === '') throw new Error('port série requis');
      return { kind: 'serial', path: form.serialPath.trim(), baudRate: form.baudRate };
    case 'replay':
      if (form.replayPath === '') throw new Error("choisissez l'enregistrement à rejouer");
      return { kind: 'replay', path: form.replayPath };
  }
}

/** Liste de priorités saisie. */
export function parsePriorities(text: string): string[] {
  return text
    .split(/[\n,]/)
    .map((p) => p.trim())
    .filter((p) => p !== '');
}

/**
 * Requête `set_sources`. La liaison NMEA n'est reconstruite qu'en mode
 * NMEA ; sinon la liaison enregistrée est conservée telle quelle.
 *
 * @throws Error si une saisie nécessaire au mode choisi est incomplète.
 */
export function requestFromForm(form: SourceForm, previous: SourcesView): SourcesRequest {
  const url = form.signalkUrl.trim();
  return {
    mode: form.mode,
    signalkUrl: url === '' ? null : url,
    nmea: form.mode === 'nmea0183' ? nmeaFromForm(form) : previous.nmea,
    priorities: parsePriorities(form.priorities),
    token: form.token.trim() === '' ? null : form.token,
    clearToken: form.clearToken,
  };
}

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** Description d'une liaison NMEA. */
export function describeNmea(nmea: NmeaSettings): string {
  switch (nmea.kind) {
    case 'tcp':
      return `TCP ${nmea.host}:${nmea.port}`;
    case 'udp':
      return `UDP, port ${nmea.port}`;
    case 'serial':
      return `série ${nmea.path} (${nmea.baudRate} Bd)`;
    case 'replay':
      return `rejeu de ${fileName(nmea.path)}`;
  }
}

/** Description de la source active. */
export function describeSource(view: SourcesView): string {
  switch (view.mode) {
    case 'simulator':
      return 'Simulateur';
    case 'signalK':
      return `Signal K — ${view.signalkUrl ?? '?'}`;
    case 'nmea0183':
      return `NMEA 0183 — ${view.nmea ? describeNmea(view.nmea) : '?'}`;
  }
}
