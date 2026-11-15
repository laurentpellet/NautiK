import { describe, expect, it, vi } from 'vitest';
import { DEFAULT_PLAN_SPEED_MPS, NavigationController, loadPlanSpeed } from './navigation.svelte';
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
  bounds: { west: -147.638, south: -14.968, east: -147.6, north: -14.9 },
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
      case 'save_route': {
        const waypoints = args?.waypoints as { id: string | null; name: string }[];
        if (waypoints.length < 2)
          return Promise.reject(new Error('une route contient de 2 à 1000 waypoints'));
        const id = typeof args?.id === 'string' ? args.id : 'b';
        const route = {
          ...rawRoute(id),
          name: String(args?.name),
          waypoints: waypoints.map((w, i) => ({
            id: w.id ?? `${id}-new-${i}`,
            name: w.name || `WP00${i + 1}`,
            position: p(0, i),
            description: null,
          })),
        };
        state.routes = [...state.routes.filter((r) => r.id !== id), route];
        return Promise.resolve(route);
      }
      case 'plan_route': {
        const positions = args?.positions as unknown[];
        const speed = Number(args?.speedMps);
        return Promise.resolve({
          speedMps: speed,
          departure: args?.departure ?? null,
          legs: positions.slice(1).map((_, i) => ({
            fromIndex: i,
            distanceM: 1000,
            bearingRad: 0,
            durationS: 1000 / speed,
            cumulativeDistanceM: 1000 * (i + 1),
            cumulativeDurationS: (1000 * (i + 1)) / speed,
            arrival: null,
          })),
          totalDistanceM: 1000 * (positions.length - 1),
          totalDurationS: (1000 * (positions.length - 1)) / speed,
          arrival: null,
          bounds: { west: 0, south: 0, east: 1, north: 1 },
        });
      }
      case 'rename_track':
        return String(args?.name).trim() === ''
          ? Promise.reject(new Error('nom invalide'))
          : Promise.resolve(null);
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
          durationS: 60,
          bounds: { west: 0, south: 0, east: 0.001, north: 0 },
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
  it('trace une nouvelle route avec départ, arrivée et noms', async () => {
    const { call } = fakeEngine();
    const c = new NavigationController(new RoutingApi(call));
    c.startDrawing();
    expect(c.drawing).toBe(true);
    c.addPoint(p(-14.968, -147.638));
    c.addPoint(p(-14.95, -147.62));
    c.setInsert('arrival');
    c.addPoint(p(-14.9, -147.6));
    c.setInsert('start');
    c.addPoint(p(-15, -147.7));
    c.setInsert(2);
    c.addPoint(p(-14.96, -147.63));
    c.renamePoint(3, 'Passe');
    expect(c.draft.map((d) => d.name)).toEqual(['Départ', 'Départ', '', 'Passe', 'Arrivée']);
    expect(c.editor?.insert, 'retour en fin de route').toBe('end');
    c.removePoint(1);
    c.movePoint(0, p(-15.1, -147.7));
    c.undoPoint();
    expect(c.draft).toHaveLength(3);
    if (c.editor) c.editor.name = 'Sortie';
    expect(await c.saveEdit()).toBe(true);
    expect(c.drawing).toBe(false);
    expect(c.draft).toEqual([]);
    expect(c.selectedRouteId).toBe('b');
    expect(c.routes.map((r) => r.name)).toContain('Sortie');
    expect(call).toHaveBeenCalledWith('save_route', {
      id: null,
      name: 'Sortie',
      waypoints: [
        { id: null, name: 'Départ', position: p(-15.1, -147.7) },
        { id: null, name: '', position: p(-14.96, -147.63) },
        { id: null, name: 'Passe', position: p(-14.95, -147.62) },
      ],
    });
  });

  it('ignore les clics hors édition et garde le tracé en cas de refus', async () => {
    const { call } = fakeEngine();
    const c = new NavigationController(new RoutingApi(call));
    c.addPoint(p(0, 0));
    expect(c.draft).toEqual([]);
    c.startDrawing();
    c.addPoint(p(0, 0));
    c.movePoint(5, p(1, 1));
    c.renamePoint(-1, 'x');
    c.removePoint(9);
    expect(await c.saveEdit()).toBe(false);
    expect(c.error).toMatch(/2 à 1000/);
    expect(c.drawing).toBe(true);
    expect(c.draft).toHaveLength(1);
  });

  it('modifie une route existante en gardant ses waypoints et cadre la carte', async () => {
    const { call } = fakeEngine();
    const c = new NavigationController(new RoutingApi(call));
    await c.refresh();
    c.editRoute('a');
    expect(c.focus?.bounds.west).toBe(-147.638);
    expect(c.draft.map((d) => d.id)).toEqual(['a-1', 'a-2']);
    c.renamePoint(0, 'Port');
    expect(await c.saveEdit()).toBe(true);
    expect(call).toHaveBeenCalledWith(
      'save_route',
      expect.objectContaining({ id: 'a', name: 'Route a' }),
    );
    c.editRoute('inconnue');
    expect(c.editor).toBeNull();
  });

  it('planifie le parcours édité ou sélectionné', async () => {
    const { call } = fakeEngine();
    const c = new NavigationController(new RoutingApi(call));
    await c.refresh();
    await c.refreshPlan();
    expect(c.plan).toBeNull();
    c.selectRoute('a');
    expect(c.focus?.seq).toBe(1);
    c.setSpeed(5);
    c.setSpeed(0);
    c.setSpeed(Number.NaN);
    expect(c.speedMps).toBe(5);
    c.setDeparture(Date.UTC(2026, 10, 14, 6));
    await c.refreshPlan();
    expect(c.plan?.totalDurationS).toBe(200);
    expect(call).toHaveBeenCalledWith('plan_route', {
      positions: [p(-14.968, -147.638), p(-14.9, -147.6)],
      speedMps: 5,
      departure: '2026-11-14T06:00:00.000Z',
    });
    c.startDrawing();
    c.addPoint(p(0, 0));
    await c.refreshPlan();
    expect(c.plan, 'moins de deux points : pas de plan').toBeNull();
  });

  it('mémorise la vitesse prévue et rejette les valeurs invalides', () => {
    expect(loadPlanSpeed({ getItem: () => null })).toBeCloseTo(DEFAULT_PLAN_SPEED_MPS);
    const storage = (value: string) => ({ getItem: () => value });
    expect(loadPlanSpeed(storage('{"speedMps":4}'))).toBe(4);
    expect(loadPlanSpeed(storage('{"speedMps":99}'))).toBe(DEFAULT_PLAN_SPEED_MPS);
    expect(loadPlanSpeed(storage('pas du json'))).toBe(DEFAULT_PLAN_SPEED_MPS);
  });

  it('affiche une trace avec sa durée et sa longueur, la cadre et la renomme', async () => {
    const { call } = fakeEngine();
    const c = new NavigationController(new RoutingApi(call));
    await c.toggleTrack('t9');
    expect(c.trackInfo.t9).toEqual({
      lengthM: 10,
      durationS: 60,
      bounds: { west: 0, south: 0, east: 0.001, north: 0 },
    });
    expect(c.focus?.bounds.east).toBe(0.001);
    await c.zoomTrack('t9');
    expect(c.focus?.seq).toBe(2);
    expect(await c.renameTrack('t9', 'Sortie')).toBe(true);
    expect(await c.renameTrack('t9', ' ')).toBe(false);
    expect(c.error).toBe('nom invalide');
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

  it('navigue jusqu’à un point : route directe créée puis activée', async () => {
    const { call, state } = fakeEngine();
    const c = new NavigationController(new RoutingApi(call));
    await c.refresh();
    const now = Date.UTC(2026, 10, 14, 9, 5);
    expect(await c.goTo(p(-17.5, -149.6), p(-17.6, -149.7), now)).toBe(true);
    expect(call).toHaveBeenCalledWith('save_route', {
      id: null,
      name: 'Direct 09:05 UTC',
      waypoints: [
        { id: null, name: 'Départ', position: p(-17.5, -149.6) },
        { id: null, name: 'Destination', position: p(-17.6, -149.7) },
      ],
    });
    expect(state.activeRouteId).toBe('b');
    expect(c.activeRouteId).toBe('b');
    expect(c.selectedRouteId).toBe('b');
    expect(c.notice).toBe('Route « Direct 09:05 UTC » active');
  });

  it('refuse de naviguer sans position fraîche ou pendant un tracé', async () => {
    const { call } = fakeEngine();
    const c = new NavigationController(new RoutingApi(call));
    expect(await c.goTo(null, p(0, 1))).toBe(false);
    expect(c.error).toMatch(/Position du bateau inconnue/);
    c.startDrawing();
    expect(await c.goTo(p(0, 0), p(0, 1))).toBe(false);
    expect(c.error).toMatch(/route en cours d’édition/);
    expect(call).not.toHaveBeenCalledWith('save_route', expect.anything());
  });

  it('garde l’erreur si l’activation est refusée', async () => {
    const { call } = fakeEngine();
    const base = call.getMockImplementation();
    call.mockImplementation((command: string, args?: Record<string, unknown>) =>
      command === 'activate_route'
        ? Promise.reject(new Error('route introuvable'))
        : (base?.(command, args) ?? Promise.resolve(null)),
    );
    const c = new NavigationController(new RoutingApi(call));
    expect(await c.goTo(p(0, 0), p(0, 1))).toBe(false);
    expect(c.error).toBe('route introuvable');
    expect(c.activeRouteId).toBeNull();
  });
});

describe('route active et recherche de départ/arrivée', () => {
  it('obtient du moteur la longueur des segments de la route active', async () => {
    const { call } = fakeEngine();
    const c = new NavigationController(new RoutingApi(call));
    await c.refresh();
    expect(c.activeLegs).toBeNull();
    await c.activate('a');
    expect(c.activeLegs).toEqual([1000]);
    const plans = () => call.mock.calls.filter(([command]) => command === 'plan_route').length;
    const before = plans();
    await c.refresh();
    expect(plans(), 'route inchangée : pas de recalcul').toBe(before);
    await c.deactivate();
    expect(c.activeLegs).toBeNull();
  });

  it('n’invente pas de longueurs si le moteur refuse le calcul', async () => {
    const { call } = fakeEngine();
    const base = call.getMockImplementation();
    call.mockImplementation((command: string, args?: Record<string, unknown>) =>
      command === 'plan_route'
        ? Promise.reject(new Error('plan indisponible'))
        : (base?.(command, args) ?? Promise.resolve(null)),
    );
    const c = new NavigationController(new RoutingApi(call));
    await c.activate('a');
    expect(c.activeRouteId).toBe('a');
    expect(c.activeLegs).toBeNull();
  });

  it('place le départ et l’arrivée trouvés, en remplaçant ceux existants', () => {
    const c = new NavigationController(new RoutingApi(fakeEngine().call));
    c.setEndpoint('start', 'Ignoré', p(0, 0));
    expect(c.draft).toEqual([]);
    c.startDrawing();
    c.setEndpoint('arrival', 'Passe Taapuna', p(1, 1));
    c.setEndpoint('start', ' Papeete ', p(0, 0));
    expect(c.draft.map((w) => w.name)).toEqual(['Papeete', 'Passe Taapuna']);
    c.addPoint(p(2, 2));
    c.setEndpoint('arrival', 'Moorea', p(3, 3));
    c.setEndpoint('start', 'x'.repeat(80), p(4, 4));
    expect(c.draft.map((w) => w.position)).toEqual([p(4, 4), p(1, 1), p(3, 3)]);
    expect(c.draft[0]?.name).toHaveLength(50);
  });
});
