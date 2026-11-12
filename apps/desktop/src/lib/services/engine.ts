import { invoke } from '@tauri-apps/api/core';

/** Informations du moteur Rust, telles qu'exposées par la commande `get_engine_info`. */
export interface EngineInfo {
  name: string;
  version: string;
  protocolVersion: number;
  safetyNotice: string;
}

/** Version du protocole d'échange attendue par cette interface. */
export const SUPPORTED_PROTOCOL_VERSION = 1;

/** Erreur levée lorsque la réponse du moteur est invalide ou incompatible. */
export class EngineProtocolError extends Error {
  override name = 'EngineProtocolError';
}

/** Signature minimale de la fonction d'appel IPC, injectable pour les tests. */
export type InvokeFn = (command: string) => Promise<unknown>;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** Valide une réponse brute du moteur. Aucune donnée n'est supposée valide sans contrôle. */
export function parseEngineInfo(raw: unknown): EngineInfo {
  if (!isRecord(raw)) {
    throw new EngineProtocolError('Réponse du moteur invalide : objet attendu');
  }
  const { name, version, protocolVersion, safetyNotice } = raw;
  if (
    typeof name !== 'string' ||
    typeof version !== 'string' ||
    typeof safetyNotice !== 'string' ||
    typeof protocolVersion !== 'number' ||
    !Number.isInteger(protocolVersion)
  ) {
    throw new EngineProtocolError('Réponse du moteur invalide : champs manquants ou mal typés');
  }
  if (protocolVersion !== SUPPORTED_PROTOCOL_VERSION) {
    throw new EngineProtocolError(
      `Protocole moteur ${protocolVersion} non supporté (attendu ${SUPPORTED_PROTOCOL_VERSION})`,
    );
  }
  return { name, version, protocolVersion, safetyNotice };
}

/** Interroge le moteur Rust et retourne ses informations validées. */
export async function fetchEngineInfo(call: InvokeFn = invoke): Promise<EngineInfo> {
  return parseEngineInfo(await call('get_engine_info'));
}
