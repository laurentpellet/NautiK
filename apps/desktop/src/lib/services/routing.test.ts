import { describe, expect, it, vi } from 'vitest';
import {
  NAVIGATION_EVENT,
  RoutingApi,
  parseNavigationStatus,
  parseRoute,
  parseTrack,
  parseTrackSummary,
  subscribeNavigation,
} from './routing';
import { EngineProtocolError } from './validate';

const wp = (id: string, lat: number, lon: number) => ({
  id,
  name: id,
  position: { latitudeDeg: lat, longitudeDeg: lon },
  description: null,
  icon: null,
  color: null,
  createdAt: '2026-11-12T10:00:00Z',
  updatedAt: '2026-11-12T10:00:00Z',
});

const rawRoute = {
  id: 'r1',
  name: 'Passe',
  description: null,
  waypoints: [wp('a', -14.968, -147.638), wp('b', -14.9, -147.6)],
  createdAt: '2026-11-12T10:00:00Z',
  updatedAt: '2026-11-12T10:00:00Z',
  totalDistanceM: 8500.5,
  active: false,
};

const rawStatus = {
  routeId: 'r1',
  routeName: 'Passe',
  targetIndex: 0,
  waypointCount: 2,
  targetName: 'a',
  targetPosition: { latitudeDeg: -14.968, longitudeDeg: -147.638 },
  distanceToTargetM: 120,
  bearingToTargetRad: 0.5,
  crossTrackM: null,
  remainingDistanceM: 8620,
  etaTarget: '2026-11-12T10:01:00Z',
  etaEnd: null,
  reachedWaypoint: null,
  finished: false,
  simulated: true,
  computedAt: '2026-11-12T10:00:30.123456789Z',
};

describe('validation des données de routage', () => {
  it('accepte une route valide', () => {
    const route = parseRoute(rawRoute);
    expect(route.waypoints).toHaveLength(2);
    expect(route.waypoints[1]?.position.latitudeDeg).toBe(-14.9);
    expect(route.totalDistanceM).toBe(8500.5);
  });

  it('rejette une route mal formée', () => {
    expect(() => parseRoute({ ...rawRoute, totalDistanceM: 'x' })).toThrow(EngineProtocolError);
    expect(() => parseRoute({ ...rawRoute, waypoints: [wp('a', 95, 0)] })).toThrow(/WGS84/);
    expect(() => parseRoute({ ...rawRoute, active: 1 })).toThrow(EngineProtocolError);
    expect(() => parseRoute(null)).toThrow(EngineProtocolError);
  });

  it('valide un état de navigation', () => {
    const status = parseNavigationStatus(rawStatus);
    expect(status.etaTarget).toBe(Date.parse('2026-11-12T10:01:00Z'));
    expect(status.crossTrackM).toBeNull();
    expect(status.simulated).toBe(true);
    expect(() => parseNavigationStatus({ ...rawStatus, distanceToTargetM: NaN })).toThrow();
    expect(() => parseNavigationStatus({ ...rawStatus, etaTarget: 'demain' })).toThrow(
      /horodatage/,
    );
    expect(() => parseNavigationStatus({ ...rawStatus, targetIndex: -1 })).toThrow();
  });

  it('valide traces et résumés', () => {
    const summary = parseTrackSummary({
      id: 't',
      name: 'Sortie',
      startedAt: '2026-11-12T10:00:00Z',
      endedAt: null,
      isRecording: true,
      pointCount: 3,
    });
    expect(summary.endedAt).toBeNull();
    const track = parseTrack({
      id: 't',
      name: 'Sortie',
      startedAt: '2026-11-12T10:00:00Z',
      endedAt: null,
      lengthM: 10,
      points: [
        {
          timestamp: '2026-11-12T10:00:00Z',
          position: { latitudeDeg: 1, longitudeDeg: 2 },
          sogMps: null,
          cogRad: null,
          depthM: null,
        },
      ],
    });
    expect(track.points).toEqual([{ latitudeDeg: 1, longitudeDeg: 2 }]);
    expect(() =>
      parseTrack({ id: 't', name: 'x', lengthM: 0, points: [{ position: null }] }),
    ).toThrow();
  });
});

describe('RoutingApi', () => {
  it('transmet les arguments attendus par le moteur', async () => {
    const call = vi.fn((command: string) =>
      Promise.resolve(command === 'create_route' ? rawRoute : null),
    );
    const api = new RoutingApi(call);
    const positions = [
      { latitudeDeg: -14.968, longitudeDeg: -147.638 },
      { latitudeDeg: -14.9, longitudeDeg: -147.6 },
    ];
    await api.createRoute('Passe', positions);
    expect(call).toHaveBeenCalledWith('create_route', { name: 'Passe', positions });
    await api.deleteRoute('r1');
    expect(call).toHaveBeenLastCalledWith('delete_route', { id: 'r1' });
  });

  it("traite l'annulation d'un import comme une absence de résultat", async () => {
    const api = new RoutingApi(() => Promise.resolve(null));
    expect(await api.importGpx()).toBeNull();
  });

  it("valide le bilan d'import", async () => {
    const api = new RoutingApi(() =>
      Promise.resolve({ waypoints: 1, routes: 2, tracks: 0, warnings: ['route écartée'] }),
    );
    expect(await api.importGpx()).toEqual({
      waypoints: 1,
      routes: 2,
      tracks: 0,
      warnings: ['route écartée'],
    });
  });

  it('propage les erreurs du moteur', async () => {
    const api = new RoutingApi(() => Promise.reject(new Error('route active')));
    await expect(api.deleteRoute('r1')).rejects.toThrow('route active');
  });
});

describe('subscribeNavigation', () => {
  it("transmet l'état, la désactivation et les erreurs", async () => {
    let handler: ((e: { payload: unknown }) => void) | undefined;
    const listen = vi.fn((event: string, h: (e: { payload: unknown }) => void) => {
      expect(event).toBe(NAVIGATION_EVENT);
      handler = h;
      return Promise.resolve(() => {});
    });
    const onStatus = vi.fn();
    const onError = vi.fn();
    await subscribeNavigation(onStatus, onError, listen);
    handler?.({ payload: rawStatus });
    handler?.({ payload: null });
    handler?.({ payload: { routeId: 1 } });
    expect(onStatus).toHaveBeenCalledTimes(2);
    expect(onStatus).toHaveBeenLastCalledWith(null);
    expect(onError).toHaveBeenCalledTimes(1);
  });
});
