import { describe, expect, it, vi } from 'vitest';
import { MAX_SIMULATED_SPEED_MPS, SimulatorApi, parseSimulatorSettings } from './simulator';
import { EngineProtocolError } from './validate';

describe('réglages du simulateur', () => {
  it('valide la vitesse reçue', () => {
    expect(parseSimulatorSettings({ speedMps: 4.1 })).toEqual({ speedMps: 4.1 });
    expect(parseSimulatorSettings({ speedMps: MAX_SIMULATED_SPEED_MPS }).speedMps).toBeCloseTo(
      15.43,
      2,
    );
    for (const bad of [{ speedMps: -1 }, { speedMps: 16 }, { speedMps: 'vite' }, null]) {
      expect(() => parseSimulatorSettings(bad)).toThrow(EngineProtocolError);
    }
  });

  it('transmet les commandes', async () => {
    const invokeFn = vi.fn((command: string, args?: Record<string, unknown>) =>
      Promise.resolve(command === 'get_simulator_settings' ? { speedMps: 4 } : args),
    );
    const api = new SimulatorApi(invokeFn);
    expect(await api.get()).toEqual({ speedMps: 4 });
    expect(await api.setSpeed(5)).toEqual({ speedMps: 5 });
    expect(invokeFn).toHaveBeenLastCalledWith('set_simulator_settings', { speedMps: 5 });
  });
});
