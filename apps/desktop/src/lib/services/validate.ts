/** Utilitaires de validation des données reçues du moteur. */

/** Erreur levée lorsqu'une donnée reçue du moteur est invalide. */
export class EngineProtocolError extends Error {
  override name = 'EngineProtocolError';
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function expectRecord(value: unknown, what: string): Record<string, unknown> {
  if (!isRecord(value)) {
    throw new EngineProtocolError(`${what} : objet attendu`);
  }
  return value;
}

export function expectString(value: unknown, what: string): string {
  if (typeof value !== 'string') {
    throw new EngineProtocolError(`${what} : chaîne attendue`);
  }
  return value;
}

export function expectNumber(value: unknown, what: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new EngineProtocolError(`${what} : nombre fini attendu`);
  }
  return value;
}

export function expectInteger(value: unknown, what: string, min: number, max: number): number {
  const n = expectNumber(value, what);
  if (!Number.isInteger(n) || n < min || n > max) {
    throw new EngineProtocolError(`${what} : entier entre ${min} et ${max} attendu`);
  }
  return n;
}

export function optional<T>(value: unknown, parse: (v: unknown) => T): T | null {
  return value === null || value === undefined ? null : parse(value);
}

export function expectBoolean(value: unknown, what: string): boolean {
  if (typeof value !== 'boolean') {
    throw new EngineProtocolError(`${what} : booléen attendu`);
  }
  return value;
}

/** Horodatage RFC 3339 → millisecondes depuis l'epoch. */
export function expectTimestamp(value: unknown, what: string): number {
  const ms = Date.parse(expectString(value, what));
  if (Number.isNaN(ms)) {
    throw new EngineProtocolError(`${what} : horodatage invalide`);
  }
  return ms;
}

/** Position WGS84 `{ latitudeDeg, longitudeDeg }`, rejetée si hors bornes. */
export interface LatLon {
  latitudeDeg: number;
  longitudeDeg: number;
}

export function expectPosition(value: unknown, what: string): LatLon {
  const r = expectRecord(value, what);
  const latitudeDeg = expectNumber(r.latitudeDeg, `${what}.latitudeDeg`);
  const longitudeDeg = expectNumber(r.longitudeDeg, `${what}.longitudeDeg`);
  if (Math.abs(latitudeDeg) > 90 || Math.abs(longitudeDeg) > 180) {
    throw new EngineProtocolError(`${what} : position hors des bornes WGS84`);
  }
  return { latitudeDeg, longitudeDeg };
}

export function expectArray<T>(value: unknown, what: string, parse: (v: unknown) => T): T[] {
  if (!Array.isArray(value)) {
    throw new EngineProtocolError(`${what} : tableau attendu`);
  }
  return value.map(parse);
}
