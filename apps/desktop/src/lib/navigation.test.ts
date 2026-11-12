import { describe, expect, it, vi } from 'vitest';
import { NavigationController } from './navigation.svelte';
import { RoutingApi, type NavigationStatus } from './services/routing';

const p = (latitudeDeg: number, longitudeDeg: number) => ({ latitudeDeg, longitudeDeg });

const rawRoute = (id: string, active = false) => ({
  id,
  name: `Route ${id}`,
  description: null,
  waypoints: [
    { id: `${id}-1`, name: 'WP001', position: p(-14.968, -147.638), description: null },
    { id: `${id}-2`, name: 'WP002', position: p(-14.9, -147.6), description: null },
  ],
  totalDistanceM: 8000,
  active,
});

/** Faux moteur minimal, avec état. */
function fakeEngine() {
  const state = {
    routes: [rawRoute('a')],
    tracks: [] as unknown[],
    activeRouteId: null as string | null,
    recordingTrackId: null as string | null,
  };
  const call = vi.fn((command: string, args?: Record<string, unknown>): Promise<unknown> => {
    switch (command) {
      case 'list_routes':
        return Promise.resolve(
          state.routes.map((r) => ({ ...r, active: r.id === state.activeRouteId })),
        );
      case 'list_tracks':
        return Promise.resolve(state.tracks);
      case 'get_navigation_state':
        return Promise.resolve({
          activeRouteId: state.activeRouteId,
          recordingTrackId: state.recordingTrackId,
        });
      case 'create_route': {
        const positions = args?.positions as unknown[];
        if (positions.length < 2)
          return Promise.reject(new Error('une route contient de 2 à 1000 waypoints'));
        const route = { ...rawRoute('b'), name: String(args?.name) };
        state.routes.push(route);
        return Promise.resolve(route);
      }
      case 'activate_route':
        state.activeRouteId = String(args?.id);
        return Promise.resolve(null);
      case 'deactivate_route':
        state.activeRouteId = null;
        return Promise.resolve(null);
      case 'delete_route':
        return state.activeRouteId === args?.id
          ? Promise.reject(new Error('route active'))
          : Promise.resolve(null);
      case 'start_track_recording':
        state.recordingTrackId = 't1';
        return Promise.resolve({
          id: 't1',
          name: 'SIMULATION — Trace',
          startedAt: '2026-11-12T10:00:00Z',
          endedAt: null,
          isRecording: true,
          pointCount: 0,
        });
      case 'stop_track_recording':
        state.recordingTrackId = null;
        return Promise.resolve(null);
      case 'get_track':
        return Promise.resolve({
          id: args?.id,
          name: 'Trace',
          lengthM: 10,
          points: [{ position: p(0, 0) }, { position: p(0, 0.001) }],
        });
      case 'import_gpx':
        return Promise.resolve({ waypoints: 0, routes: 1, tracks: 0, warnings: ['x'] });
      default:
        return Promise.reject(new Error(`commande inattendue ${command}`));
    }
  });
  return { state, call };
}

describe('NavigationController', () => {
  it('trace une route point par point puis l’enregistre', async () => {
    const { call } = fakeEngine();
    const c = new NavigationController(new RoutingApi(call));
    c.startDrawing();
    c.addPoint(p(-14.968, -147.638));
    c.addPoint(p(-14.95, -147.62));
    c.addPoint(p(-14.9, -147.6));
    c.undoPoint();
    expect(c.draft).toHaveLength(2);
    expect(await c.finishDrawing('Sortie')).toBe(true);
    expect(c.drawing).toBe(false);
    expect(c.draft).toEqual([]);
    expect(c.routes.map((r) => r.name)).toContain('Sortie');
    expect(call).toHaveBeenCalledWith('create_route', {
      name: 'Sortie',
      positions: [p(-14.968, -147.638), p(-14.95, -147.62)],
    });
  });

  it('ignore les clics hors mode tracé et garde le tracé en cas de refus', async () => {
    const { call } = fakeEngine();
    const c = new NavigationController(new RoutingApi(call));
    c.addPoint(p(0, 0));
    expect(c.draft).toEqual([]);
    c.startDrawing();
    c.addPoint(p(0, 0));
    expect(await c.finishDrawing('X')).toBe(false);
    expect(c.error).toMatch(/2 à 1000/);
    expect(c.drawing).toBe(true);
    expect(c.draft).toHaveLength(1);
  });

  it('active puis désactive la navigation', async () => {
    const { call } = fakeEngine();
    const c = new NavigationController(new RoutingApi(call));
    await c.refresh();
    await c.activate('a');
    expect(c.activeRouteId).toBe('a');
    expect(c.routes[0]?.active).toBe(true);
    await c.deleteRoute('a');
    expect(c.error).toBe('route active');
    c.onStatus({ reachedWaypoint: 'WP001', finished: false } as NavigationStatus);
    expect(c.notice).toBe('Waypoint atteint : WP001');
    await c.refresh();
    expect(c.error, "l'erreur reste visible après rechargement").toBe('route active');
    await c.deactivate();
    expect(c.error).toBeNull();
    expect(c.status).toBeNull();
    expect(c.activeRouteId).toBeNull();
  });

  it("enregistre une trace et l'affiche", async () => {
    const { call } = fakeEngine();
    const c = new NavigationController(new RoutingApi(call));
    await c.startRecording();
    expect(c.recordingId).toBe('t1');
    expect(c.displayedTracks).toEqual([{ id: 't1', points: [], recording: true }]);
    await c.refreshRecording();
    expect(c.displayedTracks[0]?.points).toHaveLength(2);
    await c.stopRecording();
    expect(c.recordingId).toBeNull();
    expect(c.displayedTracks[0]?.recording).toBe(false);
    await c.toggleTrack('t1');
    expect(c.displayedTracks).toEqual([]);
  });

  it("résume l'import et signale les éléments écartés", async () => {
    const { call } = fakeEngine();
    const c = new NavigationController(new RoutingApi(call));
    await c.importGpx();
    expect(c.notice).toContain('1 route(s)');
    expect(c.notice).toContain('1 élément(s) écarté(s)');
  });

  it('expose les erreurs du moteur sans planter', async () => {
    const c = new NavigationController(new RoutingApi(() => Promise.reject(new Error('IPC'))));
    await c.refresh();
    expect(c.error).toBe('IPC');
    expect(c.busy).toBe(false);
  });
});
