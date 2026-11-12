import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/svelte';
import App from './App.svelte';

// MapLibre requiert WebGL, absent de jsdom.
vi.mock('./lib/components/ChartView.svelte', async () => ({
  default: (await import('./test/ChartViewStub.svelte')).default,
}));

const engine = {
  name: 'NautiK',
  version: '0.1.0',
  protocolVersion: 1,
  safetyNotice: "Ce logiciel n'est pas un ECDIS certifié.",
};

const emptyCatalog = {
  chartsDir: 'C:\\NautiK\\charts',
  tileUrlBase: 'http://nautik-tiles.localhost',
  charts: [],
  failures: [],
  error: null,
};

type Handler = (event: { payload: unknown }) => void;

const routingDefaults: Record<string, () => Promise<unknown>> = {
  list_routes: () => Promise.resolve([]),
  list_tracks: () => Promise.resolve([]),
  get_navigation_state: () => Promise.resolve({ activeRouteId: null, recordingTrackId: null }),
  get_sources: () => Promise.resolve(sourcesView()),
  list_serial_ports: () => Promise.resolve(['COM3']),
};

function sourcesView(patch: Record<string, unknown> = {}) {
  return {
    mode: 'simulator',
    signalkUrl: null,
    nmea: null,
    priorities: [],
    tokenStored: false,
    connection: null,
    rejections: { count: 0, lastReason: null },
    ...patch,
  };
}

function fakeIpc(responses: Record<string, () => Promise<unknown>>) {
  const all = { ...routingDefaults, ...responses };
  return (command: string) =>
    all[command]?.() ?? Promise.reject(new Error(`commande inattendue ${command}`));
}

function fakeListen() {
  const handlers = new Map<string, Handler[]>();
  const listen = (event: string, handler: Handler) => {
    handlers.set(event, [...(handlers.get(event) ?? []), handler]);
    return Promise.resolve(() => {});
  };
  const emit = (payload: unknown, event = 'instruments') =>
    (handlers.get(event) ?? []).forEach((h) => h({ payload }));
  return { listen, emit };
}

function snapshotWith(fix: Record<string, unknown>, stale = false, readings: unknown[] = []) {
  const now = new Date().toISOString();
  return {
    computedAt: now,
    position: {
      fix: { cogRad: null, sogMps: 0, measuredAt: now, receivedAt: now, ...fix },
      ageMs: stale ? 12_000 : 400,
      stale,
      freshSources: 1,
    },
    readings,
  };
}

const navigationStatus = {
  routeId: 'r1',
  routeName: 'Tiputa → Avatoru',
  targetIndex: 0,
  waypointCount: 2,
  targetName: 'WP001',
  targetPosition: { latitudeDeg: -14.968, longitudeDeg: -147.638 },
  distanceToTargetM: 1852,
  bearingToTargetRad: Math.PI,
  crossTrackM: -92.6,
  remainingDistanceM: 18520,
  etaTarget: null,
  etaEnd: null,
  reachedWaypoint: null,
  finished: false,
  simulated: true,
  computedAt: new Date().toISOString(),
};

describe('App', () => {
  it("affiche la version du moteur et l'avertissement de sûreté", async () => {
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.reject(new Error('pas de carte en test')),
      }),
      listen: fakeListen().listen,
    });
    const status = await screen.findByTestId('engine-status');
    expect(status.textContent).toContain('Moteur 0.1.0');
    expect(screen.getByRole('note').textContent).toContain('ECDIS');
  });

  it("signale clairement l'indisponibilité du moteur", async () => {
    render(App, {
      invoke: () => Promise.reject(new Error('ipc down')),
      listen: fakeListen().listen,
    });
    expect(await screen.findByText('Moteur indisponible : ipc down')).toBeTruthy();
    expect(screen.getByText(/Catalogue indisponible : ipc down/)).toBeTruthy();
  });

  it('signale une réponse moteur invalide', async () => {
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve({ name: 'NautiK' }),
        get_chart_catalog: () => Promise.reject(new Error('x')),
      }),
      listen: fakeListen().listen,
    });
    expect((await screen.findByText(/Moteur indisponible/)).textContent).toContain('invalide');
  });

  it("signale l'absence de position puis affiche une position simulée", async () => {
    const events = fakeListen();
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.reject(new Error('pas de carte en test')),
      }),
      listen: events.listen,
    });
    expect(await screen.findByText('Aucune position')).toBeTruthy();

    events.emit(
      snapshotWith({
        position: { latitudeDeg: -17.535, longitudeDeg: -149.569 },
        source: 'simulator',
        sourceId: 'simulator',
      }),
    );
    expect((await screen.findByTestId('position')).textContent).toContain('17°32.100′ S');
    expect(screen.getByText('POSITION SIMULÉE')).toBeTruthy();
    expect(screen.queryByText(/Position obsolète/)).toBeNull();
  });

  it('signale une position Signal K obsolète et affiche les instruments', async () => {
    const events = fakeListen();
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.reject(new Error('x')),
      }),
      listen: events.listen,
    });
    await screen.findByText('Aucune position');
    const now = new Date().toISOString();
    events.emit(
      snapshotWith(
        {
          position: { latitudeDeg: -14.968, longitudeDeg: -147.638 },
          source: 'signalK',
          sourceId: 'gps.1',
        },
        true,
        [
          {
            quantity: 'depthBelowTransducer',
            value: 12.46,
            source: 'signalK',
            sourceId: 'sounder',
            measuredAt: now,
            ageMs: 500,
            stale: false,
            freshSources: 1,
          },
          {
            quantity: 'speedOverGround',
            value: 4.115,
            source: 'signalK',
            sourceId: 'gps.1',
            measuredAt: now,
            ageMs: 9000,
            stale: true,
            freshSources: 0,
          },
        ],
      ),
    );
    expect((await screen.findByText(/Position obsolète/)).textContent).toContain('12 s');
    expect(screen.queryByText('POSITION SIMULÉE')).toBeNull();
    const instruments = screen.getByTestId('instruments');
    expect(instruments.textContent).toContain('12.5 m');
    expect(instruments.textContent).toContain('8.0 kn');
    const sog = instruments.querySelector('[data-quantity="speedOverGround"]');
    expect(sog?.classList.contains('stale')).toBe(true);
    expect(screen.getByLabelText('donnée obsolète')).toBeTruthy();
  });

  it("affiche l'état de la connexion Signal K et sa perte", async () => {
    const events = fakeListen();
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.reject(new Error('x')),
        get_sources: () =>
          Promise.resolve(
            sourcesView({
              mode: 'signalK',
              signalkUrl: 'http://boat.local:3000',
              tokenStored: true,
              connection: { state: 'connecting', attempt: 1 },
            }),
          ),
      }),
      listen: events.listen,
    });
    expect((await screen.findByTestId('source-mode')).textContent).toContain('boat.local');
    expect(screen.getByTestId('source-status').textContent).toContain('tentative 1');

    events.emit(
      { state: 'connected', serverName: 'signalk-server', serverVersion: '2.8.0' },
      'source-status',
    );
    await vi.waitFor(() =>
      expect(screen.getByTestId('source-status').textContent).toContain('Connecté'),
    );

    events.emit(
      { state: 'disconnected', reason: 'connexion fermée', retryInMs: 2000 },
      'source-status',
    );
    await vi.waitFor(() => {
      const status = screen.getByTestId('source-status');
      expect(status.textContent).toContain('Déconnecté : connexion fermée');
      expect(status.getAttribute('role')).toBe('alert');
    });
  });

  it('enregistre une source Signal K sans conserver le jeton saisi', async () => {
    const calls: unknown[] = [];
    const base = fakeIpc({
      get_engine_info: () => Promise.resolve(engine),
      get_chart_catalog: () => Promise.reject(new Error('x')),
    });
    const invoke = (command: string, args?: Record<string, unknown>) => {
      if (command === 'set_sources') {
        calls.push(args?.request);
        return Promise.resolve(
          sourcesView({ mode: 'signalK', signalkUrl: 'http://boat.local:3000', tokenStored: true }),
        );
      }
      return base(command);
    };
    render(App, { invoke, listen: fakeListen().listen });
    await fireEvent.click(await screen.findByRole('button', { name: 'Modifier la source' }));
    await fireEvent.click(screen.getByLabelText('Signal K'));
    await fireEvent.input(screen.getByLabelText(/Adresse du serveur/), {
      target: { value: 'http://boat.local:3000' },
    });
    await fireEvent.input(screen.getByLabelText(/Jeton/), { target: { value: 'secret-token' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Appliquer' }));
    await vi.waitFor(() => expect(calls).toHaveLength(1));
    expect(calls[0]).toEqual({
      mode: 'signalK',
      signalkUrl: 'http://boat.local:3000',
      nmea: null,
      priorities: [],
      token: 'secret-token',
      clearToken: false,
    });
    expect((await screen.findByTestId('source-status')).textContent).toContain('En attente');
    expect(screen.queryByLabelText(/Jeton/)).toBeNull();
  });

  it('rejette visiblement un état Signal K invalide', async () => {
    const events = fakeListen();
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.reject(new Error('x')),
      }),
      listen: events.listen,
    });
    await screen.findByText('Aucune position');
    events.emit({ state: 'flying' }, 'source-status');
    expect((await screen.findByText(/État de la source rejeté/)).textContent).toContain('flying');
  });

  it('configure une liaison NMEA série et affiche les rejets', async () => {
    const events = fakeListen();
    const calls: unknown[] = [];
    const base = fakeIpc({
      get_engine_info: () => Promise.resolve(engine),
      get_chart_catalog: () => Promise.reject(new Error('x')),
    });
    const invoke = (command: string, args?: Record<string, unknown>) => {
      if (command === 'set_sources') {
        const request = args?.request as Record<string, unknown>;
        calls.push(request);
        return Promise.resolve(
          sourcesView({ mode: 'nmea0183', nmea: request.nmea, priorities: request.priorities }),
        );
      }
      return base(command);
    };
    render(App, { invoke, listen: events.listen });
    await fireEvent.click(await screen.findByRole('button', { name: 'Modifier la source' }));
    await fireEvent.click(screen.getByLabelText('NMEA 0183'));
    await fireEvent.change(screen.getByLabelText('Liaison'), { target: { value: 'serial' } });
    await fireEvent.input(await screen.findByLabelText('Port série'), {
      target: { value: 'COM3' },
    });
    await fireEvent.change(screen.getByLabelText('Débit'), { target: { value: '38400' } });
    await fireEvent.input(screen.getByLabelText(/Priorités/), {
      target: { value: 'nmea0183.GP\nnmea0183.GN' },
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Appliquer' }));
    await vi.waitFor(() => expect(calls).toHaveLength(1));
    expect(calls[0]).toMatchObject({
      mode: 'nmea0183',
      nmea: { kind: 'serial', path: 'COM3', baudRate: 38400 },
      priorities: ['nmea0183.GP', 'nmea0183.GN'],
    });
    expect((await screen.findByTestId('source-mode')).textContent).toContain(
      'NMEA 0183 — série COM3 (38400 Bd)',
    );

    events.emit({ count: 4, lastReason: 'somme de contrôle fausse' }, 'source-rejections');
    expect((await screen.findByTestId('source-rejections')).textContent).toContain(
      '4 données rejetées — dernière : somme de contrôle fausse',
    );
  });

  it('refuse une liaison NMEA incomplète sans appeler le moteur', async () => {
    const calls: string[] = [];
    const base = fakeIpc({
      get_engine_info: () => Promise.resolve(engine),
      get_chart_catalog: () => Promise.reject(new Error('x')),
    });
    const invoke = (command: string) => {
      calls.push(command);
      return base(command);
    };
    render(App, { invoke, listen: fakeListen().listen });
    await fireEvent.click(await screen.findByRole('button', { name: 'Modifier la source' }));
    await fireEvent.click(screen.getByLabelText('NMEA 0183'));
    await fireEvent.click(screen.getByRole('button', { name: 'Appliquer' }));
    expect(await screen.findByText("adresse de l'équipement requise")).toBeTruthy();
    expect(calls).not.toContain('set_sources');
  });

  it('signale un rejeu comme position non actuelle', async () => {
    const events = fakeListen();
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.reject(new Error('x')),
      }),
      listen: events.listen,
    });
    await screen.findByText('Aucune position');
    const s = snapshotWith({
      position: { latitudeDeg: -14.968, longitudeDeg: -147.638 },
      source: 'replay',
      sourceId: 'replay.GP',
    });
    s.position.freshSources = 2;
    events.emit(s);
    expect(await screen.findByText('REJEU — POSITION NON ACTUELLE')).toBeTruthy();
    expect(screen.getByTestId('position-sources').textContent).toContain('2 sources');
  });

  it('rejette visiblement une position invalide', async () => {
    const events = fakeListen();
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.reject(new Error('x')),
      }),
      listen: events.listen,
    });
    await screen.findByText('Aucune position');
    events.emit(
      snapshotWith({
        position: { latitudeDeg: 95, longitudeDeg: 0 },
        source: 'simulator',
        sourceId: 'simulator',
      }),
    );
    expect((await screen.findByText(/Données rejetées/)).textContent).toContain('WGS84');
  });

  it('indique le répertoire des cartes lorsque le catalogue est vide', async () => {
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.resolve(emptyCatalog),
      }),
      listen: fakeListen().listen,
    });
    const hint = await screen.findByTestId('no-charts');
    expect(hint.textContent).toContain('C:\\NautiK\\charts');
    expect(await screen.findByTestId('chart-view-stub')).toBeTruthy();
  });

  it('affiche les cartes chargées et signale les cartes rejetées', async () => {
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () =>
          Promise.resolve({
            ...emptyCatalog,
            charts: [
              {
                id: 'tahiti',
                name: 'Tahiti (test)',
                fileName: 'tahiti.mbtiles',
                description: null,
                format: 'png',
                minZoom: 5,
                maxZoom: 11,
                bounds: null,
                center: null,
                attribution: null,
                warnings: [],
              },
            ],
            failures: [{ fileName: 'bad.mbtiles', reason: 'base corrompue' }],
          }),
      }),
      listen: fakeListen().listen,
    });
    expect(await screen.findByText(/Tahiti \(test\)/)).toBeTruthy();
    expect(screen.getByText(/Carte ignorée bad.mbtiles : base corrompue/)).toBeTruthy();
  });

  it('affiche les routes enregistrées', async () => {
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.reject(new Error('x')),
        list_routes: () =>
          Promise.resolve([
            {
              id: 'r1',
              name: 'Tiputa → Avatoru',
              description: null,
              waypoints: [
                {
                  id: 'a',
                  name: 'WP001',
                  position: { latitudeDeg: -14.968, longitudeDeg: -147.638 },
                  description: null,
                },
                {
                  id: 'b',
                  name: 'WP002',
                  position: { latitudeDeg: -14.95, longitudeDeg: -147.7 },
                  description: null,
                },
              ],
              totalDistanceM: 18520,
              active: false,
            },
          ]),
      }),
      listen: fakeListen().listen,
    });
    expect(await screen.findByText('Tiputa → Avatoru')).toBeTruthy();
    expect(screen.getByText(/10\.0 NM — 2 WP/)).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Naviguer' })).toBeTruthy();
  });

  it("affiche l'état de navigation reçu du moteur puis sa désactivation", async () => {
    const events = fakeListen();
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.reject(new Error('x')),
      }),
      listen: events.listen,
    });
    await screen.findByText('Aucune position');
    events.emit(navigationStatus, 'navigation-status');
    const data = await screen.findByTestId('navigation-data');
    expect(data.textContent).toContain('1.0 NM');
    expect(data.textContent).toContain('180°');
    expect(data.textContent).toContain('0.05 NM G');
    expect(screen.getByText('SIMULATION')).toBeTruthy();

    events.emit(null, 'navigation-status');
    await vi.waitFor(() => expect(screen.queryByTestId('navigation-data')).toBeNull());
  });

  it('rejette visiblement un état de navigation invalide', async () => {
    const events = fakeListen();
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.reject(new Error('x')),
      }),
      listen: events.listen,
    });
    await screen.findByText('Aucune position');
    events.emit({ ...navigationStatus, distanceToTargetM: 'loin' }, 'navigation-status');
    expect(await screen.findByText(/État de navigation rejeté/)).toBeTruthy();
    expect(screen.queryByTestId('navigation-data')).toBeNull();
  });

  it('signale une erreur de chargement des routes', async () => {
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.reject(new Error('x')),
        list_routes: () => Promise.reject(new Error('base verrouillée')),
      }),
      listen: fakeListen().listen,
    });
    expect(await screen.findByText('base verrouillée')).toBeTruthy();
  });

  it('signale un répertoire de cartes inaccessible', async () => {
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.resolve({ ...emptyCatalog, error: 'accès refusé' }),
      }),
      listen: fakeListen().listen,
    });
    expect((await screen.findByText(/Cartes indisponibles/)).textContent).toContain('accès refusé');
  });
});
