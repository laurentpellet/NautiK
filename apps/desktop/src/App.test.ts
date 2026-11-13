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
  get_simulator_settings: () => Promise.resolve({ speedMps: 4.115 }),
  get_alarms: () => Promise.resolve(alarmsSnapshot()),
  list_alarm_events: () => Promise.resolve([]),
};

function alarmsSnapshot(patch: Record<string, unknown> = {}) {
  return {
    computedAt: new Date().toISOString(),
    alarms: [
      { kind: 'anchor', label: 'Mouillage', state: 'off', since: null, message: 'Ancre levée' },
      {
        kind: 'positionLost',
        label: 'Perte de position',
        state: 'normal',
        since: null,
        message: 'Position reçue',
      },
    ],
    anchor: null,
    anchorRadiusM: 50,
    sounding: false,
    simulated: true,
    ...patch,
  };
}

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

async function openSettings() {
  await fireEvent.click(await screen.findByRole('tab', { name: 'Paramètres' }));
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
    expect(instruments.querySelector('[data-tile="depth"]')?.textContent).toMatch(/12\.5\s*m/);
    const sog = instruments.querySelector('[data-tile="sog"]');
    expect(sog?.textContent).toMatch(/8\.0\s*kn/);
    expect(sog?.classList.contains('stale')).toBe(true);
    expect(screen.getByLabelText('donnée obsolète')).toBeTruthy();
    // En-tête : la position obsolète est une alerte active.
    expect(screen.getByTestId('pill-position').textContent).toContain('Obsolète');
    expect(screen.getByTestId('header-alerts').textContent).toMatch(/alerte/);
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
    await openSettings();
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
    // La perte reste signalée sur la carte, hors de l'onglet Paramètres.
    await fireEvent.click(screen.getByRole('tab', { name: 'Carte' }));
    expect(screen.getByTestId('link-alert').textContent).toContain('connexion fermée');
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
    await openSettings();
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
    await openSettings();
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
    await openSettings();
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

  it('liste les cibles AIS, signale la simulation et les cibles perdues', async () => {
    const events = fakeListen();
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.reject(new Error('x')),
      }),
      listen: events.listen,
    });
    expect(await screen.findByText('Aucune cible AIS reçue.')).toBeTruthy();
    const now = new Date().toISOString();
    const target = {
      mmsi: '970000001',
      class: 'A',
      name: 'SIM CARGO',
      callsign: 'SIM1',
      shipType: 70,
      navigationStatus: 0,
      position: { latitudeDeg: -14.99, longitudeDeg: -147.66 },
      sogMps: 6.17,
      cogRad: Math.PI / 2,
      headingRad: Math.PI / 2,
      vectorEnd: null,
      lastSeenAt: now,
      positionAgeMs: 2000,
      state: 'active',
      history: [],
    };
    events.emit(
      {
        computedAt: now,
        simulated: true,
        targets: [target, { ...target, mmsi: '970000002', class: 'B', name: null, state: 'lost' }],
      },
      'ais-targets',
    );
    expect((await screen.findByTestId('ais-count')).textContent).toContain('2 cibles, 1 perdue');
    expect(screen.getByTestId('ais-simulated').textContent).toContain('SIMULATION');
    const rows = screen.getByTestId('ais-targets').querySelectorAll('li');
    expect(rows[0]?.textContent).toContain('SIM CARGO');
    expect(rows[0]?.textContent).toContain('12.0 kn · 090°');
    expect(rows[1]?.textContent).toContain('MMSI 970000002');
    expect(rows[1]?.textContent).toContain('PERDUE');
    await fireEvent.click(screen.getByRole('button', { name: /SIM CARGO/ }));
    expect(rows[0]?.classList.contains('selected')).toBe(true);

    events.emit({ computedAt: now, simulated: false, targets: [{ mmsi: 'x' }] }, 'ais-targets');
    expect((await screen.findByText(/Cibles AIS rejetées/)).textContent).toContain('MMSI');
  });

  it("bascule vers l'onglet Instruments et revient à la carte", async () => {
    const events = fakeListen();
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.reject(new Error('x')),
      }),
      listen: events.listen,
    });
    const chartTab = await screen.findByRole('tab', { name: 'Carte' });
    const instrumentsTab = screen.getByRole('tab', { name: 'Instruments' });
    expect(chartTab.getAttribute('aria-selected')).toBe('true');
    expect(screen.queryByTestId('tile-sog')).toBeNull();

    await fireEvent.click(instrumentsTab);
    expect(instrumentsTab.getAttribute('aria-selected')).toBe('true');
    expect(screen.getByTestId('tile-sog').textContent).toContain('—');
    expect(screen.getByTestId('instruments-status').textContent).toContain('AUCUNE POSITION');
    expect(screen.getByTestId('trend').textContent).toContain('Aucune mesure');
    expect(screen.queryByTestId('wind-arrow')).toBeNull();

    events.emit(
      snapshotWith(
        {
          position: { latitudeDeg: -14.968, longitudeDeg: -147.638 },
          source: 'simulator',
          sourceId: 'simulator',
        },
        false,
        [
          {
            quantity: 'speedOverGround',
            value: 3.3,
            source: 'simulator',
            sourceId: 'simulator',
            measuredAt: new Date().toISOString(),
            ageMs: 200,
            stale: false,
            freshSources: 1,
          },
          {
            quantity: 'windSpeedApparent',
            value: 5,
            source: 'simulator',
            sourceId: 'simulator',
            measuredAt: new Date().toISOString(),
            ageMs: 200,
            stale: false,
            freshSources: 1,
          },
          {
            quantity: 'windAngleApparent',
            value: -Math.PI / 4,
            source: 'simulator',
            sourceId: 'simulator',
            measuredAt: new Date().toISOString(),
            ageMs: 200,
            stale: false,
            freshSources: 1,
          },
        ],
      ),
    );
    await vi.waitFor(() => expect(screen.getByTestId('tile-sog').textContent).toMatch(/6\.4\s*kn/));
    expect(screen.getByTestId('tile-position').textContent).toContain('14°58.080′ S');
    expect(screen.getByTestId('wind-dial').getAttribute('aria-label')).toBe(
      'Vent apparent 45° bâbord',
    );
    expect(screen.getByTestId('wind-arrow')).toBeTruthy();
    expect(screen.getByTestId('trend').textContent).toMatch(/9\.7 kn/);
    const systems = screen.getByTestId('systems');
    expect(systems.querySelector('[data-quantity="speedOverGround"]')?.textContent).toContain(
      'simulator',
    );
    expect(screen.getByTestId('history-Vitesse').textContent).toMatch(/moy 6\.4 · max 6\.4 kn/);
    expect(screen.getByTestId('history-Profondeur').textContent).toContain('Aucune mesure');
    await fireEvent.click(screen.getByRole('radio', { name: '6 h' }));
    expect(screen.getByRole('radio', { name: '6 h' }).getAttribute('aria-checked')).toBe('true');

    await fireEvent.click(chartTab);
    expect(screen.queryByTestId('tile-sog')).toBeNull();
    expect(screen.getByTestId('compass').getAttribute('aria-label')).toBe('Cap inconnu');

    // Calques : un calque masqué reste signalé.
    await fireEvent.click(screen.getByRole('button', { name: /Calques/ }));
    await fireEvent.click(screen.getByRole('checkbox', { name: 'Cibles AIS' }));
    expect(screen.getByTestId('hidden-layers').textContent).toContain('Cibles AIS');
    await fireEvent.click(screen.getByRole('checkbox', { name: 'Cibles AIS' }));
    expect(screen.queryByTestId('hidden-layers')).toBeNull();
  });

  it('règle les unités et la vitesse simulée dans Paramètres', async () => {
    const calls: unknown[] = [];
    const base = fakeIpc({
      get_engine_info: () => Promise.resolve(engine),
      get_chart_catalog: () => Promise.reject(new Error('x')),
    });
    const invoke = (command: string, args?: Record<string, unknown>) => {
      if (command === 'set_simulator_settings') {
        calls.push(args);
        return Promise.resolve({ speedMps: args?.speedMps });
      }
      return base(command);
    };
    localStorage.clear();
    render(App, { invoke, listen: fakeListen().listen });
    await openSettings();
    const speed = await screen.findByLabelText(/Vitesse du bateau simulé \(kn\)/);
    await vi.waitFor(() => expect((speed as HTMLInputElement).value).toBe('8.0'));

    await fireEvent.change(screen.getByLabelText('Vitesse'), { target: { value: 'kmh' } });
    expect((speed as HTMLInputElement).value).toBe('14.8');
    expect(JSON.parse(localStorage.getItem('nautik.units') ?? '{}')).toMatchObject({
      speed: 'kmh',
    });

    await fireEvent.input(speed, { target: { value: '18' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Enregistrer' }));
    await vi.waitFor(() => expect(calls).toHaveLength(1));
    expect((calls[0] as { speedMps: number }).speedMps).toBeCloseTo(5);
    expect(await screen.findByText('Vitesse enregistrée.')).toBeTruthy();

    await fireEvent.input(speed, { target: { value: 'vite' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Enregistrer' }));
    expect(await screen.findByText('Vitesse invalide.')).toBeTruthy();
    expect(calls).toHaveLength(1);

    // Les sources sont réglées dans cet onglet.
    expect(screen.getByRole('button', { name: 'Modifier la source' })).toBeTruthy();
    await fireEvent.change(screen.getByLabelText('Vitesse'), { target: { value: 'kn' } });
  });

  it('bascule le plein écran : panneau masqué, alertes conservées', async () => {
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.reject(new Error('x')),
      }),
      listen: fakeListen().listen,
    });
    const button = await screen.findByRole('button', { name: 'Plein écran' });
    expect(screen.queryByTestId('fullscreen-status')).toBeNull();
    await fireEvent.click(button);
    await vi.waitFor(() =>
      expect(screen.getByTestId('fullscreen-status').textContent).toContain('AUCUNE POSITION'),
    );
    expect(screen.getByRole('complementary', { hidden: true }).hidden).toBe(true);
    await fireEvent.keyDown(window, { key: 'Escape' });
    await vi.waitFor(() => expect(screen.queryByTestId('fullscreen-status')).toBeNull());
    await fireEvent.keyDown(window, { key: 'F11' });
    await vi.waitFor(() => expect(screen.getByTestId('fullscreen-status')).toBeTruthy());
  });

  it('affiche un fichier GRIB sur la carte depuis l’onglet Météo', async () => {
    const run = Date.UTC(2026, 10, 12, 0);
    const file = {
      id: 'gfs-20261112-00z.grb2',
      sizeBytes: 5504,
      referenceTime: new Date(run).toISOString(),
      validTimes: [0, 3].map((h) => new Date(run + h * 3_600_000).toISOString()),
      bounds: { west: -150, south: -18, east: -146, north: -14 },
      skipped: [],
      error: null,
    };
    const windCalls: unknown[] = [];
    const base = fakeIpc({
      get_engine_info: () => Promise.resolve(engine),
      get_chart_catalog: () => Promise.resolve(emptyCatalog),
      list_weather_files: () => Promise.resolve([file]),
    });
    const invoke = (command: string, args?: Record<string, unknown>) => {
      if (command !== 'get_wind_field') return base(command);
      windCalls.push(args);
      return Promise.resolve({
        id: file.id,
        referenceTime: file.referenceTime,
        validTime: file.validTimes[1],
        step: 1,
        steps: 2,
        points: [{ lat: -16, lon: -148, speedMps: 7, directionRad: 1.57 }],
      });
    };
    render(App, { invoke, listen: fakeListen().listen });

    await fireEvent.click(await screen.findByRole('tab', { name: 'Météo' }));
    await vi.waitFor(() =>
      expect(screen.getByTestId('download-area').textContent).toContain('18°S–14°S'),
    );
    await fireEvent.click(await screen.findByRole('button', { name: 'Afficher sur la carte' }));

    // Retour à la carte, vent dessiné et barre d'échéances affichée.
    await vi.waitFor(() => expect(screen.getByTestId('wind-stub').textContent).toBe('1 vents'));
    expect(screen.getByTestId('wind-bar').textContent).toContain('PRÉVISION ANCIENNE');
    expect(windCalls[0]).toMatchObject({
      id: file.id,
      area: { west: -150, south: -18, east: -146, north: -14 },
      maxPoints: 1500,
    });

    await fireEvent.click(screen.getByRole('button', { name: 'Masquer le vent' }));
    await vi.waitFor(() => expect(screen.getByTestId('wind-stub').textContent).toBe('sans vent'));
    expect(screen.queryByTestId('wind-bar')).toBeNull();
  });

  it('signale un champ de vent illisible sans rien dessiner', async () => {
    const file = {
      id: 'a.grb2',
      sizeBytes: 10,
      referenceTime: new Date().toISOString(),
      validTimes: [new Date().toISOString()],
      bounds: null,
      skipped: [],
      error: null,
    };
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.resolve(emptyCatalog),
        list_weather_files: () => Promise.resolve([file]),
        get_wind_field: () => Promise.reject(new Error('fichier corrompu')),
      }),
      listen: fakeListen().listen,
    });
    await fireEvent.click(await screen.findByRole('tab', { name: 'Météo' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Afficher sur la carte' }));
    expect(await screen.findByText('Vent indisponible : fichier corrompu')).toBeTruthy();
    expect(screen.getByTestId('wind-stub').textContent).toBe('sans vent');
  });

  const anchorView = {
    position: { latitudeDeg: -17.5, longitudeDeg: -149.5 },
    radiusM: 60,
    hysteresisM: 6,
    delayS: 10,
    distanceM: 72,
    bearingRad: 0.5,
  };

  const dragging = (state: 'active' | 'acknowledged') =>
    alarmsSnapshot({
      alarms: [
        {
          kind: 'anchor',
          label: 'Mouillage',
          state,
          since: new Date().toISOString(),
          message: 'DÉRAPAGE — 72 m de l’ancre, rayon 60 m',
        },
        {
          kind: 'positionLost',
          label: 'Perte de position',
          state: 'normal',
          since: null,
          message: 'Position reçue',
        },
      ],
      anchor: anchorView,
      sounding: state === 'active',
    });

  it('affiche une alarme déclenchée partout et l’acquitte', async () => {
    const events = fakeListen();
    const calls: [string, unknown][] = [];
    const base = fakeIpc({
      get_engine_info: () => Promise.resolve(engine),
      get_chart_catalog: () => Promise.reject(new Error('x')),
    });
    render(App, {
      invoke: (command: string, args?: Record<string, unknown>) => {
        calls.push([command, args]);
        if (command === 'acknowledge_alarm') return Promise.resolve(dragging('acknowledged'));
        return base(command);
      },
      listen: events.listen,
    });
    await screen.findByText('Aucune position');
    events.emit(dragging('active'), 'alarms');

    const banner = await screen.findByTestId('alarm-banner');
    expect(banner.textContent).toContain('MOUILLAGE');
    expect(banner.textContent).toContain('72 m');
    expect(banner.textContent).toContain('SIMULATION');
    expect(screen.getByTestId('pill-alarm').className).toContain('alert');

    // Visible aussi dans un autre onglet.
    await fireEvent.click(screen.getByRole('tab', { name: 'Instruments' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Acquitter' }));
    expect(calls).toContainEqual(['acknowledge_alarm', { kind: 'anchor' }]);
    await vi.waitFor(() =>
      expect(screen.getByTestId('alarm-banner').textContent).toContain('ACQUITTÉE'),
    );
    expect(screen.queryByRole('button', { name: 'Acquitter' })).toBeNull();

    // Résolution par le moteur : le bandeau disparaît.
    events.emit(alarmsSnapshot(), 'alarms');
    await vi.waitFor(() => expect(screen.queryByTestId('alarm-banner')).toBeNull());
  });

  it("pose l'ancre à la position courante et la lève après confirmation", async () => {
    const events = fakeListen();
    const calls: [string, unknown][] = [];
    const base = fakeIpc({
      get_engine_info: () => Promise.resolve(engine),
      get_chart_catalog: () => Promise.resolve(emptyCatalog),
    });
    render(App, {
      invoke: (command: string, args?: Record<string, unknown>) => {
        calls.push([command, args]);
        if (command === 'drop_anchor') {
          return Promise.resolve(
            alarmsSnapshot({ anchor: { ...anchorView, distanceM: 3 }, anchorRadiusM: 60 }),
          );
        }
        if (command === 'raise_anchor')
          return Promise.resolve(alarmsSnapshot({ anchorRadiusM: 60 }));
        return base(command);
      },
      listen: events.listen,
    });
    const card = await screen.findByTestId('anchor-card');
    const drop = screen.getByRole('button', { name: 'Mouiller ici' });
    expect((drop as HTMLButtonElement).disabled).toBe(true);
    expect(card.textContent).toContain('Position récente requise');

    events.emit(
      snapshotWith({
        position: { latitudeDeg: -17.5, longitudeDeg: -149.5 },
        source: 'simulator',
        sourceId: 'simulator',
      }),
    );
    await vi.waitFor(() => expect((drop as HTMLButtonElement).disabled).toBe(false));
    const radius = screen.getByLabelText("Rayon d'évitage (m)");
    expect((radius as HTMLInputElement).value).toBe('50');
    await fireEvent.input(radius, { target: { value: '5' } });
    await fireEvent.click(drop);
    expect(await screen.findByText(/Rayon invalide/)).toBeTruthy();
    expect(calls.some(([c]) => c === 'drop_anchor')).toBe(false);

    await fireEvent.input(radius, { target: { value: '60' } });
    await fireEvent.click(drop);
    expect(calls).toContainEqual(['drop_anchor', { radiusM: 60, delayS: 10 }]);
    expect((await screen.findByTestId('anchor-distance')).textContent).toContain('3 m');

    await fireEvent.click(screen.getByRole('button', { name: "Lever l'ancre" }));
    expect(calls.some(([c]) => c === 'raise_anchor')).toBe(false);
    await fireEvent.click(screen.getByRole('button', { name: "Confirmer : lever l'ancre" }));
    expect(calls.some(([c]) => c === 'raise_anchor')).toBe(true);
    await screen.findByRole('button', { name: 'Mouiller ici' });
  });

  it("refuse de poser l'ancre si le moteur n'a pas de position", async () => {
    const events = fakeListen();
    render(App, {
      invoke: fakeIpc({
        get_engine_info: () => Promise.resolve(engine),
        get_chart_catalog: () => Promise.resolve(emptyCatalog),
        drop_anchor: () =>
          Promise.reject(new Error('aucune position récente : impossible de poser l’ancre')),
      }),
      listen: events.listen,
    });
    await screen.findByTestId('anchor-card');
    events.emit(
      snapshotWith({
        position: { latitudeDeg: -17.5, longitudeDeg: -149.5 },
        source: 'signalK',
        sourceId: 'gps',
      }),
    );
    const drop = screen.getByRole('button', { name: 'Mouiller ici' });
    await vi.waitFor(() => expect((drop as HTMLButtonElement).disabled).toBe(false));
    await fireEvent.click(drop);
    expect(await screen.findByText(/aucune position récente/)).toBeTruthy();
  });

  it("montre l'historique et confirme la désactivation de la surveillance", async () => {
    const calls: [string, unknown][] = [];
    const base = fakeIpc({
      get_engine_info: () => Promise.resolve(engine),
      get_chart_catalog: () => Promise.reject(new Error('x')),
      list_alarm_events: () =>
        Promise.resolve([
          {
            id: 'e1',
            alarm: 'anchor',
            kind: 'activated',
            at: new Date().toISOString(),
            details: { message: 'Dérapage : 62 m de l’ancre', distanceM: 62, simulated: true },
          },
        ]),
    });
    render(App, {
      invoke: (command: string, args?: Record<string, unknown>) => {
        calls.push([command, args]);
        if (command === 'set_alarm_enabled') {
          return Promise.resolve(
            alarmsSnapshot({
              alarms: [
                {
                  kind: 'anchor',
                  label: 'Mouillage',
                  state: 'off',
                  since: null,
                  message: 'Ancre levée',
                },
                {
                  kind: 'positionLost',
                  label: 'Perte de position',
                  state: 'off',
                  since: null,
                  message: 'Surveillance désactivée',
                },
              ],
            }),
          );
        }
        return base(command);
      },
      listen: fakeListen().listen,
    });
    await openSettings();
    const events = await screen.findByTestId('alarm-events');
    expect(events.textContent).toContain('Déclenchée');
    expect(events.textContent).toContain('(simulation)');

    await fireEvent.click(screen.getByRole('button', { name: 'Désactiver' }));
    expect(calls.some(([c]) => c === 'set_alarm_enabled')).toBe(false);
    await fireEvent.click(screen.getByRole('button', { name: 'Confirmer la désactivation' }));
    expect(calls).toContainEqual(['set_alarm_enabled', { kind: 'positionLost', enabled: false }]);
    expect(await screen.findByText('DÉSACTIVÉE')).toBeTruthy();
    expect(screen.getByTestId('pill-alarm').textContent).toContain('Réduites');
  });
});
