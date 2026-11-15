import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import { createRawSnippet } from 'svelte';
import MarksPanel from './MarksPanel.svelte';
import TrackReplayPanel from './TrackReplayPanel.svelte';
import SearchBox from './SearchBox.svelte';
import LogbookView from './LogbookView.svelte';
import RoutesPanel from './RoutesPanel.svelte';
import { NavigationController } from '../navigation.svelte';
import { RoutingApi } from '../services/routing';
import { MarksController } from '../marks.svelte';
import { MarksApi } from '../services/marks';
import { ReplayController } from '../replay.svelte';
import { LogbookController } from '../logbook.svelte';
import { LogbookApi } from '../services/logbook';

const pos = { latitudeDeg: -17.5, longitudeDeg: -149.6 };
const mark = {
  id: 'm1',
  name: 'Patate',
  position: pos,
  icon: 'danger',
  note: 'Affleure à basse mer',
  createdAt: '2026-11-14T06:00:00Z',
  updatedAt: '2026-11-14T06:00:00Z',
};

describe('panneau des marques', () => {
  it('liste, cadre et pose une marque au bateau', async () => {
    const call = vi.fn((command: string) =>
      Promise.resolve(command === 'list_marks' ? [mark] : mark),
    );
    const marks = new MarksController(new MarksApi(call));
    await marks.refresh();
    const onzoom = vi.fn();
    render(MarksPanel, { marks, boat: pos, onzoom });
    expect(screen.getByText('Affleure à basse mer')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Patate' }));
    expect(onzoom).toHaveBeenCalledWith(pos);
    await fireEvent.click(screen.getByRole('button', { name: '+ Au bateau' }));
    expect(marks.draft?.position).toEqual(pos);
    await fireEvent.input(screen.getByLabelText('Nom'), { target: { value: 'Mouillage' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Enregistrer' }));
    expect(call).toHaveBeenCalledWith(
      'save_mark',
      expect.objectContaining({ name: 'Mouillage', icon: 'flag' }),
    );
  });

  it('refuse la marque au bateau sans position fraîche', () => {
    const marks = new MarksController(new MarksApi(vi.fn(() => Promise.resolve([]))));
    render(MarksPanel, { marks, boat: null, onzoom: vi.fn() });
    const button = screen.getByRole('button', { name: '+ Au bateau' }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    expect(screen.getByText('Aucune marque.')).toBeTruthy();
  });
});

describe('panneau de relecture', () => {
  it('affiche les statistiques et le point courant', async () => {
    const replay = new ReplayController(() =>
      Promise.resolve({
        stats: {
          distanceM: 1852,
          durationS: 3600,
          movingS: 1800,
          averageMps: 0.5,
          movingAverageMps: 1,
          maxMps: 2,
          minDepthM: 3,
        },
        points: [
          { at: 0, position: pos, speedMps: null, depthM: null, distanceM: 0 },
          { at: 3_600_000, position: pos, speedMps: 2, depthM: 3, distanceM: 1852 },
        ],
      }),
    );
    await replay.load('t1', 'Sortie');
    render(TrackReplayPanel, { replay });
    expect(screen.getByTestId('replay-stats').textContent).toContain('1 h 00');
    await fireEvent.input(screen.getByLabelText('Position dans la trace'), {
      target: { value: '1' },
    });
    expect(replay.index).toBe(1);
    expect(screen.getByTestId('replay-now').textContent).toContain('3.9 kn');
    await fireEvent.click(screen.getByRole('button', { name: 'Fermer la relecture' }));
    expect(replay.active).toBe(false);
  });
});

describe('recherche', () => {
  it('affiche les résultats et transmet le choix', async () => {
    const call = vi.fn(() =>
      Promise.resolve([{ kind: 'mark', name: 'Patate', detail: null, position: pos }]),
    );
    const onselect = vi.fn();
    render(SearchBox, { onselect, call });
    await fireEvent.input(screen.getByLabelText('Rechercher un lieu'), {
      target: { value: 'pat' },
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Chercher' }));
    const option = await screen.findByRole('option');
    expect(option.textContent).toContain('Marque');
    await fireEvent.click(option);
    expect(onselect).toHaveBeenCalledWith(expect.objectContaining({ name: 'Patate' }));
  });

  it('signale l’absence de résultat et les erreurs', async () => {
    const call = vi
      .fn()
      .mockResolvedValueOnce([])
      .mockRejectedValueOnce(new Error('recherche trop longue'));
    render(SearchBox, { onselect: vi.fn(), call });
    await fireEvent.click(screen.getByRole('button', { name: 'Chercher' }));
    expect(await screen.findByText('Aucun résultat.')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Chercher' }));
    expect((await screen.findByRole('alert')).textContent).toBe('recherche trop longue');
  });
});

describe('journal de bord', () => {
  it('affiche les entrées, signale une position obsolète et ajoute une entrée', async () => {
    const entry = {
      id: 'e1',
      at: '2026-11-14T08:00:00Z',
      kind: 'auto',
      text: null,
      position: pos,
      positionStale: true,
      sogMps: null,
      cogRad: null,
      headingRad: null,
      twsMps: null,
      twdRad: null,
      pressurePa: 101300,
      depthM: null,
      simulated: true,
    };
    const call = vi.fn((command: string) => {
      if (command === 'list_log_entries') return Promise.resolve([entry]);
      if (command === 'get_logbook_settings') return Promise.resolve({ autoIntervalMin: 60 });
      return Promise.resolve({ ...entry, id: 'e2', kind: 'manual', text: 'Ris pris' });
    });
    const logbook = new LogbookController(new LogbookApi(call));
    const status = createRawSnippet(() => ({ render: () => '<span></span>' }));
    render(LogbookView, { logbook, status });
    expect(await screen.findByText('obsolète')).toBeTruthy();
    expect(screen.getByText('simulation')).toBeTruthy();
    expect(screen.getByText('1013.0 hPa')).toBeTruthy();
    await fireEvent.input(screen.getByLabelText("Texte de l'entrée"), {
      target: { value: 'Ris pris' },
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Ajouter' }));
    expect(await screen.findByText('Ris pris')).toBeTruthy();
    expect(call).toHaveBeenCalledWith('add_log_entry', { text: 'Ris pris' });
  });
});

describe('recherche du départ et de l’arrivée d’une route', () => {
  it('place le lieu choisi en départ puis en arrivée', async () => {
    const nav = new NavigationController(new RoutingApi(vi.fn(() => Promise.resolve([]))));
    nav.startDrawing();
    const call = vi.fn((_command: string, args?: Record<string, unknown>) =>
      Promise.resolve([
        {
          kind: 'poi',
          name: String(args?.query) === 'pap' ? 'Papeete' : 'Passe Taapuna',
          detail: null,
          position:
            String(args?.query) === 'pap' ? pos : { latitudeDeg: -17.6, longitudeDeg: -149.6 },
        },
      ]),
    );
    render(RoutesPanel, { nav, call });
    const searches = screen.getAllByRole('button', { name: 'Chercher' });
    await fireEvent.input(screen.getByLabelText('Rechercher le départ'), {
      target: { value: 'pap' },
    });
    await fireEvent.click(searches[0] as HTMLElement);
    await fireEvent.click(await screen.findByRole('option'));
    await fireEvent.input(screen.getByLabelText("Rechercher l'arrivée"), {
      target: { value: 'taa' },
    });
    await fireEvent.click(searches[1] as HTMLElement);
    await fireEvent.click(await screen.findByRole('option'));
    expect(nav.draft.map((w) => w.name)).toEqual(['Papeete', 'Passe Taapuna']);
    expect(screen.queryByRole('option'), 'résultats effacés après le choix').toBeNull();
  });
});
