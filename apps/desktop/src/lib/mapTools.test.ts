import { describe, expect, it, vi } from 'vitest';
import { MAX_MEASURE_POINTS, MAX_VRM_M, MapToolsController } from './mapTools.svelte';
import { RoutingApi } from './services/routing';
import { SimulatorApi } from './services/simulator';

const p = (latitudeDeg: number, longitudeDeg: number) => ({ latitudeDeg, longitudeDeg });

function planOf(args: Record<string, unknown> | undefined) {
  const n = (args?.positions as unknown[]).length;
  return {
    speedMps: args?.speedMps,
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
  };
}

function setup(
  place: () => Promise<unknown> = () => Promise.resolve(null),
  boat: { latitudeDeg: number; longitudeDeg: number } | null = null,
) {
  const calls: [string, Record<string, unknown> | undefined][] = [];
  const call = vi.fn((command: string, args?: Record<string, unknown>): Promise<unknown> => {
    calls.push([command, args]);
    if (command === 'plan_route') return Promise.resolve(planOf(args));
    if (command === 'place_simulated_boat') return place();
    return Promise.reject(new Error(command));
  });
  const tools = new MapToolsController(
    new RoutingApi(call),
    new SimulatorApi(call),
    () => 3,
    () => boat,
  );
  return { tools, calls };
}

describe('outils de la carte', () => {
  it('mesure avec le moteur : distance, relèvement, annulation', async () => {
    const { tools, calls } = setup();
    tools.toggleMeasure();
    expect(tools.measuring).toBe(true);
    expect(tools.picking).toBe(true);
    await tools.click(p(0, 0));
    expect(tools.plan, 'un seul point : rien à mesurer').toBeNull();
    expect(calls).toHaveLength(0);
    await tools.click(p(-0.03, 0));
    await tools.click(p(-0.06, 0));
    expect(tools.plan?.totalDistanceM).toBe(3704);
    expect(calls.at(-1)).toEqual([
      'plan_route',
      { positions: [p(0, 0), p(-0.03, 0), p(-0.06, 0)], speedMps: 3, departure: null },
    ]);
    await tools.undoPoint();
    expect(tools.points).toHaveLength(2);
    expect(tools.plan?.legs).toHaveLength(1);
    tools.toggleMeasure();
    expect(tools.tool).toBe('none');
    expect(tools.points, 'mesure effacée en quittant').toEqual([]);
  });

  it('commence une mesure depuis le menu contextuel', async () => {
    const { tools } = setup();
    tools.togglePlaceBoat();
    await tools.measureFrom(p(1, 2));
    expect(tools.tool).toBe('measure');
    expect(tools.points).toEqual([p(1, 2)]);
  });

  it('limite le nombre de points', async () => {
    const { tools } = setup();
    tools.toggleMeasure();
    tools.points = Array.from({ length: MAX_MEASURE_POINTS }, (_, i) => p(0, i / 1000));
    await tools.click(p(1, 1));
    expect(tools.points).toHaveLength(MAX_MEASURE_POINTS);
    expect(tools.error).toMatch(/limitée/);
  });

  it('signale une mesure refusée par le moteur', async () => {
    const call = vi.fn(() => Promise.reject(new Error('vitesse invalide')));
    const tools = new MapToolsController(new RoutingApi(call), new SimulatorApi(call), () => 3);
    tools.toggleMeasure();
    await tools.click(p(0, 0));
    await tools.click(p(0, 1));
    expect(tools.plan).toBeNull();
    expect(tools.error).toBe('Mesure indisponible : vitesse invalide');
  });

  it('place le bateau simulé puis referme l’outil', async () => {
    const { tools, calls } = setup();
    tools.togglePlaceBoat();
    expect(tools.tool).toBe('placeBoat');
    await tools.click(p(-17.5, -149.6));
    expect(calls).toEqual([['place_simulated_boat', { position: p(-17.5, -149.6) }]]);
    expect(tools.tool).toBe('none');
    expect(tools.notice).toMatch(/déplacé/);
  });

  it('garde l’outil et affiche le refus du moteur (source réelle)', async () => {
    const { tools } = setup(() =>
      Promise.reject(new Error('le bateau ne peut être placé qu’en mode simulateur')),
    );
    tools.togglePlaceBoat();
    expect(await tools.placeBoat(p(0, 0))).toBe(false);
    expect(tools.tool).toBe('placeBoat');
    expect(tools.error).toMatch(/simulateur/);
    expect(tools.notice).toBeNull();
  });

  it('règle le VRM/EBL par un clic (moteur) ou à la main', async () => {
    const { tools, calls } = setup(undefined, p(-17.5, -149.6));
    tools.toggleRangeBearing();
    expect(tools.tool).toBe('rangeBearing');
    await tools.click(p(-17.53, -149.6));
    expect(calls.at(-1)).toEqual([
      'plan_route',
      { positions: [p(-17.5, -149.6), p(-17.53, -149.6)], speedMps: 3, departure: null },
    ]);
    expect(tools.rangeBearing).toEqual({ radiusM: 1852, bearingRad: Math.PI });
    tools.setRangeBearing({ radiusM: 500, bearingRad: -Math.PI / 2 });
    expect(tools.rangeBearing.bearingRad).toBeCloseTo((3 * Math.PI) / 2);
    tools.setRangeBearing({ radiusM: MAX_VRM_M + 1, bearingRad: 0 });
    expect(tools.error).toMatch(/Rayon invalide/);
    expect(tools.rangeBearing.radiusM, 'valeur précédente conservée').toBe(500);
    tools.setRangeBearing({ radiusM: null, bearingRad: Number.NaN });
    expect(tools.error).toMatch(/Relèvement invalide/);
    tools.toggleRangeBearing();
    expect(tools.rangeBearing).toEqual({ radiusM: null, bearingRad: null });
  });

  it('refuse le VRM/EBL sans position du bateau', async () => {
    const { tools, calls } = setup();
    tools.toggleRangeBearing();
    await tools.click(p(0, 0));
    expect(calls).toHaveLength(0);
    expect(tools.error).toMatch(/Position du bateau inconnue/);
  });
});
