/** Réglages du bateau simulé (vitesse en m/s, validée par le moteur). */
import { invoke } from '@tauri-apps/api/core';
import type { InvokeFn } from './engine';
import { EngineProtocolError, expectNumber, expectRecord } from './validate';

/** Vitesse simulée maximale acceptée par le moteur : 30 nœuds. */
export const MAX_SIMULATED_SPEED_MPS = (30 * 1852) / 3600;

export interface SimulatorSettings {
  speedMps: number;
}

export function parseSimulatorSettings(value: unknown): SimulatorSettings {
  const r = expectRecord(value, 'simulateur');
  const speedMps = expectNumber(r.speedMps, 'speedMps');
  if (speedMps < 0 || speedMps > MAX_SIMULATED_SPEED_MPS + 1e-9) {
    throw new EngineProtocolError(`vitesse simulée hors bornes : ${speedMps}`);
  }
  return { speedMps };
}

export class SimulatorApi {
  constructor(private readonly invokeFn: InvokeFn = invoke) {}

  async get(): Promise<SimulatorSettings> {
    return parseSimulatorSettings(await this.invokeFn('get_simulator_settings'));
  }

  async setSpeed(speedMps: number): Promise<SimulatorSettings> {
    return parseSimulatorSettings(await this.invokeFn('set_simulator_settings', { speedMps }));
  }
}
