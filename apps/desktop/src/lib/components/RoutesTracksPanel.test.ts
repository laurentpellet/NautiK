import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import RoutesTracksPanel from './RoutesTracksPanel.svelte';
import { NavigationController } from '../navigation.svelte';
import { RoutingApi } from '../services/routing';

const p = (latitudeDeg: number, longitudeDeg: number) => ({ latitudeDeg, longitudeDeg });

const route = {
  id: 'r1',
  name: 'Papeete - Moorea',
  description: null,
  waypoints: [
    { id: 'w1', name: 'Papeete', position: p(-17.53, -149.57), description: null },
    { id: 'w2', name: 'Vaiare', position: p(-17.52, -149.77), description: null },
  ],
  totalDistanceM: 21_000,
  active: false,
  bounds: { west: -149.77, south: -17.53, east: -149.57, north: -17.52 },
};

const track = {
  id: 't1',
  name: 'Tour de Moorea',
  startedAt: '2026-11-10T18:00:00Z',
  endedAt: '2026-11-10T23:10:00Z',
  isRecording: false,
  pointCount: 3,
};

function setup() {
  localStorage.removeItem('nautik.routesTab');
  const calls: [string, Record<string, unknown> | undefined][] = [];
  const call = vi.fn((command: string, args?: Record<string, unknown>): Promise<unknown> => {
    calls.push([command, args]);
    switch (command) {
      case 'list_routes':
        return Promise.resolve([route]);
      case 'list_tracks':
        return Promise.resolve([track]);
      case 'get_navigation_state':
        return Promise.resolve({ activeRouteId: null, recordingTrackId: null });
      case 'plan_route': {
        const n = (args?.positions as unknown[]).length;
        return Promise.resolve({
          speedMps: args?.speedMps,
          departure: args?.departure ?? null,
          legs: Array.from({ length: n - 1 }, (_, i) => ({
            fromIndex: i,
            distanceM: 18_520,
            bearingRad: Math.PI / 2,
            durationS: 3600,
            cumulativeDistanceM: 18_520 * (i + 1),
            cumulativeDurationS: 3600 * (i + 1),
            arrival: args?.departure ? '2026-11-14T07:00:00Z' : null,
          })),
          totalDistanceM: 18_520 * (n - 1),
          totalDurationS: 3600 * (n - 1),
          arrival: args?.departure ? '2026-11-14T07:00:00Z' : null,
          bounds: route.bounds,
        });
      }
      case 'get_track':
        return Promise.resolve({
          ...track,
          points: [{ position: p(-17.48, -149.85) }, { position: p(-17.47, -149.75) }],
          lengthM: 31_000,
          durationS: 18_600,
          bounds: { west: -149.85, south: -17.48, east: -149.75, north: -17.47 },
        });
      case 'rename_track':
        return Promise.resolve(null);
      case 'save_route':
        return Promise.resolve({ ...route, name: String(args?.name) });
      default:
        return Promise.resolve(null);
    }
  });
  const nav = new NavigationController(new RoutingApi(call));
  return { nav, calls };
}

describe('panneau Routes et traces', () => {
  it('affiche ou masque traces et routes avec l’œil', async () => {
    localStorage.removeItem('nautik.hiddenRoutes');
    const { nav } = setup();
    await nav.refresh();
    render(RoutesTracksPanel, { nav });
    // Route : œil ouvert, puis barré, mémorisé.
    const routeEye = screen.getByRole('button', { name: 'Masquer Papeete - Moorea' });
    expect(routeEye.getAttribute('aria-pressed')).toBe('true');
    await fireEvent.click(routeEye);
    expect(nav.hiddenRouteIds).toEqual(['r1']);
    expect(JSON.parse(localStorage.getItem('nautik.hiddenRoutes') ?? '[]')).toEqual(['r1']);
    const shown = screen.getByRole('button', { name: 'Afficher Papeete - Moorea' });
    expect(shown.getAttribute('aria-pressed')).toBe('false');
    await fireEvent.click(shown);
    expect(nav.hiddenRouteIds).toEqual([]);

    // Trace : masquée par défaut, l'œil l'affiche puis la masque.
    await fireEvent.click(screen.getByRole('tab', { name: /Traces/ }));
    await fireEvent.click(screen.getByRole('button', { name: 'Afficher Tour de Moorea' }));
    await vi.waitFor(() => expect(nav.displayedTracks.map((t) => t.id)).toEqual(['t1']));
    await fireEvent.click(screen.getByRole('button', { name: 'Masquer Tour de Moorea' }));
    expect(nav.displayedTracks).toEqual([]);
    localStorage.removeItem('nautik.hiddenRoutes');
  });

  it('garde la route suivie affichée : œil désactivé', async () => {
    localStorage.removeItem('nautik.hiddenRoutes');
    const { nav } = setup();
    await nav.refresh();
    nav.routes = nav.routes.map((r) => ({ ...r, active: true }));
    render(RoutesTracksPanel, { nav });
    const eye = screen.getByRole('button', {
      name: 'Masquer Papeete - Moorea',
    }) as HTMLButtonElement;
    expect(eye.disabled).toBe(true);
    expect(eye.title).toBe('Route suivie : toujours affichée');
  });

  it('passe des routes aux traces et garde l’onglet Routes pendant l’édition', async () => {
    const { nav } = setup();
    await nav.refresh();
    render(RoutesTracksPanel, { nav });
    const tracksTab = screen.getByRole('tab', { name: /Traces/ });
    await fireEvent.click(tracksTab);
    expect(tracksTab.getAttribute('aria-selected')).toBe('true');
    expect(screen.getByText('Tour de Moorea')).toBeTruthy();
    expect(localStorage.getItem('nautik.routesTab')).toBe('tracks');

    nav.startDrawing();
    await vi.waitFor(() =>
      expect(screen.getByRole('tab', { name: /Routes/ }).getAttribute('aria-selected')).toBe(
        'true',
      ),
    );
    expect((screen.getByRole('tab', { name: /Traces/ }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('édite une route : départ, arrivée, noms, plan et enregistrement', async () => {
    const { nav, calls } = setup();
    await nav.refresh();
    render(RoutesTracksPanel, { nav });
    await fireEvent.click(screen.getByRole('button', { name: 'Modifier' }));
    expect(nav.focus?.bounds.west).toBe(-149.77);

    // Arrivée : le prochain clic sur la carte ajoute « Arrivée » en fin de route.
    const arrival = screen.getByRole('button', { name: '+ Arrivée' });
    await fireEvent.click(arrival);
    expect(arrival.getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByRole('status').textContent).toMatch(/nouvelle arrivée/);
    nav.addPoint(p(-17.49, -149.85));
    await vi.waitFor(() =>
      expect((screen.getByLabelText('Nom du point 3') as HTMLInputElement).value).toBe('Arrivée'),
    );

    // Renommage d'un point depuis la liste.
    await fireEvent.input(screen.getByLabelText('Nom du point 2'), {
      target: { value: 'Passe de Taapuna' },
    });
    expect(nav.draft[1]?.name).toBe('Passe de Taapuna');

    // Plan calculé par le moteur : longueur et durée.
    await vi.waitFor(() => expect(screen.getByTestId('plan-duration').textContent).toBe('2 h 00'));
    expect(screen.getByTestId('plan-length').textContent).toBe('20.0 NM');
    // Longueur et relèvement de chaque segment, au-dessus du point d'arrivée.
    const legs = screen.getAllByTestId('editor-leg').map((l) => l.textContent?.trim());
    expect(legs).toEqual(['↓ 10.0 NM · 090°', '↓ 10.0 NM · 090°']);

    // Point ajouté : segment en attente du nouveau plan, jamais une valeur périmée.
    nav.addPoint(p(-17.45, -149.9));
    await vi.waitFor(() => expect(screen.getAllByTestId('editor-leg')).toHaveLength(3));
    expect(screen.getAllByTestId('editor-leg')[2]?.textContent?.trim()).toBe('↓ …');
    await vi.waitFor(() =>
      expect(screen.getAllByTestId('editor-leg')[2]?.textContent?.trim()).toBe('↓ 10.0 NM · 090°'),
    );
    nav.removePoint(3);

    // Vitesse prévue saisie dans l'unité choisie (nœuds), transmise en m/s.
    await fireEvent.change(screen.getByLabelText('Vitesse prévue'), { target: { value: '5' } });
    expect(nav.speedMps).toBeCloseTo((5 * 1852) / 3600);

    await fireEvent.click(screen.getByRole('button', { name: 'Enregistrer' }));
    await vi.waitFor(() => expect(nav.editor).toBeNull());
    const saved = calls.find(([c]) => c === 'save_route')?.[1];
    expect(saved?.id).toBe('r1');
    expect((saved?.waypoints as { name: string }[]).map((w) => w.name)).toEqual([
      'Papeete',
      'Passe de Taapuna',
      'Arrivée',
    ]);
  });

  it('n’enregistre pas une route sans nom ni deux points', async () => {
    const { nav } = setup();
    render(RoutesTracksPanel, { nav });
    await fireEvent.click(screen.getByRole('button', { name: 'Nouvelle route' }));
    const save = screen.getByRole('button', { name: 'Enregistrer' }) as HTMLButtonElement;
    expect(save.disabled).toBe(true);
    nav.addPoint(p(0, 0));
    nav.addPoint(p(0, 1));
    await vi.waitFor(() => expect(screen.getByLabelText('Nom du point 1')).toBeTruthy());
    expect(save.disabled, 'nom manquant').toBe(true);
    expect(screen.getByText(/Au moins deux points|LONGUEUR|Longueur/)).toBeTruthy();
  });

  it('affiche la longueur et la durée d’une trace, la cadre et la renomme', async () => {
    const { nav, calls } = setup();
    await nav.refresh();
    render(RoutesTracksPanel, { nav });
    await fireEvent.click(screen.getByRole('tab', { name: /Traces/ }));
    const panel = screen.getByRole('tabpanel', { name: /Traces/ });
    await fireEvent.click(within(panel).getByRole('button', { name: 'Zoomer' }));
    const info = await screen.findByTestId('track-info');
    expect(info.textContent).toContain('16.7 NM');
    expect(info.textContent).toContain('5 h 10');
    expect(nav.focus?.bounds.west).toBe(-149.85);

    await fireEvent.click(within(panel).getByRole('button', { name: 'Renommer' }));
    const input = screen.getByLabelText('Nouveau nom de la trace');
    await fireEvent.input(input, { target: { value: 'Sortie du dimanche' } });
    await fireEvent.click(within(panel).getByRole('button', { name: 'OK' }));
    await vi.waitFor(() =>
      expect(calls).toContainEqual(['rename_track', { id: 't1', name: 'Sortie du dimanche' }]),
    );
  });
});
