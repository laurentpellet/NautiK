import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import RangeBearingPanel from './RangeBearingPanel.svelte';
import { MapToolsController } from '../mapTools.svelte';
import { RoutingApi } from '../services/routing';
import { SimulatorApi } from '../services/simulator';

function tools() {
  const call = vi.fn(() => Promise.reject(new Error('non utilisé')));
  const t = new MapToolsController(new RoutingApi(call), new SimulatorApi(call), () => 3);
  t.toggleRangeBearing();
  return t;
}

describe('panneau VRM/EBL', () => {
  it('règle le repère à la main, en milles et degrés', async () => {
    const t = tools();
    render(RangeBearingPanel, { tools: t, boatKnown: true });
    expect(screen.getByTestId('vrm-value').textContent).toBe('—');
    await fireEvent.input(screen.getByLabelText(/Rayon/), { target: { value: '1,5' } });
    await fireEvent.input(screen.getByLabelText(/Relèvement \(°\)/), { target: { value: '90' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Appliquer' }));
    expect(t.rangeBearing.radiusM).toBeCloseTo(1.5 * 1852);
    expect(t.rangeBearing.bearingRad).toBeCloseTo(Math.PI / 2);
    expect(screen.getByTestId('ebl-value').textContent).toBe('090°');
  });

  it('refuse une saisie non numérique et signale un bateau inconnu', async () => {
    const t = tools();
    render(RangeBearingPanel, { tools: t, boatKnown: false });
    expect(screen.getByText(/Position du bateau inconnue/)).toBeTruthy();
    await fireEvent.input(screen.getByLabelText(/Rayon/), { target: { value: 'abc' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Appliquer' }));
    expect(t.error).toBe('Valeur non numérique.');
    expect(t.rangeBearing.radiusM).toBeNull();
  });
});
