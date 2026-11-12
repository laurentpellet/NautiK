import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
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

function fakeIpc(responses: Record<string, () => Promise<unknown>>) {
  return (command: string) =>
    responses[command]?.() ?? Promise.reject(new Error(`commande inattendue ${command}`));
}

function fakeListen() {
  const handlers: Handler[] = [];
  const listen = (_event: string, handler: Handler) => {
    handlers.push(handler);
    return Promise.resolve(() => {});
  };
  return { listen, emit: (payload: unknown) => handlers.forEach((h) => h({ payload })) };
}

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

    events.emit({
      position: { latitudeDeg: -17.535, longitudeDeg: -149.569 },
      cogRad: null,
      sogMps: 0,
      source: 'simulator',
      sourceId: 'simulator',
      measuredAt: new Date().toISOString(),
      receivedAt: new Date().toISOString(),
    });
    expect((await screen.findByTestId('position')).textContent).toContain('17°32.100′ S');
    expect(screen.getByText('POSITION SIMULÉE')).toBeTruthy();
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
    events.emit({ position: { latitudeDeg: 95, longitudeDeg: 0 } });
    expect((await screen.findByText(/Relevé rejeté/)).textContent).toContain('WGS84');
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
