import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/svelte';
import MeasurePanel from './MeasurePanel.svelte';
import { MapToolsController } from '../mapTools.svelte';
import { RoutingApi } from '../services/routing';
import { SimulatorApi } from '../services/simulator';

const p = (latitudeDeg: number, longitudeDeg: number) => ({ latitudeDeg, longitudeDeg });

function setup() {
  const call = vi.fn((command: string, args?: Record<string, unknown>): Promise<unknown> => {
    if (command !== 'plan_route') return Promise.reject(new Error(command));
    const n = (args?.positions as unknown[]).length;
    return Promise.resolve({
      speedMps: 3,
      departure: null,
      legs: Array.from({ length: n - 1 }, (_, i) => ({
        fromIndex: i,
        distanceM: 1852,
        bearingRad: Math.PI,
        durationS: 600,
        cumulativeDistanceM: 1852 * (i + 1),
        cumulativeDurationS: 600 * (i + 1),
        arrival: null,
      })),
      totalDistanceM: 1852 * (n - 1),
      totalDurationS: 600 * (n - 1),
      arrival: null,
      bounds: { west: 0, south: 0, east: 1, north: 1 },
    });
  });
  return new MapToolsController(new RoutingApi(call), new SimulatorApi(call), () => 3);
}

describe('panneau de mesure', () => {
  it('propose la conversion en route à partir de deux points', async () => {
    const tools = setup();
    tools.toggleMeasure();
    const onconvert = vi.fn();
    render(MeasurePanel, { tools, onconvert });
    await tools.click(p(0, 0));
    expect(screen.queryByRole('button', { name: 'Convertir en route' })).toBeNull();
    await tools.click(p(-0.03, 0));
    await fireEvent.click(await screen.findByRole('button', { name: 'Convertir en route' }));
    expect(onconvert).toHaveBeenCalledOnce();
  });
});
