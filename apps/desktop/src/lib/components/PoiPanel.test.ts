import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import PoiPanel from './PoiPanel.svelte';
import { PoiApi } from '../services/poi';

const region = (fetched: boolean) => ({
  id: 'wallis-futuna',
  name: 'Wallis-et-Futuna',
  boxes: [{ south: -14.6, west: -178.3, north: -13, east: -175.9 }],
  fetchedAt: fetched ? '2026-11-14T06:00:00Z' : null,
  poiCount: fetched ? 42 : 0,
});

function setup(download: () => Promise<unknown>) {
  const calls: string[] = [];
  const api = new PoiApi((command) => {
    calls.push(command);
    if (command === 'list_poi_regions') return Promise.resolve([region(false)]);
    if (command === 'download_pois') return download();
    if (command === 'delete_pois') return Promise.resolve([region(false)]);
    return Promise.reject(new Error(command));
  });
  const onchange = vi.fn();
  render(PoiPanel, { api, onchange });
  return { calls, onchange };
}

describe('panneau des îles et amers', () => {
  afterEach(() => vi.restoreAllMocks());

  it('télécharge une région puis signale le changement', async () => {
    const { onchange } = setup(() => Promise.resolve([region(true)]));
    await fireEvent.click(
      await screen.findByRole('button', { name: 'Télécharger Wallis-et-Futuna' }),
    );
    expect(await screen.findByText(/42 îles et amers enregistrés/)).toBeTruthy();
    expect(screen.getByText(/42 éléments/)).toBeTruthy();
    expect(onchange).toHaveBeenCalledOnce();
    expect(screen.getByRole('button', { name: 'Mettre à jour Wallis-et-Futuna' })).toBeTruthy();
  });

  it("affiche l'erreur de téléchargement sans rien changer", async () => {
    const { onchange } = setup(() => Promise.reject(new Error('délai dépassé')));
    await fireEvent.click(
      await screen.findByRole('button', { name: 'Télécharger Wallis-et-Futuna' }),
    );
    expect((await screen.findByRole('alert')).textContent).toContain('délai dépassé');
    expect(onchange).not.toHaveBeenCalled();
    expect(screen.getByText('non téléchargée')).toBeTruthy();
  });

  it('demande confirmation avant de supprimer', async () => {
    const { calls } = setup(() => Promise.resolve([region(true)]));
    await fireEvent.click(
      await screen.findByRole('button', { name: 'Télécharger Wallis-et-Futuna' }),
    );
    const remove = await screen.findByRole('button', { name: 'Supprimer Wallis-et-Futuna' });
    vi.spyOn(window, 'confirm').mockReturnValue(false);
    await fireEvent.click(remove);
    expect(calls).not.toContain('delete_pois');
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    await fireEvent.click(remove);
    expect(await screen.findByText(/données supprimées/)).toBeTruthy();
    expect(calls).toContain('delete_pois');
  });
});
