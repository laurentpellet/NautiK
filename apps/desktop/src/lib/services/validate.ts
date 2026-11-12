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

export function expectArray<T>(value: unknown, what: string, parse: (v: unknown) => T): T[] {
  if (!Array.isArray(value)) {
    throw new EngineProtocolError(`${what} : tableau attendu`);
  }
  return value.map(parse);
}
