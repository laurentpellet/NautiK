import { invoke } from '@tauri-apps/api/core';
import { EngineProtocolError, expectInteger, expectRecord, expectString } from './validate';

export { EngineProtocolError } from './validate';

/** Informations du moteur Rust, telles qu'exposées par la commande `get_engine_info`. */
export interface EngineInfo {
  name: string;
  version: string;
  protocolVersion: number;
  safetyNotice: string;
}

/** Version du protocole d'échange attendue par cette interface. */
export const SUPPORTED_PROTOCOL_VERSION = 1;

/** Signature minimale de la fonction d'appel IPC, injectable pour les tests. */
export type InvokeFn = (command: string) => Promise<unknown>;

/** Valide une réponse brute du moteur. Aucune donnée n'est supposée valide sans contrôle. */
export function parseEngineInfo(raw: unknown): EngineInfo {
  let info: EngineInfo;
  try {
    const r = expectRecord(raw, 'objet attendu');
    info = {
      name: expectString(r.name, 'name'),
      version: expectString(r.version, 'version'),
      protocolVersion: expectInteger(r.protocolVersion, 'protocolVersion', 0, 2 ** 31),
      safetyNotice: expectString(r.safetyNotice, 'safetyNotice'),
    };
  } catch (e) {
    const detail = e instanceof Error ? e.message : String(e);
    throw new EngineProtocolError(`Réponse du moteur invalide : ${detail}`);
  }
  if (info.protocolVersion !== SUPPORTED_PROTOCOL_VERSION) {
    throw new EngineProtocolError(
      `Protocole moteur ${info.protocolVersion} non supporté (attendu ${SUPPORTED_PROTOCOL_VERSION})`,
    );
  }
  return info;
}

/** Interroge le moteur Rust et retourne ses informations validées. */
export async function fetchEngineInfo(call: InvokeFn = invoke): Promise<EngineInfo> {
  return parseEngineInfo(await call('get_engine_info'));
}
