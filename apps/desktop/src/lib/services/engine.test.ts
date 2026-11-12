import { describe, expect, it, vi } from 'vitest';
import {
  EngineProtocolError,
  SUPPORTED_PROTOCOL_VERSION,
  fetchEngineInfo,
  parseEngineInfo,
} from './engine';

const valid = {
  name: 'NautiK',
  version: '0.1.0',
  protocolVersion: SUPPORTED_PROTOCOL_VERSION,
  safetyNotice: "Ce logiciel n'est pas un ECDIS certifié.",
};

describe('parseEngineInfo', () => {
  it('accepte une réponse valide', () => {
    expect(parseEngineInfo(valid)).toEqual(valid);
  });

  it('ignore les champs supplémentaires', () => {
    expect(parseEngineInfo({ ...valid, extra: true })).toEqual(valid);
  });

  it.each([null, undefined, 42, 'NautiK', [valid]])('rejette une valeur non objet (%s)', (raw) => {
    expect(() => parseEngineInfo(raw)).toThrow(EngineProtocolError);
  });

  it('rejette un champ manquant', () => {
    const partial: Partial<typeof valid> = { ...valid };
    delete partial.safetyNotice;
    expect(() => parseEngineInfo(partial)).toThrow(EngineProtocolError);
  });

  it('rejette un champ mal typé', () => {
    expect(() => parseEngineInfo({ ...valid, version: 1 })).toThrow(EngineProtocolError);
  });

  it('rejette une version de protocole non entière', () => {
    expect(() => parseEngineInfo({ ...valid, protocolVersion: 1.5 })).toThrow(EngineProtocolError);
  });

  it('rejette une version de protocole non supportée', () => {
    expect(() => parseEngineInfo({ ...valid, protocolVersion: 99 })).toThrow(/non supporté/);
  });
});

describe('fetchEngineInfo', () => {
  it('appelle la commande get_engine_info', async () => {
    const call = vi.fn().mockResolvedValue(valid);
    await expect(fetchEngineInfo(call)).resolves.toEqual(valid);
    expect(call).toHaveBeenCalledWith('get_engine_info');
  });

  it("propage l'échec de l'IPC", async () => {
    const call = vi.fn().mockRejectedValue(new Error('ipc down'));
    await expect(fetchEngineInfo(call)).rejects.toThrow('ipc down');
  });
});
